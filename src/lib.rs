pub mod crypt;

mod multiplex;
mod timer;
pub use multiplex::*;
mod pipe;

mod utilities;

pub use pipe::client::connect;
pub use pipe::listener::Listener;
pub use pipe::*;
// pub(crate) type SVec<T> = SmallVec<[T; 16]>;
