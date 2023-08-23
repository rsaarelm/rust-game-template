use std::fmt::{self, Write};

use glam::IVec2;
use navni::prelude::*;
use util::{v2, write};

use crate::prelude::*;

pub struct Cursor<'a, P: Pixel> {
    c: &'a mut Buffer<P>,
    win: Window<P>,
    pos: IVec2,
}

impl<'a, P: Pixel> Cursor<'a, P> {
    pub fn new(buffer: &'a mut Buffer<P>, win: Window<P>) -> Self {
        Cursor {
            c: buffer,
            win,
            pos: Default::default(),
        }
    }

    pub fn print_button(&mut self, mouse: &MouseState, text: &str) -> bool {
        debug_assert!(
            !text.chars().any(|c| c == '\n'),
            "print_button: only single line supported"
        );
        let w = text.chars().count() as i32;
        // Move to next line if line would go past right edge.
        if self.win.width() - self.pos.x < w {
            self.pos.x = 0;
            self.pos.y += 1;
        }
        let bounds = Rect::new(self.pos, self.pos + v2([w, 1]));
        // TODO: Make the colors different if hovering (bold) or pressed
        // (invert) with the mouse.
        write!(self, "{}", text);
        self.win.clicked_on(mouse, &bounds)
    }
}

impl<'a, P: Pixel> fmt::Write for Cursor<'a, P> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for (i, line) in s.lines().enumerate() {
            // Newlines
            if i > 0 {
                self.pos.y += 1;
                self.pos.x = 0;
            }
            self.pos = self.win.write(self.c, self.pos, line);
        }
        // .lines() doesn't catch the final newline.
        if s.ends_with('\n') {
            self.pos.y += 1;
            self.pos.x = 0;
        }
        Ok(())
    }
}
