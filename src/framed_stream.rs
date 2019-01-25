use mio::{Evented, Poll, PollOpt, Ready, Token};

use failure::Error;

use bytes::{Buf, BufMut, Bytes, BytesMut, IntoBuf};

use std::fmt::Debug;
use std::io::Result as IOResult;
use std::io::{Read, Write};

#[derive(Debug)]
pub struct Frame {
    pub len: usize,
    pub buf: Bytes,
}

impl Frame {
    pub fn new(len: usize, buf: Bytes) -> Self {
        Self { len, buf }
    }
}

pub trait Stream: Read + Write + Evented + Debug {}
impl<T> Stream for T where T: Read + Write + Evented + Debug {}

impl Stream {
    pub fn read_buf<B: BufMut>(&mut self, buf: &mut B) -> IOResult<usize> {
        unsafe {
            let b = buf.bytes_mut();
            let rv = self.read(b)?;
            buf.advance_mut(rv);
            Ok(rv)
        }
    }
    pub fn try_read(&mut self, buf: &mut [u8]) -> IOResult<Option<usize>> {
        use std::io::ErrorKind::WouldBlock;
        match self.read(buf) {
            Ok(len) => Ok(Some(len)),
            Err(err) => {
                if let WouldBlock = err.kind() {
                    Ok(None)
                } else {
                    Err(err)
                }
            }
        }
    }

    pub fn try_read_frame(&mut self, buf: &mut BytesMut) -> IOResult<Option<Frame>> {
        // When we receive multiple frames per read,
        // process them all before reading again
        let should_read = if buf.is_empty() || buf.len() < 2 {
            if buf.capacity() < 1024 {
                buf.reserve(4096);
            }
            true
        } else {
            let msg_size = u16::from_le_bytes([buf[0], buf[1]]) as usize;
            let buf_msg_len = buf.len() - 2;
            if msg_size <= buf_msg_len {
                false
            } else {
                if msg_size > buf_msg_len + buf.capacity() {
                    buf.reserve(msg_size - buf_msg_len);
                }
                true
            }
        };
        if should_read {
            let read_bytes = self.read_buf(buf)?;
            if read_bytes == 0 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "Connection Closed",
                ));
            }
            if read_bytes < 2 {
                return Ok(None);
            }
        };
        let msg_size = u16::from_le_bytes([buf[0], buf[1]]) as usize;
        let buf_msg_len = buf.len() - 2;
        if msg_size <= buf_msg_len {
            buf.advance(2);
            let frame_buf = buf.split_to(msg_size).freeze();
            let frame = Frame::new(msg_size, frame_buf);
            Ok(Some(frame))
        } else {
            Ok(None)
        }
    }

    pub fn try_write(&mut self, buf: &[u8]) -> IOResult<Option<usize>> {
        use std::io::ErrorKind::WouldBlock;
        match self.write(buf) {
            Ok(len) => Ok(Some(len)),
            Err(err) => {
                if let WouldBlock = err.kind() {
                    Ok(None)
                } else {
                    Err(err)
                }
            }
        }
    }
}

pub struct FramedStream {
    stream: Box<Stream>,
    read_buf: BytesMut,
}

impl Evented for FramedStream {
    fn register(&self, poll: &Poll, token: Token, interest: Ready, opts: PollOpt) -> IOResult<()> {
        self.stream.register(poll, token, interest, opts)
    }
    fn reregister(
        &self,
        poll: &Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> IOResult<()> {
        self.stream.reregister(poll, token, interest, opts)
    }
    fn deregister(&self, poll: &Poll) -> IOResult<()> {
        self.stream.deregister(poll)
    }
}

impl FramedStream {
    pub fn new<S: Stream + 'static>(stream: S) -> Self {
        FramedStream {
            stream: Box::new(stream),
            read_buf: BytesMut::with_capacity(8192),
        }
    }
    pub fn handle_read(&mut self, poll: &mut Poll) -> (Vec<Frame>, IOResult<()>) {
        let mut frames = vec![];
        loop {
            match self.stream.try_read_frame(&mut self.read_buf) {
                Ok(Some(frame)) => {
                    frames.push(frame);
                }
                Ok(None) => {
                    continue;
                }
                Err(err) => match err.kind() {
                    std::io::ErrorKind::WouldBlock => return (frames, Ok(())),
                    _ => return (frames, Err(err)),
                },
            }
        }
    }
    pub fn handle_write(&mut self, poll: &mut Poll) -> IOResult<()> {
        unimplemented!()
    }
}
