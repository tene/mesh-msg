use mio::{Evented, Poll, PollOpt, Ready, Token};

use failure::Error;

use bytes::{Buf, BufMut, Bytes, BytesMut, IntoBuf};

use std::io::Result as IOResult;
use std::io::{Read, Write};

pub trait Stream: Read + Write + Evented {}
impl<T> Stream for T where T: Read + Write + Evented {}

impl Stream {
    pub fn try_read_buf<B: BufMut>(&mut self, buf: &mut B) -> IOResult<Option<usize>> {
        unsafe {
            let b = buf.bytes_mut();
            let rv = self.try_read(b)?;
            if let Some(n) = rv {
                buf.advance_mut(n);
            }
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

enum FrameState {
    NewFrame,
    InProgress { size: usize, pending: usize },
}

impl FrameState {
    pub fn new() -> Self {
        FrameState::NewFrame
    }
}

pub struct FramedStream {
    stream: Box<Stream>,
    read_state: FrameState,
    write_state: FrameState,
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
            read_state: FrameState::new(),
            write_state: FrameState::new(),
            read_buf: BytesMut::with_capacity(8192),
        }
    }
    pub fn handle_read(&mut self, poll: &mut Poll) -> IOResult<()> {
        while let Some(mut read_count) = self.stream.try_read_buf(&mut self.read_buf)? {
            loop {
                match self.read_state {
                    FrameState::NewFrame => {
                        let size =
                            self.read_buf.split_to(2).freeze().into_buf().get_u16_le() as usize;
                        read_count -= 2;
                        if size >= read_count {
                            let pending = size - read_count;
                            self.read_state = FrameState::InProgress { size, pending };
                            if pending > self.read_buf.capacity() {
                                self.read_buf.reserve(pending);
                            }
                            break;
                        } else {
                            self.read_state = FrameState::InProgress { size, pending: 0 };
                            continue;
                            // XXX TODO what to do with read_count here?  This is a mess.
                        }
                    }
                    FrameState::InProgress { size, pending } => {}
                }
            }
        }
        unimplemented!()
    }
    pub fn handle_write(&mut self, poll: &mut Poll) -> IOResult<()> {
        unimplemented!()
    }
}
