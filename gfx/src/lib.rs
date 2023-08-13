#![feature(lazy_cell)]

mod buffer;
pub use buffer::Buffer;

mod pixel;
pub use pixel::Pixel;

mod font;
pub use font::Font;

mod image;
pub use crate::image::{Field, Image, SubImage};

mod window;
pub use window::Window;

pub type Rect = util::Rect<i32>;

pub fn v2(a: impl Into<glam::IVec2>) -> glam::IVec2 {
    a.into()
}
