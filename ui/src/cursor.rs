use std::fmt;

use gfx::Rect;
use glam::IVec2;
use util::v2;

use crate::Window;

pub struct Cursor {
    win: Window,
    pub pos: IVec2,
}

impl Cursor {
    pub fn new(win: Window) -> Self {
        Cursor {
            win,
            pos: Default::default(),
        }
    }

    pub fn print_button(&mut self, text: &str) -> bool {
        debug_assert!(
            !text.chars().any(|c| c == '\n'),
            "print_button: only single line supported"
        );
        let w = text.chars().count() as i32;

        // Trim to size
        let text = if w > self.win.width() {
            text.chars().take(self.win.width() as usize).collect()
        } else {
            text.to_owned()
        };

        // Move to next line if line would go past right edge.
        if self.win.width() - self.pos.x < w && self.pos.x > 0 {
            self.pos.x = 0;
            self.pos.y += 1;
        }

        let bounds = self.win.sub(Rect::new(self.pos, self.pos + v2([w, 1])));
        self.pos.x += w;

        bounds.button(&text)
    }
}

impl fmt::Write for Cursor {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for (i, line) in s.lines().enumerate() {
            // Newlines
            if i > 0 {
                self.pos.y += 1;
                self.pos.x = 0;
            }
            self.pos = self.win.write(self.pos, line);
        }
        // .lines() doesn't catch the final newline.
        if s.ends_with('\n') {
            self.pos.y += 1;
            self.pos.x = 0;
        }
        Ok(())
    }
}
