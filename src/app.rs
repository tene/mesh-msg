use crate::{Context, Core};
use bytes::Bytes;

use std::marker::PhantomData;

pub trait App {
    fn handle_init(&mut self, _ctx: &Context) {}
    fn handle_listen(&mut self, _ctx: &Context, _id: usize) {}
    // XXX also include connectiondetails
    fn handle_connect(&mut self, _ctx: &Context, _id: usize) {}
    fn handle_accept(&mut self, _ctx: &Context, _listen_socket: usize, _id: usize) {}
    fn handle_close(&mut self, _ctx: &Context, _id: usize) {}
    fn handle_frames(&mut self, _ctx: &Context, _id: usize, _frames: Vec<Bytes>) {}
    fn handle_shutdown(&mut self) {}
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

pub struct SerdeContext<'a, Tx> {
    _ctx: &'a Context,
    _phantom: PhantomData<Tx>,
}

pub trait SerdeApp {
    type Rx;
    type Tx;
    fn handle_init(&mut self, _ctx: &SerdeContext<Self::Tx>) {}
    fn handle_listen(&mut self, _ctx: &SerdeContext<Self::Tx>, _id: usize) {}
    // XXX also include connectiondetails
    fn handle_connect(&mut self, _ctx: &SerdeContext<Self::Tx>, _id: usize) {}
    fn handle_accept(&mut self, _ctx: &SerdeContext<Self::Tx>, _listen_socket: usize, _id: usize) {}
    fn handle_close(&mut self, _ctx: &SerdeContext<Self::Tx>, _id: usize) {}
    fn handle_items(&mut self, _ctx: &SerdeContext<Self::Tx>, _id: usize, _items: Vec<Self::Rx>) {}
    fn handle_shutdown(&mut self) {}
}

pub struct SerdeAppCore<A: SerdeApp> {
    app: A,
}

impl<A: SerdeApp> SerdeAppCore<A> {
    fn wrap_context<'a>(&self, ctx: &'a Context) -> SerdeContext<'a, A::Tx> {
        SerdeContext {
            _ctx: ctx,
            _phantom: PhantomData,
        }
    }
}

pub fn new_serde<Rx: Sized, Tx: Sized, A: SerdeApp<Rx = Rx, Tx = Tx>>(app: A) -> SerdeAppCore<A> {
    SerdeAppCore { app }
}

impl<A: SerdeApp> App for SerdeAppCore<A> where {
    fn handle_init(&mut self, ctx: &Context) {
        let ctx = self.wrap_context(ctx);
        self.app.handle_init(&ctx)
    }
    fn handle_listen(&mut self, ctx: &Context, id: usize) {
        let ctx = self.wrap_context(ctx);
        self.app.handle_listen(&ctx, id)
    }
    // XXX also include connectiondetails
    fn handle_connect(&mut self, ctx: &Context, id: usize) {
        let ctx = self.wrap_context(ctx);
        self.app.handle_connect(&ctx, id)
    }
    fn handle_accept(&mut self, ctx: &Context, listen_socket: usize, id: usize) {
        let ctx = self.wrap_context(ctx);
        self.app.handle_accept(&ctx, listen_socket, id)
    }
    fn handle_close(&mut self, ctx: &Context, id: usize) {
        let ctx = self.wrap_context(ctx);
        self.app.handle_close(&ctx, id)
    }
    fn handle_frames(&mut self, ctx: &Context, _id: usize, _frames: Vec<Bytes>) {
        let _ctx = self.wrap_context(ctx);
        unimplemented!()
    }
    fn handle_shutdown(&mut self) {
        self.app.handle_shutdown()
    }
}
