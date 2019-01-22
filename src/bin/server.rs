use mio::net::{TcpListener, TcpStream};
use mio::{Evented, Events, Poll, PollOpt, Ready, Token};

use slab::Slab;

use failure::Error;

use bytes::{Buf, BufMut, Bytes, BytesMut};

use std::io::Result as IOResult;
use std::io::{Read, Write};

use mesh_msg::framed_stream::FramedStream;

enum Conn {
    Listen(TcpListener),
    Stream(FramedStream),
}

impl Evented for Conn {
    fn register(&self, poll: &Poll, token: Token, interest: Ready, opts: PollOpt) -> IOResult<()> {
        use Conn::*;
        match self {
            Listen(conn) => conn.register(poll, token, interest, opts),
            Stream(conn) => conn.register(poll, token, interest, opts),
        }
    }
    fn reregister(
        &self,
        poll: &Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> IOResult<()> {
        use Conn::*;
        match self {
            Listen(conn) => conn.reregister(poll, token, interest, opts),
            Stream(conn) => conn.reregister(poll, token, interest, opts),
        }
    }
    fn deregister(&self, poll: &Poll) -> IOResult<()> {
        use Conn::*;
        match self {
            Listen(conn) => conn.deregister(poll),
            Stream(conn) => conn.deregister(poll),
        }
    }
}

impl Conn {
    pub fn register_and_save(self, poll: &mut Poll, slab: &mut Slab<Self>) -> IOResult<()> {
        let entry = slab.vacant_entry();
        poll.register(
            &self,
            Token(entry.key()),
            Ready::readable(),
            PollOpt::edge(),
        )?;
        entry.insert(self);
        Ok(())
    }
}

fn main() -> Result<(), Error> {
    let mut slab: Slab<Conn> = Slab::new();

    let addr = "127.0.0.1:13265".parse().unwrap();

    // Setup the server socket
    let server = Conn::Listen(TcpListener::bind(&addr).unwrap());

    // Create a poll instance
    let mut poll = Poll::new().unwrap();

    // Start listening for incoming connections
    server.register_and_save(&mut poll, &mut slab)?;
    // Create storage for events
    let mut events = Events::with_capacity(1024);

    loop {
        poll.poll(&mut events, None).unwrap();

        for event in events.iter() {
            let Token(idx) = event.token();
            match slab.get_mut(idx) {
                Some(Conn::Listen(server)) => match server.accept() {
                    Ok((stream, _client_addr)) => {
                        Conn::Stream(FramedStream::new(stream))
                            .register_and_save(&mut poll, &mut slab)
                            .expect("Register Stream");
                    }
                    Err(e) => eprintln!("Error when accepting connection: {}", e),
                },
                Some(Conn::Stream(stream)) => {
                    if event.readiness().is_readable() {
                        stream.handle_read(&mut poll);
                    }
                    if event.readiness().is_writable() {
                        stream.handle_write(&mut poll);
                    }
                }
                _ => unreachable!(),
            }
        }
    }
}
