use std::sync::LazyLock;

use navni::{CharCell, Rgba, X256Color};

use crate::{Buffer, Font, Rect, SubImage};

/// Trait for buffer cells, can be pixels or character cells.
///
/// Assumption: `<T: Cell>::default().is_transparent()` is always true.
pub trait Pixel:
    Copy + Default + Eq + PartialEq + From<Rgba> + 'static
{
    type Color;

    fn is_transparent(&self) -> bool;
    fn colorize(&self, col: Self::Color) -> Self;

    fn default_font() -> &'static Font<Self>;
    fn invert(&mut self);
}

impl Pixel for Rgba {
    type Color = Rgba;

    fn is_transparent(&self) -> bool {
        self.a == 0x00
    }

    fn colorize(&self, col: Self::Color) -> Self {
        col
    }

    fn default_font() -> &'static Font<Self> {
        static FONT_BUFFER: LazyLock<Buffer<Rgba>> = LazyLock::new(|| {
            Buffer::from_bytes(include_bytes!("../assets/font-8x8.png"))
                .expect("invalid default font data")
        });

        static FONT: LazyLock<Font<Rgba>> =
            LazyLock::new(|| Font::from_sheet([8, 8], &FONT_BUFFER));

        &FONT
    }

    fn invert(&mut self) {
        self.r = 0xff - self.r;
        self.g = 0xff - self.g;
        self.b = 0xff - self.b;
    }
}

impl Pixel for CharCell {
    type Color = (X256Color, X256Color);

    fn is_transparent(&self) -> bool {
        self.c == 0
    }

    fn colorize(&self, (foreground, background): Self::Color) -> Self {
        CharCell {
            c: self.c,
            foreground,
            background,
        }
    }

    fn default_font() -> &'static Font<Self> {
        // Instead of having a separate code path for "text mode doesn't need
        // a font, just write in cells directly", do the somewhat silly thing
        // of constructing explicit font data for text mode out of 1x1
        // charcell blocks.
        static FONT_BUFFER: LazyLock<Buffer<CharCell>> = LazyLock::new(|| {
            let mut ret: Buffer<CharCell> = Buffer::new(96, 1);
            for (i, p) in ret.pixels_mut().enumerate() {
                p.c = (i + 32) as u16;
            }
            ret
        });

        static FONT: LazyLock<Font<CharCell>> = LazyLock::new(|| {
            Font(
                (0..96)
                    .map(|x| {
                        SubImage::new(
                            &*FONT_BUFFER,
                            Rect::new([x, 0], [x + 1, 1]),
                        )
                    })
                    .collect(),
            )
        });

        &FONT
    }

    fn invert(&mut self) {
        std::mem::swap(&mut self.foreground, &mut self.background);
    }
}
