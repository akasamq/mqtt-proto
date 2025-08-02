use core::future::Future;
use core::mem::MaybeUninit;
use core::pin::Pin;
use core::sync::atomic::{AtomicUsize, Ordering};
use core::task::{Context, Poll, Waker};

use alloc::collections::VecDeque;
use alloc::sync::Arc;
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
    pub max_waiters: usize,
}

impl Default for MockBufferConfig {
    fn default() -> Self {
        Self {
            buffer_size: 8192,
            pool_capacity: 64,
            chunk_size: 8192,
            max_waiters: 32,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MockBufferStats {
    pub total_capacity: usize,
    pub free_buffers: usize,
    pub allocated_buffers: usize,
}

#[derive(Debug)]
struct MockBufferPoolInner {
    #[cfg(feature = "std")]
    free_buffers: std::sync::Mutex<VecDeque<Vec<u8>>>,
    #[cfg(not(feature = "std"))]
    free_buffers: embassy_sync::mutex::Mutex<
        embassy_sync::blocking_mutex::raw::NoopRawMutex,
        VecDeque<Vec<u8>>,
    >,

    #[cfg(feature = "std")]
    waiters: std::sync::Mutex<VecDeque<Waker>>,
    #[cfg(not(feature = "std"))]
    waiters: embassy_sync::mutex::Mutex<
        embassy_sync::blocking_mutex::raw::NoopRawMutex,
        VecDeque<Waker>,
    >,

    config: MockBufferConfig,
    current_count: AtomicUsize,
}

impl MockBufferPoolInner {
    fn new(config: MockBufferConfig) -> Self {
        #[cfg(feature = "std")]
        let free_buffers = std::sync::Mutex::new(VecDeque::with_capacity(config.pool_capacity));
        #[cfg(not(feature = "std"))]
        let free_buffers = embassy_sync::mutex::Mutex::new(VecDeque::new());

        #[cfg(feature = "std")]
        let waiters = std::sync::Mutex::new(VecDeque::with_capacity(config.max_waiters));
        #[cfg(not(feature = "std"))]
        let waiters = embassy_sync::mutex::Mutex::new(VecDeque::new());

        Self {
            free_buffers,
            waiters,
            config,
            current_count: AtomicUsize::new(0),
        }
    }

    fn try_acquire_buffer(self: &Arc<Self>, size: usize) -> Option<MockBufferHandle> {
        if size > self.config.buffer_size {
            return Some(MockBufferHandle::new_owned(size));
        }

        #[cfg(feature = "std")]
        let mut free_buffers = self.free_buffers.lock().unwrap();
        #[cfg(not(feature = "std"))]
        let mut free_buffers = self.free_buffers.lock();

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

    fn return_buffer(&self, buffer: Vec<u8>) {
        #[cfg(feature = "std")]
        let mut free_buffers = self.free_buffers.lock().unwrap();
        #[cfg(not(feature = "std"))]
        let mut free_buffers = self.free_buffers.lock();

        if free_buffers.len() < self.config.pool_capacity
            && buffer.capacity() >= self.config.buffer_size
        {
            free_buffers.push_back(buffer);
            self.current_count.fetch_add(1, Ordering::Relaxed);
        }

        drop(free_buffers);

        self.notify_waiters();
    }

    fn register_waker(&self, waker: Waker) {
        #[cfg(feature = "std")]
        let mut waiters = self.waiters.lock().unwrap();
        #[cfg(not(feature = "std"))]
        let mut waiters = self.waiters.lock();

        if waiters.len() < self.config.max_waiters {
            waiters.push_back(waker);
        }
    }

    fn notify_waiters(&self) {
        #[cfg(feature = "std")]
        let mut waiters = self.waiters.lock().unwrap();
        #[cfg(not(feature = "std"))]
        let mut waiters = self.waiters.lock();

        while let Some(waker) = waiters.pop_front() {
            waker.wake();
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
                pool.return_buffer(core::mem::take(&mut self.data));
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
        self.data.len()
    }
}

struct WaitForBuffer {
    pool: Arc<MockBufferPoolInner>,
    size: usize,
    waker_registered: bool,
}

impl Future for WaitForBuffer {
    type Output = Result<MockBufferHandle, Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if let Some(buffer) = self.pool.try_acquire_buffer(self.size) {
            return Poll::Ready(Ok(buffer));
        }

        if !self.waker_registered {
            self.pool.register_waker(cx.waker().clone());
            self.waker_registered = true;
        }

        Poll::Pending
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
        if let Some(handle) = self.inner.try_acquire_buffer(size) {
            return Ok(handle);
        }

        let wait_future = WaitForBuffer {
            pool: Arc::clone(&self.inner),
            size,
            waker_registered: false,
        };

        wait_future.await
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
