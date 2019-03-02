use mio::{Evented, Poll, PollOpt, Ready, Token};

use bytes::{Buf, BufMut, Bytes, BytesMut};

use std::io::Result as IOResult;
use std::io::{Error, ErrorKind, Read, Write};

pub trait Stream: Read + Write + Evented + Send {}
impl<T> Stream for T where T: Read + Write + Evented + Send {}

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
    write_buf: BytesMut,
    interest: Ready,
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
        let interest = Ready::readable();
        let write_buf = BytesMut::with_capacity(8192);
        let stream = Box::new(stream);
        let read_buf = BytesMut::with_capacity(8192);
        FramedStream {
            stream,
            read_buf,
            write_buf,
            interest,
        }
    }
    pub fn interest(&self) -> Ready {
        self.interest
    }
    fn ensure_read_buf_capacity(&mut self) {
        let buf = &mut self.read_buf;
        let buf_len = buf.len();
        let buf_capacity = buf.capacity();
        if buf.len() > 2 {
            let msg_size = u16::from_le_bytes([buf[0], buf[1]]) as usize;
            let buf_msg_len = buf_len - 2;
            if msg_size > buf_msg_len + buf_capacity {
                buf.reserve(msg_size - buf_msg_len);
            }
        } else if buf_capacity < 1024 {
            buf.reserve(4096);
        }
    }
    pub fn read_frames(&mut self) -> (Vec<Bytes>, Option<Error>) {
        let mut frames: Vec<Bytes> = vec![];
        let mut err: Option<Error> = None;
        'outer: loop {
            self.ensure_read_buf_capacity();
            let buf = &mut self.read_buf;
            match self.stream.read_buf(buf) {
                Ok(0) => {
                    err = Some(Error::new(ErrorKind::UnexpectedEof, "Connection Closed"));
                    break 'outer;
                }
                Err(e) => {
                    if e.kind() != ErrorKind::WouldBlock {
                        err = Some(e);
                    }
                    break 'outer;
                }
                Ok(_n) => {
                    // Successful read
                }
            }
            'inner: while buf.len() > 2 {
                let msg_size = u16::from_le_bytes([buf[0], buf[1]]) as usize;
                let buf_msg_len = buf.len() - 2;
                if msg_size <= buf_msg_len {
                    buf.advance(2);
                    let frame_bytes = buf.split_to(msg_size).freeze();
                    frames.push(frame_bytes);
                } else {
                    break 'inner;
                }
            }
        }
        return (frames, err);
    }

    pub fn queue_write<B: Buf + Send + 'static>(
        &mut self,
        buf: B,
        poll: &mut Poll,
        token: Token,
    ) -> IOResult<()> {
        // XXX TODO Optimistically attempt writing immediately?
        let msg_size = buf.remaining();
        if msg_size > std::u16::MAX as usize {
            return Err(Error::new(ErrorKind::InvalidData, "Message too big"));
        }
        if self.write_buf.capacity() < msg_size + 2 {
            self.write_buf.reserve(msg_size + 2);
        }
        self.write_buf.put_u16_le(msg_size as u16);
        self.write_buf.put(buf);

        if !self.interest.is_writable() {
            self.interest.insert(Ready::writable());
            poll.reregister(self, token, self.interest(), PollOpt::edge())?;
        }
        Ok(())
    }

    pub fn handle_write(&mut self) -> IOResult<usize> {
        // XXX TODO use vectored writes?
        // Not quite sure how to populate the array of iovecs
        // https://docs.rs/mio/0.6.16/mio/net/struct.TcpStream.html#method.read_bufs
        // https://docs.rs/bytes/0.4.11/bytes/trait.Buf.html#method.bytes_vec

        let mut count = 0;
        loop {
            if self.write_buf.is_empty() {
                break;
            }
            match self.stream.write(&self.write_buf) {
                Ok(0) => {
                    // XXX TODO When precisely will this happen?
                    break;
                }
                Err(e) => {
                    if e.kind() == ErrorKind::WouldBlock {
                        break;
                    }
                    return Err(e);
                }
                Ok(n) => {
                    self.write_buf.advance(n);
                    count += n;
                    // Successful read
                }
            }
        }
        if self.write_buf.is_empty() {
            self.interest.remove(Ready::writable());
        } else {
            // Pending unhandled writes remain
        }
        Ok(count)
    }
}
