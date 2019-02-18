use mio::net::{TcpListener, TcpStream};
use mio::{Evented, Events, Poll, PollOpt, Ready, Token};
use mio_extras::channel::{channel, Receiver, Sender};

use bytes::{Buf, Bytes};
use slab::Slab;

use failure::Error;

use std::io;
use std::io::Result as IOResult;
use std::time::Duration;

use crate::{App, FramedStream};

pub struct Context;

enum ControlMsg {
    WriteFrame(usize, Box<Buf + Send>),
    /*
    Connect,
    Listen,
    Write,
    Close,
    */
}

enum Socket {
    Listen(TcpListener),
    Stream(FramedStream),
    Control(Receiver<ControlMsg>),
}

impl Evented for Socket {
    fn register(&self, poll: &Poll, token: Token, interest: Ready, opts: PollOpt) -> IOResult<()> {
        use Socket::*;
        match self {
            Listen(conn) => conn.register(poll, token, interest, opts),
            Stream(conn) => conn.register(poll, token, interest, opts),
            Control(conn) => conn.register(poll, token, interest, opts),
        }
    }
    fn reregister(
        &self,
        poll: &Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> IOResult<()> {
        use Socket::*;
        match self {
            Listen(conn) => conn.reregister(poll, token, interest, opts),
            Stream(conn) => conn.reregister(poll, token, interest, opts),
            Control(conn) => conn.reregister(poll, token, interest, opts),
        }
    }
    fn deregister(&self, poll: &Poll) -> IOResult<()> {
        use Socket::*;
        match self {
            Listen(conn) => conn.deregister(poll),
            Stream(conn) => conn.deregister(poll),
            Control(conn) => conn.deregister(poll),
        }
    }
}

impl Socket {
    pub fn framed_stream(stream: TcpStream) -> Self {
        let stream = FramedStream::new(stream);
        Socket::Stream(stream)
    }
    pub fn register_and_save(self, poll: &mut Poll, slab: &mut Slab<Self>) -> IOResult<usize> {
        let entry = slab.vacant_entry();
        poll.register(
            &self,
            Token(entry.key()),
            Ready::readable(),
            PollOpt::edge(),
        )?;
        let key = entry.key();
        entry.insert(self);
        Ok(key)
    }
}

#[derive(Debug)]
pub enum FrameEvent {
    // XXX TODO Include addresses in Listening and Accepted
    // XXX TODO Naming issue, should these be verbs, or have a noun prefix?
    Listening(usize),
    AcceptError(usize, Error),
    Accepted {
        listen_socket: usize,
        conn_id: usize,
    },
    Closed(usize),
    ReadError(usize, Error),
    ReceivedFrames(usize, Vec<Bytes>),
}

// XXX TODO NAMING wtf should I call this??
pub struct Core {
    slab: Slab<Socket>,
    poll: Poll,
    events: Events,
    frame_events: Vec<FrameEvent>,
    control_tx: Sender<ControlMsg>,
}

impl Core {
    pub fn new() -> Self {
        let mut slab: Slab<Socket> = Slab::new();
        let mut poll = Poll::new().unwrap();
        let (control_tx, control_rx) = channel();
        let _ = Socket::Control(control_rx).register_and_save(&mut poll, &mut slab);
        let events = Events::with_capacity(1024);
        let frame_events = vec![];
        Self {
            slab,
            poll,
            events,
            frame_events,
            control_tx,
        }
    }

    pub fn listen(&mut self, addr: &str) -> Result<usize, Error> {
        let addr = addr.parse()?;
        let server = Socket::Listen(TcpListener::bind(&addr)?);
        let id = server.register_and_save(&mut self.poll, &mut self.slab)?;
        self.frame_events.push(FrameEvent::Listening(id));
        Ok(id)
    }

    pub fn connect(&mut self, addr: &str) -> Result<usize, Error> {
        let addr = addr.parse()?;
        let server = Socket::framed_stream(TcpStream::connect(&addr)?);
        let id = server.register_and_save(&mut self.poll, &mut self.slab)?;
        self.frame_events.push(FrameEvent::Listening(id));
        Ok(id)
    }

    pub fn write_frame<B: Buf + Send + 'static>(&mut self, idx: usize, buf: B) {
        match self.slab.get_mut(idx) {
            Some(Socket::Listen(_)) => {
                // Should return error
            }
            Some(Socket::Stream(stream)) => {
                // Should return error
                let _ = stream.queue_write(buf, &mut self.poll, Token(idx));
            }
            Some(Socket::Control(_)) => {
                // Should return error
            }
            None => {
                // Should return error
            }
        }
    }

