mod core;
mod framed_stream;

pub use crate::core::Core;
pub use crate::framed_stream::{Frame, FramedStream};

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
