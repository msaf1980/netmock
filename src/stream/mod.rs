//! A fake stream for testing network applications backed by buffers.
#![warn(missing_docs)]

use std::collections::VecDeque;
use std::io::{self, Error, Read, Write};
use std::ops::Deref;
use std::sync::Arc;
use std::time::Duration;

#[cfg(feature = "tokio")]
use std::pin::Pin;

#[cfg(feature = "tokio")]
use std::task::{self, Poll};

#[cfg(feature = "tokio")]
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

#[cfg(feature = "tokio")]
use tokio::time::{sleep_until, Instant, Sleep};

#[cfg(feature = "tokio")]
use futures_core::{ready, Future};

/// A fake stream for testing network applications backed by unchecked read/write buffers.
#[derive(Clone, Debug)]
pub struct SimpleMockStream {
    written: Vec<u8>,
    read: Vec<u8>,
    pos: usize,
}

impl SimpleMockStream {
    /// Creates a new mock stream with nothing to read.
    pub fn empty() -> SimpleMockStream {
        SimpleMockStream::new(vec![])
    }

    /// Creates a new mock stream with the specified bytes to read.
    pub fn new(initial: Vec<u8>) -> SimpleMockStream {
        SimpleMockStream {
            written: vec![],
            read: initial,
            pos: 0,
        }
    }

    /// Creates a new mock stream with the specified bytes to read and initial written buffer capacity.
    pub fn with_capacity(initial: Vec<u8>, capacity: usize) -> SimpleMockStream {
        SimpleMockStream {
            written: Vec::with_capacity(capacity),
            read: initial,
            pos: 0,
        }
    }

    /// Resets stream.
    pub fn reset(&mut self) {
        self.reset_actions();
        self.reset_written();
    }

    /// Resets stream (but preserve already written).
    pub fn reset_actions(&mut self) {
        self.pos = 0;
    }

    /// Resets written buffer.
    pub fn reset_written(&mut self) {
        self.written.clear();
    }

    /// Gets a slice of bytes representing the data that has been written.
    pub fn written(&self) -> &[u8] {
        &self.written
    }

    /// Gets a slice of bytes representing the all data that has been put to read.
    pub fn readed(&self) -> &[u8] {
        &self.read
    }

    /// Gets a slice of bytes representing the data that has been put to read.
    pub fn remaining(&self) -> &[u8] {
        &self.read[self.pos..]
    }
}

impl Read for SimpleMockStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.read.len() == self.pos || buf.len() == 0 {
            Ok(0)
        } else {
            let len = std::cmp::min(self.remaining().len(), buf.len());
            let end = len + self.pos;
            buf[..len].copy_from_slice(&self.read[self.pos..end]);
            self.pos = end;
            Ok(len)
        }
    }
}

impl Write for SimpleMockStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.written.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.written.flush()
    }
}

#[cfg(feature = "tokio")]
impl AsyncRead for SimpleMockStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        _: &mut task::Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        if self.pos < self.read.len() {
            let len = std::cmp::min(self.remaining().len(), buf.remaining());
            let end = len + self.pos;
            buf.put_slice(&self.read[self.pos..end]);
            self.pos = end;
        }
        Poll::Ready(Ok(()))
    }
}

#[cfg(feature = "tokio")]
impl AsyncWrite for SimpleMockStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        _: &mut task::Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        match self.written.write_all(buf) {
            Ok(_) => Poll::Ready(Ok(buf.len())),
            Err(err) => Poll::Ready(Err(err)),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, _: &mut task::Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _: &mut task::Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

#[derive(Debug, Clone)]
enum Action {
    Read(Vec<u8>), // return on read
    ReadError(Arc<Error>),
    Write(Vec<u8>), // check write
    WriteError(Arc<Error>),
    Wait(Duration),
}

/// A builder for [`CheckedMockStream`]
#[derive(Debug, Clone, Default)]
pub struct CheckedMockStreamBuilder {
    actions: VecDeque<Action>,
    writed: usize,
}

impl CheckedMockStreamBuilder {
    /// Create a new empty [`CheckedMockStreamBuilder`]
    pub fn new() -> Self {
        CheckedMockStreamBuilder::default()
    }

