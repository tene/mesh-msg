use bytes::IntoBuf;
use failure::Error;
use mesh_msg::Core;

use crossbeam::thread;
use linefeed::{Interface, ReadResult};

use std::sync::Arc;

fn main() -> Result<(), Error> {
    thread::scope(|s| {
        let mut core = Core::new();
        let idx = core.connect("127.0.0.1:13265").unwrap();
        let mut write_handle = core.write_handle(idx);

        write_handle.write_frame("Hello".into_buf());
        write_handle.write_frame("World".into_buf());

        let reader = Arc::new(Interface::new("my-application").unwrap());

        reader.set_prompt("my-app> ").unwrap();

        let iface = reader.clone();
        s.spawn(move |_| loop {
            core.run_frames(|_ctx, _id, frames| {
                if frames.len() > 0 {
                    writeln!(iface, "{:?}", frames).unwrap();
                }
            }).unwrap();
        });
        while let ReadResult::Input(input) = reader.read_line().unwrap() {
            write_handle.write_frame(input.into_buf());
        }
        // XXX TODO Need a way to close the core, to terminate the thread
    })
    .unwrap();

    println!("Goodbye.");

    /*
     */
    Ok(())
}
