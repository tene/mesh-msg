use bytes::{Bytes, IntoBuf};
use mesh_msg::{App, Context, Core};
use std::io;

struct BroadcastServer {}

impl App for BroadcastServer {
    fn handle_frames(&mut self, ctx: &Context, _id: usize, frames: Vec<Bytes>) {
        for frame in frames.into_iter() {
            let msg = frame.into_buf();
            for conn in ctx.connection_ids() {
                ctx.write_frame(conn, msg.clone());
            }
        }
    }
}

fn main() -> io::Result<()> {
    let mut core = Core::new();
    let _ = core.listen("127.0.0.1:13265");

    core.run_app(BroadcastServer {})
}