    /// Queue an item to be returned by the stream read
    pub fn read(mut self, value: Vec<u8>) -> Self {
        self.actions.push_back(Action::Read(value));
        self
    }

    /// Queue an error to be returned by the stream read
    pub fn read_error(mut self, err: Error) -> Self {
        self.actions.push_back(Action::ReadError(Arc::new(err)));
        self
    }

    /// Queue an item to be required to be written to the stream
    pub fn write(mut self, want: Vec<u8>) -> Self {
        self.writed += want.len();
        self.actions.push_back(Action::Write(want));
        self
    }

    /// Queue an error to be returned by the stream write
    pub fn write_error(mut self, err: Error) -> Self {
        self.actions.push_back(Action::WriteError(Arc::new(err)));
        self
    }

    /// Queue the stream to wait for a duration
    pub fn wait(mut self, duration: Duration) -> Self {
        self.actions.push_back(Action::Wait(duration));
        self
    }

    /// Build the [`CheckedMockStream`]
    pub fn build(self) -> CheckedMockStream {
        CheckedMockStream {
            actions: self.actions.into(),
            written: Vec::new(),
            action: 0,
            pos: 0,
            #[cfg(feature = "tokio")]
            sleep: None,
        }
    }

    /// Build the [`CheckedMockStream`] with preallocated writted buffer (for all wanted writes)
    pub fn build_cap(self) -> CheckedMockStream {
        CheckedMockStream {
            actions: self.actions.into(),
            written: Vec::with_capacity(self.writed),
            action: 0,
            pos: 0,
            #[cfg(feature = "tokio")]
            sleep: None,
        }
    }
}

/// A fake stream for testing network applications backed by read/write (checked) buffers.
///
/// See [`CheckedMockStreamBuilder`] for more information.
#[derive(Debug)]
pub struct CheckedMockStream {
    actions: Vec<Action>,
    written: Vec<u8>,
    action: usize,
    pos: usize,
    #[cfg(feature = "tokio")]
    sleep: Option<Pin<Box<Sleep>>>,
}

impl CheckedMockStream {
    /// Resets stream
    pub fn reset(&mut self) {
        self.reset_actions();
        self.reset_written();
    }

    /// Resets stream (but preserve already written).
    pub fn reset_actions(&mut self) {
        self.action = 0;
        self.pos = 0;
    }

    /// Seek to action for stream.
    pub fn seek_action(&mut self, action: usize) {
        self.action = action;
        self.pos = 0;
    }

    /// Resets written buffer.
    pub fn reset_written(&mut self) {
        self.written.clear();
    }

    /// Gets a slice of bytes representing the data that has been written.
    pub fn written(&self) -> &[u8] {
        &self.written
    }
}

impl Read for CheckedMockStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.action >= self.actions.len() || buf.len() == 0 {
            return Ok(0);
        }
        match &self.actions[self.action] {
            Action::ReadError(err) => {
                self.action += 1;
                Err(Error::new(err.kind(), err.to_string()))
            },
            Action::Read(data) => {
                let len = std::cmp::min(data.len() - self.pos, buf.len());
                let end = len + self.pos;
                buf[..len].copy_from_slice(&data[self.pos..end]);
                if end == data.len() {
                    self.action += 1;
                    self.pos = 0;
                } else {
                    self.pos = end;
                }
                Ok(len)
            }
            Action::Wait(wait) => {
                std::thread::sleep(*wait);
                self.action += 1;
                self.read(buf)
            }
            _ => Ok(0),
        }
    }
}

