use glam::IVec2;
use navni::prelude::*;
use util::v2;

use crate::{Buffer, Field, Font, Image, Pixel, Rect};

/// A view structure through which things can be drawn on a buffer.
#[derive(Copy, Clone, Default)]
pub struct Window<P: Pixel> {
    /// The window's bounds in the coordinates of the screen buffer.
    ///
    /// It's expected that the window is always used with a buffer that can
    /// fit its bounds rectangle.
    bounds: Rect,
    foreground_col: P,
    background_col: P,

    font: &'static Font<P>,
}

impl From<&Buffer<Rgba>> for Window<Rgba> {
    fn from(c: &Buffer<Rgba>) -> Self {
        Window::new(c.area(), Rgba::WHITE, Rgba::BLACK)
    }
}

impl From<&Buffer<CharCell>> for Window<CharCell> {
    fn from(c: &Buffer<CharCell>) -> Self {
        Window::new(c.area(), Default::default(), Default::default())
    }
}

impl<P: Pixel> Window<P> {
    pub fn new(
        region: Rect,
        foreground_col: P,
        background_col: P,
    ) -> Window<P> {
        Window {
            bounds: region,
            foreground_col,
            background_col,

            font: Default::default(),
        }
    }

    /// Area rectangle of the window in window-local coordinates, anchored to
    /// origin.
    pub fn area(&self) -> Rect {
        Rect::new([0, 0], self.bounds.dim())
    }

    pub fn width(&self) -> i32 {
        self.bounds.width()
    }

    pub fn height(&self) -> i32 {
        self.bounds.height()
    }

    /// Return whether window contains a point in absolute screen coordinates.
    pub fn contains(&self, pos: impl Into<[i32; 2]>) -> bool {
        self.bounds.contains(pos)
    }

    /// Window bounds rectangle in absolute screen coordinates.
    pub fn bounds(&self) -> &Rect {
        &self.bounds
    }

    pub fn fill(&self, c: &mut Buffer<P>, color: P) {
        let area = c.area();
        for pos in self.bounds {
            c.data[area.idx(pos)] = color;
        }
    }

    pub fn clear(&self, c: &mut Buffer<P>) {
        self.fill(c, self.background_col);
    }

    pub fn put(&self, c: &mut Buffer<P>, pos: impl Into<IVec2>, col: P) {
        let screen = c.area();
        let pos = pos.into();
        if !self.area().contains(pos) {
            return;
        }

        let pos: IVec2 = pos + v2(self.bounds.min());

        if self.bounds.contains(pos) {
            c.data[screen.idx(pos)] = col;
        }
    }

    pub fn get_mut<'a, 'b>(
        &'a self,
        c: &'b mut Buffer<P>,
        pos: impl Into<IVec2>,
    ) -> Option<&'b mut P> {
        let pos = pos.into();
        let idx = c.area().idx(pos + v2(self.bounds.min()));

