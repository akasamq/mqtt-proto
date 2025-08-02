use crate::common::buffer::{Buffer, BufferHandle, MockBuffer, MockBufferConfig};

#[tokio::test]
async fn test_mock_buffer_basic() {
    let mut buffer = MockBuffer::new(MockBufferConfig::default());

    let handle = buffer.acquire(1024).await.unwrap();
    assert!(handle.capacity() >= 1024);

    buffer.release(handle).await.unwrap();
}

#[tokio::test]
async fn test_mock_buffer_large_size() {
    let mut buffer = MockBuffer::new(MockBufferConfig::default());

    let handle = buffer.acquire(16384).await.unwrap();
    assert!(handle.capacity() >= 16384);

    buffer.release(handle).await.unwrap();
}

#[tokio::test]
async fn test_mock_buffer_sequential() {
    let mut buffer = MockBuffer::new(MockBufferConfig::default());

    let mut handles = Vec::new();
    for _ in 0..10 {
        let handle = buffer.acquire(1024).await.unwrap();
        handles.push(handle);
    }

    for handle in handles {
        buffer.release(handle).await.unwrap();
    }
}
