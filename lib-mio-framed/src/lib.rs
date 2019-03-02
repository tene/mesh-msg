mod app;
mod core;
mod framed_stream;

pub use crate::app::{new_simple, App, SimpleApp, new_serde, SerdeApp, SerdeAppCore};
pub use crate::core::{Context, Core};
pub use crate::framed_stream::FramedStream;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
