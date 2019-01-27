use bytes::IntoBuf;
use failure::Error;
use mesh_msg::{Core, FrameEvent};

fn main() -> Result<(), Error> {
    let mut core = Core::new();
    let _ = core.listen("127.0.0.1:13265");

    loop {
        let frame_events = core.poll(None)?;
        for event in frame_events.into_iter() {
            use FrameEvent::*;
            match event {
                ReceivedFrames(idx, frames) => {
                    for frame in frames.into_iter() {
                        core.write_frame(idx, frame.buf.into_buf());
                    }
                }
                _ => {
                    // XX TODO
                }
            }
        }
    }
}
