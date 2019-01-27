use failure::Error;
use mesh_msg::Core;

fn main() -> Result<(), Error> {
    let mut core = Core::new();
    let _ = core.connect("127.0.0.1:13265");

    loop {
        let frame_events = core.poll()?;
        dbg!(frame_events);
    }
}
