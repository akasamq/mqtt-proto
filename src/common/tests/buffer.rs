use alloc::sync::Arc;
use alloc::vec::Vec;

use tokio::sync::Mutex;

use crate::*;

#[tokio::test]
async fn test_mock_buffer_basic() {
    let mut buffer = MockBuffer::new(MockBufferConfig::default());

    let handle = buffer.acquire(1024).await.unwrap();
    assert!(handle.capacity() >= 1024);

    buffer.release(handle).await.unwrap();
}

#[tokio::test]
async fn test_buffer_pool_reuse() {
    let config = MockBufferConfig {
        buffer_size: 1024,
        pool_capacity: 2,
        chunk_size: 1024,
    };
    let mut buffer = MockBuffer::new(config);

    for _ in 0..5 {
        let handle1 = buffer.acquire(512).await.unwrap();
        let handle2 = buffer.acquire(512).await.unwrap();

        assert!(handle1.capacity() >= 512);
        assert!(handle2.capacity() >= 512);

        buffer.release(handle1).await.unwrap();
        buffer.release(handle2).await.unwrap();
    }
}

#[tokio::test]
async fn test_buffer_oversized_request() {
    let config = MockBufferConfig {
        buffer_size: 1024,
        pool_capacity: 2,
        chunk_size: 1024,
    };
    let mut buffer = MockBuffer::new(config);

    let handle = buffer.acquire(2048).await.unwrap();
    assert!(handle.capacity() >= 2048);

    buffer.release(handle).await.unwrap();
}

#[tokio::test]
async fn test_buffer_handle_operations() {
    let mut buffer = MockBuffer::new(MockBufferConfig::default());
    let mut handle = buffer.acquire(1024).await.unwrap();

    assert_eq!(handle.len(), 0);
    assert!(handle.is_empty());
    assert!(handle.capacity() >= 1024);

    handle.set_len(100);
    assert_eq!(handle.len(), 100);
    assert!(!handle.is_empty());

    let slice = handle.as_slice(50);
    assert_eq!(slice.len(), 50);

    let (mut_slice, capacity) = handle.as_mut_slice();
    assert!(capacity >= 1024);
    assert!(mut_slice.len() >= 1024);

    buffer.release(handle).await.unwrap();
}

#[tokio::test]
async fn test_concurrent_buffer_access() {
    let buffer = Arc::new(Mutex::new(MockBuffer::new(MockBufferConfig {
        buffer_size: 1024,
        pool_capacity: 10,
        chunk_size: 1024,
    })));

    let mut tasks = Vec::new();

    for i in 0..10 {
        let buffer_clone = Arc::clone(&buffer);
        let task = tokio::spawn(async move {
            let mut buf = buffer_clone.lock().await;
            let handle = buf.acquire(512 + i * 10).await.unwrap();

            tokio::task::yield_now().await;

            assert!(handle.capacity() >= 512 + i * 10);
            buf.release(handle).await.unwrap();
        });
        tasks.push(task);
    }

    for task in tasks {
        task.await.unwrap();
    }
}

#[tokio::test]
async fn test_read_strategy() {
    let buffer = MockBuffer::new(MockBufferConfig {
        buffer_size: 1024,
        pool_capacity: 2,
        chunk_size: 512,
    });

    let strategy = buffer.read_strategy(500);
    assert_eq!(strategy, ReadStrategy::Buffer);

    let strategy = buffer.read_strategy(2048);
    assert_eq!(strategy, ReadStrategy::Chunk(512));
}

#[tokio::test]
async fn test_buffer_config_validation() {
    let configs = [
        MockBufferConfig {
            buffer_size: 1024,
            pool_capacity: 1,
            chunk_size: 512,
        },
        MockBufferConfig {
            buffer_size: 64 * 1024,
            pool_capacity: 10,
            chunk_size: 8 * 1024,
        },
        MockBufferConfig {
            buffer_size: 512,
            pool_capacity: 0,
            chunk_size: 256,
        },
    ];

    for config in configs {
        let mut buffer = MockBuffer::new(config.clone());

        let handle = buffer.acquire(config.chunk_size / 2).await.unwrap();
        assert!(handle.capacity() >= config.chunk_size / 2);
        buffer.release(handle).await.unwrap();

        let small_strategy = buffer.read_strategy(config.buffer_size / 2);
        let large_strategy = buffer.read_strategy(config.buffer_size * 2);

        assert_eq!(small_strategy, ReadStrategy::Buffer);
        assert_eq!(large_strategy, ReadStrategy::Chunk(config.chunk_size));
    }
}
