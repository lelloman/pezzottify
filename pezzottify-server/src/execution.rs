//! Application imports of shared execution and I/O contracts.
pub use simple_server::primitives as sync;
pub use simple_server::primitives::{join, pin, select, try_join};
pub use simple_server::runtime::spawn;
pub use simple_server::{fs, io, process, runtime, runtime as task};
pub mod time {
    pub use simple_server::time::*;
    pub use std::time::Duration;
}

pub struct ReaderStream<R> {
    reader: R,
    capacity: usize,
    done: bool,
}
impl<R> ReaderStream<R> {
    pub fn new(reader: R) -> Self {
        Self::with_capacity(reader, 4096)
    }
    pub fn with_capacity(reader: R, capacity: usize) -> Self {
        Self {
            reader,
            capacity: capacity.max(1),
            done: false,
        }
    }
}
impl<R: simple_server::io::AsyncRead + Unpin> futures::Stream for ReaderStream<R> {
    type Item = std::io::Result<bytes::Bytes>;
    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let s = self.get_mut();
        if s.done {
            return std::task::Poll::Ready(None);
        }
        let mut buffer = vec![0; s.capacity];
        match std::task::ready!(std::pin::Pin::new(&mut s.reader).poll_read(cx, &mut buffer)) {
            Ok(0) => {
                s.done = true;
                std::task::Poll::Ready(None)
            }
            Ok(n) => {
                buffer.truncate(n);
                std::task::Poll::Ready(Some(Ok(buffer.into())))
            }
            Err(e) => {
                s.done = true;
                std::task::Poll::Ready(Some(Err(e)))
            }
        }
    }
}
