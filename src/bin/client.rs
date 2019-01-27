use bytes::IntoBuf;
use failure::Error;
use mesh_msg::Core;

fn main() -> Result<(), Error> {
    let mut core = Core::new();
    let idx = core.connect("127.0.0.1:13265")?;

    core.write_frame(idx, "Hello".into_buf());
    core.write_frame(idx, "World".into_buf());

    loop {
        let frame_events = core.poll(None)?;
        dbg!(frame_events);
    }
}
