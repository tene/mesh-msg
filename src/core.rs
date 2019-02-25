use mio::net::{TcpListener, TcpStream};
use mio::{Evented, Events, Poll, PollOpt, Ready, Token};
use mio_extras::channel::{channel, Receiver, Sender};

use bytes::{Buf, Bytes};
use slab::Slab;

use failure::Error;

use std::collections::HashMap;
use std::io;
use std::io::Result as IOResult;

use crate::{App, FramedStream};

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

// XXX TODO NAMING wtf should I call this??
pub struct Core<A: App> {
    // XXX TODO Maybe someday for optimization
    // XXX TODO Move slab, poll into Context/Inner
    // XXX TODO split slab into read/write pair (Slab/Map?)<Arc<Mutex<Socket>>>
    //          only write half goes in Context/Inner?
    app: A,
    slab: Slab<Socket>,
    ctx: Context,
    poll: Poll,
    events: Events,
    control_tx: Sender<ControlMsg>,
}

impl<A: App> Core<A> {
    pub fn new(app: A) -> Self {
        let mut slab: Slab<Socket> = Slab::new();
        let mut poll = Poll::new().unwrap();
        let (control_tx, control_rx) = channel();
        let ctx = Context::new(control_tx.clone());
        let _ = Socket::Control(control_rx).register_and_save(&mut poll, &mut slab);
        let events = Events::with_capacity(1024);
        Self {
            app,
            slab,
            ctx,
            poll,
            events,
            control_tx,
        }
    }

    // XXX TODO move to Context/Inner
    pub fn listen(&mut self, addr: &str) -> Result<usize, Error> {
        let addr = addr.parse()?;
        let server = Socket::Listen(TcpListener::bind(&addr)?);
        let id = server.register_and_save(&mut self.poll, &mut self.slab)?;
        self.ctx.listening(id);
        self.app.handle_listen(&self.ctx, id);
        Ok(id)
    }

    // XXX TODO move to Context/Inner
    pub fn connect(&mut self, addr: &str) -> Result<usize, Error> {
        let addr = addr.parse()?;
        let server = Socket::framed_stream(TcpStream::connect(&addr)?);
        let id = server.register_and_save(&mut self.poll, &mut self.slab)?;
        self.ctx.connected(id);
        self.app.handle_connect(&self.ctx, id);
        Ok(id)
    }

    // XXX TODO move to Context/Inner
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

    pub fn run(&mut self) -> IOResult<()> {
        loop {
            self.poll.poll(&mut self.events, None)?;
            for event in self.events.iter() {
                let Token(idx) = event.token();
                let retain: bool = match self.slab.get_mut(idx) {
                    Some(Socket::Listen(server)) => match server.accept() {
                        Ok((stream, _client_addr)) => {
                            let conn_id = Socket::framed_stream(stream)
                                .register_and_save(&mut self.poll, &mut self.slab)
                                .expect("Register Stream");
                            self.ctx.accepted(conn_id);
                            self.app.handle_accept(&self.ctx, idx, conn_id);
                            true
                        }
                        Err(_e) => {
                            // XXX app.handle_accept_error(&self.ctx, idx, e.into());
                            false
                        }
                    },
                    Some(Socket::Stream(stream)) => {
                        let mut retain = true;
                        if event.readiness().is_readable() {
                            let (frames, rv) = stream.read_frames();
                            if frames.len() > 0 {
                                self.app.handle_frames(&self.ctx, idx, frames);
                            }
                            if let Some(err) = rv {
                                if err.kind() != io::ErrorKind::UnexpectedEof {
                                    // XXX app.handle_read_err(&self.ctx, idx, err.into());
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
                    self.ctx.closed(idx);
                    self.app.handle_close(&self.ctx, idx);
                    self.slab.remove(idx);
                }
            }
        }
    }

    pub fn write_handle(&self, idx: usize) -> WriteHandle {
        let sender = self.control_tx.clone();
        WriteHandle { idx, sender }
    }
}

pub struct Context {
    connections: HashMap<usize, ConnectionDetails>,
    listening: HashMap<usize, ListenDetails>,
    sender: Sender<ControlMsg>,
}

impl Context {
    fn new(sender: Sender<ControlMsg>) -> Self {
        let connections = HashMap::new();
        let listening = HashMap::new();
        Self {
            connections,
            listening,
            sender,
        }
    }
    fn connected(&mut self, id: usize) {
        self.connections.insert(id, ConnectionDetails::new());
    }
    fn accepted(&mut self, id: usize) {
        self.connections.insert(id, ConnectionDetails::new());
    }
    fn closed(&mut self, id: usize) -> Option<ConnectionDetails> {
        self.connections.remove(&id)
    }
    fn listening(&mut self, id: usize) {
        self.listening.insert(id, ListenDetails::new());
    }
    pub fn connection_ids<'a>(&'a self) -> impl Iterator<Item = usize> + 'a {
        self.connections.keys().map(|id| *id)
    }
    pub fn write_frame<B: Buf + Send + 'static>(&self, id: usize, buf: B) {
        self.sender
            .send(ControlMsg::WriteFrame(id, Box::new(buf)))
            .unwrap();
    }
}

pub struct ConnectionDetails;

impl ConnectionDetails {
    pub fn new() -> Self {
        ConnectionDetails
    }
}

pub struct ListenDetails;

impl ListenDetails {
    pub fn new() -> Self {
        ListenDetails
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

pub struct SimpleApp<F2: FnMut(&Context, usize, Vec<Bytes>)>(F2);

impl<F2> App for SimpleApp<F2>
where
    F2: FnMut(&Context, usize, Vec<Bytes>),
{
    fn handle_frames(&mut self, ctx: &Context, id: usize, frames: Vec<Bytes>) {
        self.0(ctx, id, frames);
    }
}

pub fn new_simple<F>(func: F) -> Core<SimpleApp<F>>
where
    F: FnMut(&Context, usize, Vec<Bytes>),
{
    Core::new(SimpleApp(func))
}
