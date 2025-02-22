use std::borrow::Cow;

use glam::{IVec2, ivec2};
use navni::prelude::*;
use util::StrExt;

use crate::prelude::*;

/// An interactive GUI component that can be rendered and can generate output
/// values using immediate mode GUI during rendering.
pub trait Widget {
    type Output;

    /// Render the widget inside the given window.
    fn render(&self, win: &Window) -> Option<Self::Output>;

    /// Suggest a window size for this widget.
    fn preferred_size(&self) -> Option<IVec2> {
        None
    }
}

/// Left-justified text.
impl Widget for str {
    type Output = ();

    fn render(&self, win: &Window) -> Option<Self::Output> {
        for (y, line) in self.lines_of(win.width() as usize).enumerate() {
            win.write([0, y as i32], line);
        }

        None
    }
}

/// Centered text with widget rendering support.
pub struct Centered(str);

impl Widget for Centered {
    type Output = ();

    fn render(&self, win: &Window) -> Option<Self::Output> {
        let w = win.width() as usize;
        for (y, line) in self.0.lines_of(w).enumerate() {
            let x = w - line.chars().count().min(w) / 2;
            win.write([x as i32, y as i32], line);
        }

        None
    }
}

pub struct ConfirmationDialog<'a> {
    text: Cow<'a, str>,
}

impl<'a> ConfirmationDialog<'a> {
    pub fn new(text: impl Into<Cow<'a, str>>) -> Self {
        let text = text.into();
        ConfirmationDialog { text }
    }
}

impl Widget for ConfirmationDialog<'_> {
    type Output = bool;

    fn render(&self, win: &Window) -> Option<Self::Output> {
        let mut ret = None;

        match navni::keypress().key() {
            Key::Char('y') | Key::Enter => ret = Some(true),
            c if c.is_some() => ret = Some(false),
            _ => {}
        }

        let win = win.box_border();

        let (buttons, mut message) = win.split_bottom(2);

        message.bounds = message.bounds.shrink([1, 0], [1, 0]);
        Widget::render(&*self.text, &message);

        let (ok, buttons) = buttons.box_split_right(8);
        if ok.wide_button("yes") {
            ret = Some(true);
        }

        let (cancel, _) = buttons.box_split_right(8);
        if cancel.wide_button("no") {
            ret = Some(false);
        }

        ret
    }

    fn preferred_size(&self) -> Option<IVec2> {
        Some(ivec2(32, 8))
    }
}
