use failure::Error;
use mesh_msg::new_simple;

use crossbeam::thread;
use linefeed::{Interface, ReadResult};

use std::sync::Arc;

fn main() -> Result<(), Error> {
    thread::scope(|s| {
        let reader = Arc::new(Interface::new("my-application").unwrap());

        reader.set_prompt("my-app> ").unwrap();

        let iface = reader.clone();

        let mut core = new_simple(move |_ctx, _id, frames| {
            if frames.len() > 0 {
                writeln!(iface, "{:?}", frames).unwrap();
            }
        });

        let idx = core.connect("127.0.0.1:13265").unwrap();
        let mut write_handle = core.write_handle(idx);

        write_handle.write_frame("Hello");
        write_handle.write_frame("World");

        s.spawn(move |_| {
            core.run().unwrap();
        });
        while let ReadResult::Input(input) = reader.read_line().unwrap() {
            write_handle.write_frame(input);
        }
        // XXX TODO Need a way to close the core, to terminate the thread
    })
    .unwrap();

    println!("Goodbye.");

    /*
     */
    Ok(())
}
