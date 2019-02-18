use bytes::IntoBuf;
use failure::Error;
use mesh_msg::{Core, FrameEvent};

use std::collections::HashMap;

fn main() -> Result<(), Error> {
    let mut core = Core::new();
    let _ = core.listen("127.0.0.1:13265");

    let mut clients: HashMap<usize, ()> = HashMap::new();

    loop {
        let frame_events = core.poll(None)?;
        for event in frame_events.into_iter() {
            use FrameEvent::*;
            match event {
                ReceivedFrames(_idx, frames) => {
                    for frame in frames.into_iter() {
                        let msg = frame.buf.into_buf();
                        for (client, _) in &clients {
                            core.write_frame(*client, msg.clone());
                        }
                    }
                }
                Accepted {
                    listen_socket: _,
                    conn_id,
                } => {
                    clients.insert(conn_id, ());
                }
                Closed(id) => {
                    clients.remove(&id);
                }
                _ => {
                    // XX TODO
                }
            }
        }
    }
}
