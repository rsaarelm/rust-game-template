use navni::prelude::*;

pub use crate::{anim::Anim, Game, InputAction};

pub type Buffer = gfx::Buffer<CharCell>;
pub type Window = gfx::Window<CharCell>;
pub type Cursor<'a> = gfx::Cursor<'a, CharCell>;
pub type Rect = util::Rect<i32>;
