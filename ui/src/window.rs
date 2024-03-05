use gfx::{Buffer, Field, Image, Rect};
use glam::{ivec2, IVec2};
use navni::{prelude::*, X256Color as X};
use util::{v2, Neighbors2D};

use crate::{game, tile_display::SINGLE_LINE};

/// A view structure through which things can be drawn on a buffer.
#[derive(Copy, Clone, Default)]
pub struct Window {
    /// The window's bounds in the coordinates of the screen buffer.
    pub bounds: Rect,
    pub foreground_col: X256Color,
    pub background_col: X256Color,
}

impl Window {
    /// Open root window to buffer.
    pub fn root() -> Self {
        Window::new(canvas().area(), X::FOREGROUND, X::BACKGROUND)
    }

    pub fn new(
        region: Rect,
        foreground_col: X256Color,
        background_col: X256Color,
    ) -> Window {
        Window {
            bounds: region,
            foreground_col,
            background_col,
        }
    }

    /// Return if the window has no area.
    pub fn is_zero(&self) -> bool {
        self.area().is_empty()
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

    pub fn origin(&self) -> IVec2 {
        self.bounds.min().into()
    }

    pub fn dim(&self) -> IVec2 {
        self.bounds.dim().into()
    }

    /// Return whether window contains a point in absolute screen coordinates.
    pub fn contains(&self, pos: impl Into<[i32; 2]>) -> bool {
        self.bounds.contains(pos)
    }

    /// Window bounds rectangle in absolute screen coordinates.
    pub fn bounds(&self) -> &Rect {
        &self.bounds
    }

    pub fn fill(&self, color: CharCell) {
        for c in self.area().into_iter().filter_map(|p| self.get_mut(p)) {
            *c = color;
        }
    }

    pub fn clear(&self) {
        self.fill(CharCell::new(
            '\0',
            self.foreground_col,
            self.background_col,
        ));
    }

    /// Put method that can go outside the window's borders.
    fn unbound_put(&self, pos: impl Into<IVec2>, cell: CharCell) {
        let c = &mut canvas();
        let pos: IVec2 = pos.into() + v2(self.bounds.min());

        let screen = c.area();
        if screen.contains(pos) {
            c.data_mut()[screen.idx(pos)] = cell;
        }
    }

    pub fn put(&self, pos: impl Into<IVec2>, cell: CharCell) {
        let pos = pos.into();
        if self.area().contains(pos) {
            self.unbound_put(pos, cell);
        }
    }

    pub fn putc(&self, pos: impl Into<IVec2>, c: char) {
        self.put(
            pos,
            CharCell::new(c, self.foreground_col, self.background_col),
        );
    }

    pub fn get(&self, pos: impl Into<IVec2>) -> CharCell {
        let c = &mut canvas();
        let pos: IVec2 = pos.into() + v2(self.bounds.min());

        let screen = c.area();
        if screen.contains(pos) {
            c.data()[screen.idx(pos)]
        } else {
            Default::default()
        }
    }

    pub fn get_mut(
        &self,
        pos: impl Into<IVec2>,
    ) -> Option<&'static mut CharCell> {
        let c = canvas();

        let pos = pos.into();
        let canvas_pos = pos + v2(self.bounds.min());
        let idx = c.area().idx(canvas_pos);

        if self.area().contains(pos) && c.area().contains(canvas_pos) {
            Some(&mut c.data_mut()[idx])
        } else {
            None
        }
    }

    /// Draw an image in the window.
    pub fn blit<F: Field<CharCell>>(
        &self,
        pos: impl Into<IVec2>,
        img: &Image<CharCell, F>,
    ) {
        let pos = pos.into();
        for p in img.area() {
            let a = img.get(p);
            self.put(pos + v2(p), a);
        }
    }

    pub fn blit_masked<F: Field<CharCell>>(
        &self,
        pos: impl Into<IVec2>,
        img: &Image<CharCell, F>,
    ) {
        let pos = pos.into();
        for p in img.area() {
            let a = img.get(p);
            if a.c != 0 {
                self.put(pos + v2(p), a);
            }
        }
    }

    pub fn split_left(&self, width: i32) -> (Self, Self) {
        let [a, b] = self.area().split([width, 0]);
        (self.sub(a), self.sub(b))
    }

    pub fn split_right(&self, width: i32) -> (Self, Self) {
        let [a, b] = self.area().split([-width, 0]);
        (self.sub(b), self.sub(a))
    }

    pub fn split_top(&self, height: i32) -> (Self, Self) {
        let [a, b] = self.area().split([0, height]);
        (self.sub(a), self.sub(b))
    }

