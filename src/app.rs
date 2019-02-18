use crate::Context;
use bytes::Bytes;

pub trait App {
    fn handle_init(&mut self, _ctx: &Context) {}
    fn handle_listen(&mut self, _ctx: &Context, _id: usize) {}
    fn handle_accept(&mut self, _ctx: &Context, _listen_socket: usize, _id: usize) {}
    fn handle_close(&mut self, _ctx: &Context, _id: usize) {}
    fn handle_frames(&mut self, _ctx: &Context, _id: usize, _frames: Vec<Bytes>) {}
    fn handle_shutdown(&mut self) {}
}