        self.area().contains(pos).then_some(&mut c.data[idx])
    }

    /// Draw an image in the window.
    pub fn blit<F: Field<P>>(
        &self,
        c: &mut Buffer<P>,
        pos: impl Into<IVec2>,
        img: &Image<P, F>,
    ) {
        let pos = pos.into();
        for p in img.area() {
            let a = img.get(p);
            if !a.is_transparent() {
                self.put(c, pos + v2(p), a);
            }
        }
    }

    pub fn split_left(&self, width: i32) -> (Window<P>, Window<P>) {
        let [a, b] = self.area().split([width, 0]);
        (self.sub(a), self.sub(b))
    }

    pub fn split_right(&self, width: i32) -> (Window<P>, Window<P>) {
        let [a, b] = self.area().split([-width, 0]);
        (self.sub(a), self.sub(b))
    }

    pub fn split_top(&self, height: i32) -> (Window<P>, Window<P>) {
        let [a, b] = self.area().split([0, height]);
        (self.sub(a), self.sub(b))
    }

    pub fn split_bottom(&self, height: i32) -> (Window<P>, Window<P>) {
        let [a, b] = self.area().split([0, -height]);
        (self.sub(a), self.sub(b))
    }

    pub fn center(&self, dim: impl Into<IVec2>) -> Window<P> {
        let dim = dim.into();
        let offset = (v2(self.bounds.dim()) - dim) / 2;
        self.sub(Rect::sized(dim) + offset)
    }

    /// Create a sub-window from the area within this window's space.
    pub fn sub(&self, area: Rect) -> Window<P> {
        let area = area + self.bounds.min();
        let area = self.bounds.intersection(&area);

        let mut ret = *self;
        ret.bounds = area;
        ret
    }

    /// Write text to window, return updated position.
    pub fn write(
        &self,
        c: &mut Buffer<P>,
        pos: impl Into<IVec2>,
        text: &str,
    ) -> IVec2 {
        let mut pos = pos.into();
        // TODO apply foreground color
        for a in text.chars() {
            if let Some(idx) = self.font.idx(a) {
                let glyph = &self.font[idx];
                self.blit(c, pos, glyph);
                pos[0] += glyph.width();
            }
        }

        pos
    }

    pub fn write_center(&self, c: &mut Buffer<P>, text: &str) {
        let width = self.font.width(text);
        let x = self.width() / 2 - width / 2;
        self.write(c, [x, 0], text);
    }

    /// Draw a caption box GUI element.
    ///
    /// Returns the window for the inner area.
    pub fn caption_box(&self, c: &mut Buffer<P>, caption: &str) -> Window<P> {
        let w = self.font.width(caption);

        let x0 = 0;
        let y0 = self.font.height() / 2 - 1;
        let [x1, y1] = self.area().max();
        let [x1, y1] = [x1 - 1, y1 - 1];

        let x_extent = 8;

        self.line(c, [x0, y0], [x_extent, y0]);
        self.write(c, [x_extent, 0], caption);
        self.line(c, [x_extent + w + 1, y0], [x1, y0]);
        self.line(c, [x1, y0], [x1, y1]);
        self.line(c, [x1, y1], [x0, y1]);
        self.line(c, [x0, y1], [x0, y0]);

        self.sub(self.area().shrink([2, self.font.height() - 1], [2, 2]))
    }

    /// Write text with an updating cursor position.
    pub fn print(&self, c: &mut Buffer<P>, pos: &mut IVec2, text: &str) {
        for (i, line) in text.lines().enumerate() {
            // Newlines
            if i > 0 {
                pos.y += 1;
                pos.x = 0;
            }
            *pos = self.write(c, *pos, line);
        }
    }

    pub fn println(&self, c: &mut Buffer<P>, pos: &mut IVec2, text: &str) {
        self.print(c, pos, text);
        pos.y += 1;
        pos.x = 0;
    }

    pub fn print_button(
        &self,
        c: &mut Buffer<P>,
        pos: &mut IVec2,
        mouse: &MouseState,
        text: &str,
    ) -> bool {
        debug_assert!(
            !text.chars().any(|c| c == '\n'),
            "print_button: only single line supported"
        );
        let w = text.chars().count() as i32;
        // Move to next line if line would go past right edge.
        if self.width() - pos.x < w {
            pos.x = 0;
            pos.y += 1;
        }
        let bounds = Rect::new(*pos, *pos + v2([w, 1]));
        // TODO: Make the colors different if hovering (bold) or pressed
        // (invert) with the mouse.
        self.print(c, pos, text);
        self.clicked_on(mouse, &bounds)
    }

    pub fn line(
        &self,
        c: &mut Buffer<P>,
        p1: impl Into<IVec2>,
        p2: impl Into<IVec2>,
    ) {
        for p in util::bresenham_line(p1, p2) {
            self.put(c, p, self.foreground_col);
        }
    }

    pub fn draw_border(&self, c: &mut Buffer<P>) -> Window<P> {
        let [x1, y1] = self.bounds.dim();
        let [x1, y1] = [x1 - 1, y1 - 1];

        self.line(c, [0, 0], [x1, 0]);
        self.line(c, [x1, 0], [x1, y1]);
        self.line(c, [x1, y1], [0, y1]);
        self.line(c, [0, y1], [0, 0]);

        self.sub(self.area().shrink([1, 1], [1, 1]))
    }

    pub fn invert(&self, c: &mut Buffer<P>) {
        let bounds = c.area();
        for p in self.bounds {
            c.data[bounds.idx(p)].invert();
        }
    }

    pub fn button(
        &self,
        c: &mut Buffer<P>,
        mouse: &MouseState,
        text: &str,
    ) -> bool {
        self.clear(c);
        let sub = self.draw_border(c);
        // Double border when hovering.
        if self.hovering(mouse) {
            sub.draw_border(c);
        }

        self.center([self.width(), self.font.height()])
            .write_center(c, text);

        // Invert when pressed
        if self.pressed(mouse) {
            self.invert(c);
        }

        self.clicked(mouse)
    }

    fn hovering(&self, mouse: &MouseState) -> bool {
        if let MouseState::Hover(pos) = mouse {
            self.bounds.contains(*pos)
        } else {
            false
        }
    }

    fn pressed(&self, mouse: &MouseState) -> bool {
        if let MouseState::Drag(_, pos, MouseButton::Left) = mouse {
            self.bounds.contains(*pos)
        } else {
            false
        }
    }

    fn clicked(&self, mouse: &MouseState) -> bool {
        self.clicked_on(mouse, &self.area())
    }

    pub(crate) fn clicked_on(&self, mouse: &MouseState, bounds: &Rect) -> bool {
        if let MouseState::Release(current_pos, pos, MouseButton::Left) = mouse
        {
            bounds.contains(*pos) && bounds.contains(*current_pos)
        } else {
            false
        }
    }
}
