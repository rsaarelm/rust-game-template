use std::{
    str::FromStr,
    sync::{LazyLock, Mutex},
};

use glam::{IVec2, ivec2};
use navni::{CharCell, Rgba};
use util::{HashMap, hash_map::Entry};

use crate::{Image, Pixel, Rect};

pub struct Buffer<P> {
    width: u32,
    height: u32,
    pub(crate) data: Vec<P>,
}

impl<P> AsRef<Buffer<P>> for Buffer<P> {
    fn as_ref(&self) -> &Buffer<P> {
        self
    }
}

impl<P> AsMut<Buffer<P>> for Buffer<P> {
    fn as_mut(&mut self) -> &mut Buffer<P> {
        self
    }
}

impl<P> AsRef<[P]> for Buffer<P> {
    fn as_ref(&self) -> &[P] {
        &self.data
    }
}

impl<P> AsMut<[P]> for Buffer<P> {
    fn as_mut(&mut self) -> &mut [P] {
        &mut self.data
    }
}

impl From<image::DynamicImage> for Buffer<Rgba> {
    fn from(image: image::DynamicImage) -> Self {
        let image = image.to_rgba8();
        let (width, height) = (image.width(), image.height());
        let data = image.to_vec();

        // Reconfigure the u8 buffer we get from image into a bit-by-bit
        // equivalent Rgba buffer.
        assert!(data.len() % std::mem::size_of::<Rgba>() == 0);
        assert!(data.capacity() % std::mem::size_of::<Rgba>() == 0);
        let buf = unsafe {
            let ptr = data.as_ptr() as *mut Rgba;
            let len = data.len() / std::mem::size_of::<Rgba>();
            let cap = data.capacity() / std::mem::size_of::<Rgba>();
            std::mem::forget(data); // Don't run destructor.
            Vec::from_raw_parts(ptr, len, cap)
        };

        Buffer {
            width,
            height,
            data: buf,
        }
    }
}

static CACHE: LazyLock<Mutex<HashMap<&'static str, &'static Buffer<Rgba>>>> =
    LazyLock::new(Default::default);

impl FromStr for &'static Buffer<Rgba> {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        CACHE.lock().unwrap().get(s).ok_or(()).copied()
    }
}

impl Buffer<Rgba> {
    pub const KEY_COLOR: Rgba = Rgba::CYAN;

    /// Register a permanent asset to be accessible later using `<&'static
    /// Buffer<Rgba>>::from_str`.
    pub fn reg(name: &'static str, png_data: &'static [u8]) {
        match CACHE.lock().unwrap().entry(name) {
            Entry::Occupied(_) => panic!("asset is already defined"),
            // Leak the memory to get a &'static, these are expected to stay
            // around for the duration of the program.
            Entry::Vacant(e) => e.insert(Box::leak(Box::new(
                Buffer::from_bytes(png_data).expect("failed to decode PNG"),
            ))),
        };
    }

    pub fn from_bytes(
        bytes: &[u8],
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let mut ret: Buffer<Rgba> = image::load_from_memory(bytes)?.into();
        ret.set_key_to_transparent(Buffer::KEY_COLOR);
        Ok(ret)
    }

    /// Create a screenshot PNG of the buffer.
    pub fn to_png(&self) -> Vec<u8> {
        use image::ImageEncoder;

        let bounds = self.area();
        let img = image::RgbImage::from_fn(self.width, self.height, |x, y| {
            let p = self.data[bounds.idx([x as _, y as _])];
            image::Rgb([p.r, p.g, p.b])
        });

        let mut ret = Vec::new();
        let encoder = image::codecs::png::PngEncoder::new(&mut ret);
        encoder
            .write_image(
                &img.into_raw(),
                self.width,
                self.height,
                image::ExtendedColorType::Rgb8,
            )
            .unwrap();

        ret
    }
}

impl Buffer<CharCell> {
    /// Create a screenshot ANSI coded string of the buffer.
    pub fn to_ansi(&self) -> String {
        use navni::X256Color;

        let mut ret = String::new();
        // XXX: This is pretty gross.
        let col = |ret: &mut String, cell: CharCell| {
            let is_inverse = cell.foreground == X256Color::BACKGROUND
                && cell.background != X256Color::BACKGROUND;
            let foreground = if is_inverse {
                cell.background.0
            } else {
                cell.foreground.0
            };

            let is_bold = cell.foreground.0 >= 8 && cell.foreground.0 < 16;

            let fore_s = format!("38;5;{}", foreground);
            let back_s = format!("48;5;{}", cell.background.0);
            ret.push_str(&format!(
                "\x1b[0;{}{}{}{}m",
                if is_inverse { "7;" } else { "" },
                if is_bold { "1;" } else { "" },
                if foreground != X256Color::FOREGROUND.0 {
                    &fore_s
                } else {
                    ""
                },
                if !is_inverse && cell.background != X256Color::BACKGROUND {
                    &back_s
                } else {
                    ""
                }
            ));
        };

        col(&mut ret, self.data[0]);
        let mut prev = (self.data[0].foreground, self.data[0].background);
        let bounds = self.area();

        for y in 0..self.height {
            for x in 0..self.width {
                let cell = self.data[bounds.idx([x as _, y as _])];
                let current = (cell.foreground, cell.background);
                if current != prev {
                    col(&mut ret, cell);
                    prev = current;
                }
                let mut c = char::from_u32(cell.c as u32).unwrap();
                if c == '\0' {
                    c = ' ';
                }
                ret.push(c);
            }
            ret.push('\n');
        }

        // Reset settings.
        ret.push_str("\x1b[0m");

        ret
    }
}

impl<P: Pixel> Buffer<P> {
    pub fn new(width: u32, height: u32) -> Self {
        Buffer {
            width,
            height,
            data: vec![Default::default(); (width * height) as usize],
        }
    }

    pub fn from_fn(width: u32, height: u32, f: impl Fn(i32, i32) -> P) -> Self {
        let area = Rect::sized([width as i32, height as i32]);
        let data = (0..(width * height) as usize)
            .map(|i| {
                let [x, y] = area.get(i);
                f(x, y)
            })
            .collect();
        Buffer {
            width,
            height,
            data,
        }
    }

    pub fn pixels_mut(&mut self) -> impl Iterator<Item = &mut P> {
        self.data.iter_mut()
    }

    pub fn data(&self) -> &[P] {
        &self.data
    }

    pub fn data_mut(&mut self) -> &mut [P] {
        &mut self.data
    }

    pub fn set_key_to_transparent(&mut self, key: P) {
        for p in self.pixels_mut() {
            if *p == key {
                *p = P::default();

                debug_assert!(
                    p.is_transparent(),
                    "default cell is not transparent"
                );
            }
        }
    }

    pub fn dim(&self) -> IVec2 {
        ivec2(self.width as i32, self.height as i32)
    }

    pub fn width(&self) -> i32 {
        self.width as i32
    }

    pub fn height(&self) -> i32 {
        self.height as i32
    }

    pub fn area(&self) -> Rect {
        Rect::sized(self.dim())
    }

    pub fn subimages(&self, dim: impl Into<[i32; 2]>) -> Vec<Image<P, &Self>> {
        let dim = dim.into();
        let area = self.area();

        area.enclosed_lattice_iter(dim)
            .map(move |c| {
                debug_assert!(area.contains_other(&c));
                Image::new(self, c)
            })
            .collect()
    }
}