    pub fn split_bottom(&self, height: i32) -> (Self, Self) {
        let [a, b] = self.area().split([0, -height]);
        (self.sub(b), self.sub(a))
    }

    pub fn center(&self, dim: impl Into<IVec2>) -> Self {
        let dim = dim.into();
        let offset = (v2(self.bounds.dim()) - dim) / 2;
        self.sub(Rect::sized(dim) + offset)
    }

    pub fn box_split_left(&self, width: i32) -> (Self, Self) {
        let (a, b) = self.split_left(width + 1);
        (a.sticky_box_border(), b)
    }

    pub fn box_split_right(&self, width: i32) -> (Self, Self) {
        let (a, b) = self.split_right(width + 1);
        (a.sticky_box_border(), b)
    }

    pub fn box_split_top(&self, height: i32) -> (Self, Self) {
        let (a, b) = self.split_top(height + 1);
        (a.sticky_box_border(), b)
    }

    pub fn box_split_bottom(&self, height: i32) -> (Self, Self) {
        let (a, b) = self.split_bottom(height + 1);
        (a.sticky_box_border(), b)
    }

    /// Draw a box on the outer rim of the window and return a new window for
    /// the area inside the border.
    pub fn box_border(&self) -> Self {
        let area = self.area();
        for x in 1..area.width() - 1 {
            self.putc([x, 0], '─');
            self.putc([x, area.height() - 1], '─');
        }

        for y in 1..area.height() - 1 {
            self.putc([0, y], '│');
            self.putc([area.width() - 1, y], '│');
        }

        self.putc([0, 0], '┌');
        self.putc([area.width() - 1, 0], '┐');
        self.putc([0, area.height() - 1], '└');
        self.putc([area.width() - 1, area.height() - 1], '┘');

        let ret = self.sub(area.shrink([1, 1], [1, 1]));
        ret.clear();
        ret
    }

    /// Draw a box border that merges with intact borders immediately outside
    /// the window if there are any.
    pub fn sticky_box_border(&self) -> Self {
        let [w, h] = self.bounds.dim();

        let mut ret_area = self.area();

        // 0 1  bit 1 = x axis
        // 2 3  bit 2 = y axis
        //
        // Each edge that gets drawn adds 1 to draw_corner.
        // Two edges -> draw_corner is 2 -> corner is drawn.
        let mut draw_corner = [0; 4];

        // Top
        if !(0..w).all(|x| box_mask(self.get([x, -1])) != 0) {
            ret_area = ret_area.shrink([0, 1], [0, 0]);

            draw_corner[0] += 1;
            draw_corner[1] += 1;

            for x in 0..w {
                self.putc([x, 0], '─');
            }

            self.weld([-1, 0]);
            self.weld([w, 0]);
        }

        // Left
        if !(0..h).all(|y| box_mask(self.get([-1, y])) != 0) {
            ret_area = ret_area.shrink([1, 0], [0, 0]);

            draw_corner[0] += 1;
            draw_corner[2] += 1;

            for y in 0..h {
                self.putc([0, y], '│');
            }

            self.weld([0, -1]);
            self.weld([0, h]);
        }

        // Bottom
        if !(0..w).all(|x| box_mask(self.get([x, h])) != 0) {
            ret_area = ret_area.shrink([0, 0], [0, 1]);

            draw_corner[2] += 1;
            draw_corner[3] += 1;

            for x in 0..w {
                self.putc([x, h - 1], '─');
            }

            self.weld([-1, h - 1]);
            self.weld([w, h - 1]);
        }

        // Right
        if !(0..h).all(|y| box_mask(self.get([w, y])) != 0) {
            ret_area = ret_area.shrink([0, 0], [1, 0]);

            draw_corner[1] += 1;
            draw_corner[3] += 1;

            for y in 0..h {
                self.putc([w - 1, y], '│');
            }

            self.weld([w - 1, -1]);
            self.weld([w - 1, h]);
        }

        // Same order as draw_corner array in the string.
        for (i, c) in "┌┐└┘"
            .chars()
            .enumerate()
            .filter(|&(i, _)| draw_corner[i] >= 2)
        {
            let x = (i as i32 & 1).signum();
            let y = (i as i32 & 2).signum();
            let p = ivec2(x * w, y * h);

            // Draw the corner
            self.putc(p, c);
        }

        self.sub(ret_area)
    }

    pub fn box_caption(&self, title: &str) {
        let caption_area =
            self.unbound_sub(Rect::new([1, -1], [self.area().width(), 0]));
        caption_area.write([0, 0], title);
    }

    /// Create a sub-window from the area within this window's space.
    pub fn sub(&self, area: Rect) -> Self {
        let area = area + self.bounds.min();
        let area = self.bounds.intersection(&area);

        let mut ret = *self;
        ret.bounds = area;
        ret
    }

