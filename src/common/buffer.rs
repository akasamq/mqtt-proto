use core::mem::MaybeUninit;
use core::ptr;
use core::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};

use alloc::boxed::Box;
use alloc::collections::VecDeque;
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;

use super::Error;

#[derive(Debug)]
pub enum BufferResult<H: BufferHandle> {
    Pooled(H),
    Owned(Vec<u8>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadStrategy {
    Buffer,
    Chunk(usize),
}

#[allow(async_fn_in_trait)]
pub trait BufferHandle: Send + Sync {
    type Error;

    fn as_mut_slice(&mut self) -> (&mut [MaybeUninit<u8>], usize);

    fn as_slice(&self, len: usize) -> &[u8];

    fn set_len(&mut self, len: usize);

    fn len(&self) -> usize;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn capacity(&self) -> usize;
}

#[allow(async_fn_in_trait)]
pub trait Buffer: Send + Sync {
    type Handle: BufferHandle<Error = Self::Error> + Clone;
    type Error;

    async fn acquire(&mut self, size: usize) -> Result<Self::Handle, Self::Error>;

    async fn release(&mut self, handle: Self::Handle) -> Result<(), Self::Error>;

    fn read_strategy(&self, packet_size: usize) -> ReadStrategy;
}

impl<B: Buffer> Buffer for &mut B {
    type Handle = B::Handle;
    type Error = B::Error;

    async fn acquire(&mut self, size: usize) -> Result<Self::Handle, Self::Error> {
        (**self).acquire(size).await
    }

    async fn release(&mut self, handle: Self::Handle) -> Result<(), Self::Error> {
        (**self).release(handle).await
    }

    fn read_strategy(&self, packet_size: usize) -> ReadStrategy {
        (**self).read_strategy(packet_size)
    }
}

#[derive(Debug, Clone)]
pub struct MockBufferConfig {
    pub buffer_size: usize,
    pub pool_capacity: usize,
    pub chunk_size: usize,
}

impl Default for MockBufferConfig {
    fn default() -> Self {
        Self {
            buffer_size: 8192,
            pool_capacity: 64,
            chunk_size: 8192,
        }
    }
}

struct PendingBufferNode {
    buffer: Vec<u8>,
    next: *mut PendingBufferNode,
}

#[derive(Debug)]
struct MockBufferPoolInner {
    #[cfg(feature = "tokio")]
    free_buffers: tokio::sync::Mutex<VecDeque<Vec<u8>>>,
    #[cfg(not(feature = "tokio"))]
    free_buffers: embassy_sync::mutex::Mutex<
        embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex,
        VecDeque<Vec<u8>>,
    >,

    // Lock-free stack for buffers returned from Drop
    pending_returns: AtomicPtr<PendingBufferNode>,

    config: MockBufferConfig,
    current_count: AtomicUsize,
}

impl MockBufferPoolInner {
    fn new(config: MockBufferConfig) -> Self {
        #[cfg(feature = "tokio")]
        let free_buffers = tokio::sync::Mutex::new(VecDeque::with_capacity(config.pool_capacity));
        #[cfg(not(feature = "tokio"))]
        let free_buffers = embassy_sync::mutex::Mutex::new(VecDeque::new());

        Self {
            free_buffers,
            pending_returns: AtomicPtr::new(ptr::null_mut()),
            config,
            current_count: AtomicUsize::new(0),
        }
    }

    async fn try_acquire_buffer(self: &Arc<Self>, size: usize) -> Option<MockBufferHandle> {
        if size > self.config.buffer_size {
            return Some(MockBufferHandle::new_owned(size));
        }

        // First, try to reclaim any pending returns
        if let Some(mut buffer) = self.pop_pending_return() {
            if buffer.capacity() < size {
                buffer.reserve(size - buffer.capacity());
            }
            buffer.clear();
            return Some(MockBufferHandle::new_pooled(buffer, Arc::clone(self)));
        }

        let mut free_buffers = self.free_buffers.lock().await;
        if let Some(mut buffer) = free_buffers.pop_front() {
            if buffer.capacity() < size {
                buffer.reserve(size - buffer.capacity());
            }
            buffer.clear();
            self.current_count.fetch_sub(1, Ordering::Relaxed);
            return Some(MockBufferHandle::new_pooled(buffer, Arc::clone(self)));
        }
        drop(free_buffers);

        if self.current_count.load(Ordering::Relaxed) < self.config.pool_capacity {
            let buffer = vec![0u8; self.config.buffer_size.max(size)];
            Some(MockBufferHandle::new_pooled(buffer, Arc::clone(self)))
        } else {
            Some(MockBufferHandle::new_owned(size))
        }
    }

    // Lock-free stack operations for pending buffer returns
    fn push_pending_return(&self, buffer: Vec<u8>) {
        let node = Box::into_raw(Box::new(PendingBufferNode {
            buffer,
            next: ptr::null_mut(),
        }));

        loop {
            let head = self.pending_returns.load(Ordering::Acquire);
            unsafe {
                (*node).next = head;
            }

            match self.pending_returns.compare_exchange_weak(
                head,
                node,
                Ordering::Release,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(_) => continue,
            }
        }
    }

    fn pop_pending_return(&self) -> Option<Vec<u8>> {
        loop {
            let head = self.pending_returns.load(Ordering::Acquire);
            if head.is_null() {
                return None;
            }

            let next = unsafe { (*head).next };

            match self.pending_returns.compare_exchange_weak(
                head,
                next,
                Ordering::Release,
                Ordering::Relaxed,
            ) {
                Ok(_) => {
                    let node = unsafe { Box::from_raw(head) };
                    return Some(node.buffer);
                }
                Err(_) => continue,
            }
        }
    }

    fn return_buffer(&self, buffer: Vec<u8>) {
        // Only pool buffers that meet minimum size requirement
        if buffer.capacity() >= self.config.buffer_size {
            #[cfg(feature = "tokio")]
            {
                // Try fast path with lock first
                if let Ok(mut free_buffers) = self.free_buffers.try_lock() {
                    if free_buffers.len() < self.config.pool_capacity {
                        free_buffers.push_back(buffer);
                        self.current_count.fetch_add(1, Ordering::Relaxed);
                        return;
                    }
                }
            }

            // Fallback to lock-free pending stack for both std and no-std
            self.push_pending_return(buffer);
        }
        // If buffer is too small, just drop it
    }
}

impl Drop for MockBufferPoolInner {
    fn drop(&mut self) {
        // Clean up any remaining nodes in the pending returns stack
        while self.pop_pending_return().is_some() {
            // Just drop the buffers, the pop_pending_return handles the node cleanup
        }
    }
}

#[derive(Debug)]
pub struct MockBufferHandle {
    data: Vec<u8>,
    logical_len: usize,
    from_pool: bool,
    pool: Option<Arc<MockBufferPoolInner>>,
}

impl Clone for MockBufferHandle {
    fn clone(&self) -> Self {
        let mut new_data = vec![0u8; self.data.capacity()];
        new_data[..self.logical_len].copy_from_slice(&self.data[..self.logical_len]);
        Self {
            data: new_data,
            logical_len: self.logical_len,
            from_pool: false,
            pool: None,
        }
    }
}

impl MockBufferHandle {
    fn new_owned(capacity: usize) -> Self {
        Self {
            data: vec![0u8; capacity],
            logical_len: 0,
            from_pool: false,
            pool: None,
        }
    }

    fn new_pooled(data: Vec<u8>, pool: Arc<MockBufferPoolInner>) -> Self {
        Self {
            data,
            logical_len: 0,
            from_pool: true,
            pool: Some(pool),
        }
    }
}

impl Drop for MockBufferHandle {
    fn drop(&mut self) {
        if self.from_pool {
            if let Some(pool) = &self.pool {
                // Take the buffer and clear it, preserving capacity
                let mut buffer = core::mem::take(&mut self.data);
                // take() replaces with Vec::new() which has 0 capacity
                // But the original buffer should have the correct capacity
                // The issue is that take() gives us the original buffer, not an empty one!
                buffer.clear(); // Clear contents but keep capacity
                pool.return_buffer(buffer);
            }
        }
    }
}

impl BufferHandle for MockBufferHandle {
    type Error = Error;

    fn as_mut_slice(&mut self) -> (&mut [MaybeUninit<u8>], usize) {
        let capacity = self.data.len();
        let ptr = self.data.as_mut_ptr() as *mut MaybeUninit<u8>;
        let slice = unsafe { core::slice::from_raw_parts_mut(ptr, capacity) };
        (slice, capacity)
    }

    fn as_slice(&self, len: usize) -> &[u8] {
        let end = len.min(self.data.len());
        &self.data[..end]
    }

    fn set_len(&mut self, len: usize) {
        if self.data.len() < len {
            self.data.resize(len, 0);
        }

        self.logical_len = len;
    }

    fn len(&self) -> usize {
        self.logical_len
    }

    fn capacity(&self) -> usize {
        self.data.capacity()
    }
}

#[derive(Clone)]
pub struct MockBuffer {
    inner: Arc<MockBufferPoolInner>,
}

impl Default for MockBuffer {
    fn default() -> Self {
        Self::new(MockBufferConfig::default())
    }
}

impl MockBuffer {
    pub fn new(config: MockBufferConfig) -> Self {
        Self {
            inner: Arc::new(MockBufferPoolInner::new(config)),
        }
    }
}

impl Buffer for MockBuffer {
    type Handle = MockBufferHandle;
    type Error = Error;

    async fn acquire(&mut self, size: usize) -> Result<Self::Handle, Self::Error> {
        // Try to get a buffer immediately
        if let Some(handle) = self.inner.try_acquire_buffer(size).await {
            return Ok(handle);
        }

        // If we can't get one immediately, keep trying in a loop
        // This is simple but effective - in practice you might want exponential backoff
        loop {
            // Use embassy_futures::yield_now() to yield control and try again
            embassy_futures::yield_now().await;

            if let Some(handle) = self.inner.try_acquire_buffer(size).await {
                return Ok(handle);
            }
        }
    }

    async fn release(&mut self, handle: Self::Handle) -> Result<(), Self::Error> {
        drop(handle);
        Ok(())
    }

    fn read_strategy(&self, packet_size: usize) -> ReadStrategy {
        if packet_size <= self.inner.config.buffer_size {
            ReadStrategy::Buffer
        } else {
            ReadStrategy::Chunk(self.inner.config.chunk_size)
        }
    }
}