    pub fn close(&mut self, _idx: usize) {
        unimplemented!()
    }

    // XXX TODO This should receive a buffer to write into
    // To avoid unnecessary allocations
    pub fn poll(&mut self, timeout: Option<Duration>) -> IOResult<Vec<FrameEvent>> {
        self.poll.poll(&mut self.events, timeout)?;
        for event in self.events.iter() {
            let Token(idx) = event.token();
            let retain: bool = match self.slab.get_mut(idx) {
                Some(Socket::Listen(server)) => match server.accept() {
                    Ok((stream, _client_addr)) => {
                        let id = Socket::framed_stream(stream)
                            .register_and_save(&mut self.poll, &mut self.slab)
                            .expect("Register Stream");
                        self.frame_events.push(FrameEvent::Accepted {
                            listen_socket: idx,
                            conn_id: id,
                        });
                        true
                    }
                    Err(e) => {
                        self.frame_events
                            .push(FrameEvent::AcceptError(idx, e.into()));
                        false
                    }
                },
                Some(Socket::Stream(stream)) => {
                    let mut retain = true;
                    if event.readiness().is_readable() {
                        let (frames, rv) = stream.read_frames();
                        self.frame_events
                            .push(FrameEvent::ReceivedFrames(idx, frames));
                        if let Some(err) = rv {
                            if err.kind() != io::ErrorKind::UnexpectedEof {
                                self.frame_events
                                    .push(FrameEvent::ReadError(idx, err.into()));
                            }
                            retain = false;
                        };
                    }
                    if event.readiness().is_writable() {
                        let pre = stream.interest();
                        match stream.handle_write() {
                            Ok(()) => {}
                            Err(_err) => {
                                retain = false;
                            }
                        }
                        if pre != stream.interest() {
                            let _ = self.poll.reregister(
                                stream,
                                Token(idx),
                                stream.interest(),
                                PollOpt::edge(),
                            );
                        }
                    }
                    retain
                }
                Some(Socket::Control(ctl)) => {
                    let mut messages = vec![];
                    loop {
                        match ctl.try_recv() {
                            Ok(msg) => {
                                messages.push(msg);
                                //dbg!(msg);
                                // XXX Do I really need to use a channel
                                // XXX Can we just handle writes directly?
                                // XXX The problem is dealing with the Poll
                                // XXX Maybe just use the control socket to deliver new registrations for newly pending writes
                                // XXX Maybe I can even optimistically try acquiring the poll
                                // XXX So we don't need the message passing overhead when not needed
                            }
                            Err(e) => {
                                use std::sync::mpsc::TryRecvError::*;
                                match e {
                                    Empty => break,
                                    Disconnected => {
                                        // Should probably do something here??
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    for msg in messages {
                        match msg {
                            ControlMsg::WriteFrame(idx, buf) => {
                                if let Some(Socket::Stream(stream)) = self.slab.get_mut(idx) {
                                    let _ = stream.queue_write(buf, &mut self.poll, Token(idx));
                                }
                            }
                        }
                    }
                    true
                }
                _ => unreachable!(),
            };
            if !retain {
                self.frame_events.push(FrameEvent::Closed(idx));
                self.slab.remove(idx);
            }
        }
        Ok(self.frame_events.split_off(0))
    }

    pub fn write_handle(&self, idx: usize) -> WriteHandle {
        let sender = self.control_tx.clone();
        WriteHandle { idx, sender }
    }

    pub fn run_events<F>(&mut self, _func: F)
    where
        F: FnMut(&mut Context, &mut Vec<FrameEvent>),
    {
        unimplemented!()
    }

    pub fn run_app<A: App>(&mut self, _app: A) {
        // XXX TODO Implement by using run_events?
        unimplemented!()
    }

    pub fn run_frames<F>(&mut self, _func: F)
    where
        F: FnMut(usize, &Vec<Bytes>),
    {
        // XXX TODO Implement by using run_events?
        unimplemented!()
    }
}

pub struct WriteHandle {
    idx: usize,
    sender: Sender<ControlMsg>,
}

impl WriteHandle {
    pub fn write_frame<B: Buf + Send + 'static>(&mut self, buf: B) {
        self.sender
            .send(ControlMsg::WriteFrame(self.idx, Box::new(buf)))
            .unwrap();
    }
}
