mod app;
mod core;
mod framed_stream;

pub use crate::app::App;
pub use crate::core::{new_simple, Context, Core, SimpleApp};
pub use crate::framed_stream::FramedStream;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
