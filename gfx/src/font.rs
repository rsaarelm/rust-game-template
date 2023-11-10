use derive_more::Deref;
use itertools::Itertools;
use navni::Rgba;

use crate::{Buffer, Pixel, Rect, SubImage};

/// Replacement char to show when a character not covered by the font is
/// printed.
const MISSING: char = '?';

#[derive(Deref)]
pub struct Font<P: 'static>(pub(crate) Vec<SubImage<P>>);

impl<P: Pixel> Default for &'static Font<P> {
    fn default() -> Self {
        P::default_font()
    }
}

impl<P: Pixel> Font<P> {
    pub fn new(glyphs: Vec<SubImage<P>>) -> Font<P> {
        // Must have at least all the capital letters to count as a font sheet.
        assert!(
            glyphs.len() > (b'Z' - b' ') as usize,
            "too few letters for font"
        );

        debug_assert!(
            glyphs.iter().map(|c| c.height()).unique().count() == 1,
            "glyphs have varying heights"
        );

        Font(glyphs)
    }

    /// Return index to current font for a printable (non-control) character.
    ///
    /// Printable characters not covered by the font are replaced with the
    /// `MISSING` char.
    pub fn idx(&self, c: char) -> Option<usize> {
        let n = c as u32;
        if c.is_control() || n < 32 {
            return None;
        }

        let n = (n - 32) as usize;
        if n >= self.0.len() {
            Some((MISSING as u32 - 32) as usize)
        } else {
            Some(n)
        }
    }

    pub fn height(&self) -> i32 {
        self.0[0].height()
    }

    pub fn width(&self, s: &str) -> i32 {
        s.chars()
            .filter_map(|c| self.idx(c).map(|i| self.0[i].width()))
            .sum()
    }
}

impl Font<Rgba> {
    pub fn from_sheet(
        glyph_size: impl Into<[i32; 2]>,
        sheet: &'static Buffer<Rgba>,
    ) -> Font<Rgba> {
        let mut glyphs = sheet.subimages(glyph_size);
        for g in glyphs.iter_mut() {
            trim(g);
        }

        Font::new(glyphs)
    }
}

fn trim<P: Pixel>(ch: &mut SubImage<P>) {
    // Trim subimage bounds up to right margin:
    //
    // ..###...
    // ....##..
    // ..####..
    // .##.##..
    // ..####..
    //       ^
    // =>
    //
    // ..###.
    // ....##
    // ..####
    // .##.##
    // ..####

    let fallback = ch.width() / 2;
    ch.trim_right();
    if ch.width() == 0 {
        // Special case for fully empty cells (usually space), make it
        // half-width.
        ch.set_area(Rect::sized([fallback, ch.height()]));
    }
}
