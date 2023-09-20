use gfx::Image;

use crate::prelude::*;

/// A buffer for the background of a window.
///
/// Can be used to restore the view to how it was before the window was drawn.
#[must_use]
pub struct Backdrop {
    win: Window,
    buf: Buffer,
}

impl Backdrop {
    pub fn restore(&self) {
        let image = Image::new(&self.buf, self.buf.area());
        self.win.blit([0, 0], &image);
    }
}

impl From<Window> for Backdrop {
    fn from(value: Window) -> Self {
        Backdrop {
            win: value,
            buf: value.into(),
        }
    }
}

impl Drop for Backdrop {
    fn drop(&mut self) {
        self.restore();
    }
}
