#![feature(lazy_cell)]

mod buffer;
pub use buffer::Buffer;

mod pixel;
pub use pixel::Pixel;

mod font;
pub use font::Font;

mod image;
pub use crate::image::{Field, Image, SubImage};

pub mod prelude;

pub type Rect = util::Rect<i32>;