impl Write for CheckedMockStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.action >= self.actions.len() || buf.len() == 0 {
            return Ok(0);
        }
        match &self.actions[self.action] {
            Action::WriteError(err) => {
                self.action += 1;
                Err(Error::new(err.kind(), err.to_string()))
            },
            Action::Write(data) => {
                if data == buf {
                    match self.written.write(buf) {
                        Ok(written) => {
                            self.action += 1;
                            Ok(written)
                        }
                        Err(err) => Err(err),
                    }
                } else if data.len() < buf.len() && data == &buf[..data.len()] {
                    match self.written.write(&buf[..data.len()]) {
                        Ok(written) => {
                            self.action += 1;
                            Ok(written)
                        }
                        Err(err) => Err(err),
                    }
                } else {
                    Err(Error::new(
                        io::ErrorKind::InvalidInput,
                        "mismatch written data",
                    ))
                }
            }
            Action::Wait(wait) => {
                std::thread::sleep(*wait);
                self.action += 1;
                self.write(buf)
            }
            _ => Ok(0),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        self.written.flush()
    }
}

#[cfg(feature = "tokio")]
impl AsyncRead for CheckedMockStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        if let Some(ref mut sleep) = self.sleep {
            ready!(Pin::new(sleep).poll(cx));
            self.sleep = None;
        }

        if self.action >= self.actions.len() || buf.remaining() == 0 {
            return Poll::Ready(Ok(()));
        }
        let result: io::Result<()>;
        match &self.actions[self.action] {
            Action::ReadError(err) => {
                result = Err(Error::new(err.kind(), err.to_string()));
            }
            Action::Read(data) => {
                let len = std::cmp::min(data.len() - self.pos, buf.remaining());
                let end = len + self.pos;
                // buf[..len].copy_from_slice(&data[self.pos..end]);
                buf.put_slice(&data[self.pos..end]);
                if end == data.len() {
                    self.action += 1;
                    self.pos = 0;
                } else {
                    self.pos = end;
                }
                return Poll::Ready(Ok(()));
            }
            Action::Wait(wait) => {
                self.sleep = Some(Box::pin(sleep_until(Instant::now() + *wait)));
                cx.waker().wake_by_ref();
                self.action += 1;

                return Poll::Pending;
            }
            _ => return Poll::Ready(Ok(())),
        }

        self.action += 1;
        Poll::Ready(result)
    }
}

#[cfg(feature = "tokio")]
impl AsyncWrite for CheckedMockStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        if let Some(ref mut sleep) = self.sleep {
            ready!(Pin::new(sleep).poll(cx));
            self.sleep = None;
        }

        if self.action >= self.actions.len() || buf.len() == 0 {
            return Poll::Ready(Ok(0));
        }
        let result: io::Result<usize>;
        match &self.actions[self.action] {
            Action::WriteError(err) => {
                result = Err(Error::new(err.kind(), err.to_string()))
            }
            Action::Write(data) => {
                let len: usize;
                if data == buf {
                    len = buf.len();
                } else if data.len() < buf.len() && data == &buf[..data.len()] {
                    len = data.len();
                } else {
                    return Poll::Ready(Err(Error::new(
                        io::ErrorKind::InvalidInput,
                        "mismatch written data",
                    )));
                }

                match self.written.write_all(&buf[..len]) {
                    Ok(_) => {
                        result = Ok(len);
                    }
                    Err(err) => {
                        return Poll::Ready(Err(err))
                    }
                }
            }
            Action::Wait(wait) => {
                self.sleep = Some(Box::pin(sleep_until(Instant::now() + *wait)));
                cx.waker().wake_by_ref();

                self.action += 1;

                return Poll::Pending;
            }
            _ => {
                return Poll::Ready(Ok(0))
            }
        }

        self.action += 1;
        Poll::Ready(result)
    }

    fn poll_flush(self: Pin<&mut Self>, _: &mut task::Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _: &mut task::Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

#[cfg(test)]
mod tests_sync;

#[cfg(feature = "tokio")]
#[cfg(test)]
mod tests_tokio;