    pub fn grow(&self) -> Self {
        let mut ret = *self;
        ret.bounds = ret.bounds.grow([1, 1], [1, 1]);
        ret
    }

    pub fn unbound_sub(&self, area: Rect) -> Self {
        let mut ret = *self;
        ret.bounds = area + self.bounds.min();
        ret
    }

    /// Write text to window, return updated position.
    pub fn write(&self, pos: impl Into<IVec2>, text: &str) -> IVec2 {
        let mut pos = pos.into();
        for a in text.chars() {
            self.putc(pos, a);
            pos.x += 1;
        }

        pos
    }

    pub fn write_center(&self, text: &str) {
        let width = text.chars().count() as i32;
        let x = self.width() / 2 - width / 2;
        self.write([x, 0], text);
    }

    pub fn invert(&self) {
        for c in self.area().into_iter().filter_map(|p| self.get_mut(p)) {
            c.invert()
        }
    }

    pub fn button(&self, text: &str) -> bool {
        let mut button = *self;

        button.clear();
        if button.hovering() {
            // Emphasize when hovering
            button.foreground_col = brighten(button.foreground_col);
        }

        button.center([button.width(), 1]).write_center(text);

        // Invert when pressed
        if button.pressed() {
            button.invert();
        }

        button.clicked()
    }

    /// Button that grabs mouse events from the surrounding border as well.
    pub fn wide_button(&self, text: &str) -> bool {
        let mut button = *self;
        let area = self.grow();

        button.clear();
        if area.hovering() {
            // Emphasize when hovering
            button.foreground_col = brighten(button.foreground_col);
        }

        button.center([button.width(), 1]).write_center(text);

        // Invert when pressed
        if area.pressed() {
            button.invert();
        }

        area.clicked()
    }

    fn hovering(&self) -> bool {
        let mouse = navni::mouse_state();
        if let MouseState::Hover(pos) = mouse {
            self.bounds.contains(pos)
        } else {
            false
        }
    }

    fn pressed(&self) -> bool {
        let mouse = navni::mouse_state();
        if let MouseState::Drag(_, pos, MouseButton::Left) = mouse {
            self.bounds.contains(pos)
        } else {
            false
        }
    }

    fn clicked(&self) -> bool {
        self.clicked_on(&self.bounds)
    }

    pub(crate) fn clicked_on(&self, bounds: &Rect) -> bool {
        let mouse = navni::mouse_state();
        if let MouseState::Release(current_pos, pos, MouseButton::Left) = mouse
        {
            bounds.contains(pos) && bounds.contains(current_pos)
        } else {
            false
        }
    }

    /// Shape a box-drawing char in a given cell to look like it connects to
    /// any of the four adjacent other box-drawing chars if they are contained
    /// in current window. The char being welded can be outside the current
    /// window.
    fn weld(&self, pos: impl Into<IVec2>) {
        let pos = pos.into();
        let mut cell = self.get(pos);
        let area = self.area();
        let old_mask = box_mask(cell);

        if old_mask != 0 {
            let mask: usize = pos
                .ns_4()
                .enumerate()
                .filter_map(|(i, p)| {
                    (area.contains(p) && box_mask(self.get(p)) != 0)
                        .then_some(1 << i)
                })
                .sum();
            // Take the old mask bits into account when doing the new mask.
            // Old mask preserves existing connections to box cells outside
            // the current window.
            cell.set_c(SINGLE_LINE[mask | old_mask]);
            self.unbound_put(pos, cell);
        }
    }
}

impl From<Window> for Buffer<CharCell> {
    fn from(value: Window) -> Self {
        let [w, h] = value.area().dim();
        Buffer::from_fn(w as u32, h as u32, |x, y| value.get([x, y]))
    }
}

fn canvas() -> &'static mut Buffer<CharCell> {
    &mut game().s
}

fn brighten(col: X256Color) -> X256Color {
    match col {
        X256Color(x) if x < 8 => X256Color(x + 8),
        a => a,
    }
}

fn box_mask(c: impl Into<char>) -> usize {
    match c.into() {
        '╵' => 0x1,
        '╶' => 0x2,
        '└' | '╰' => 0x3,
        '╷' => 0x4,
        '│' => 0x5,
        '┌' | '╭' => 0x6,
        '├' => 0x7,
        '╴' => 0x8,
        '┘' | '╯' => 0x9,
        '─' => 0xA,
        '┴' => 0xB,
        '┐' | '╮' => 0xC,
        '┤' => 0xD,
        '┬' => 0xE,
        '┼' => 0xF,
        _ => 0,
    }
}
