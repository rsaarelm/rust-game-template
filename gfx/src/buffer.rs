use std::{
    collections::hash_map::Entry,
    str::FromStr,
    sync::{LazyLock, Mutex},
};

use glam::{ivec2, IVec2};
use navni::Rgba;
use util::HashMap;

use crate::{Image, Pixel, Rect};

pub struct Buffer<P> {
    width: u32,
    height: u32,
    pub(crate) data: Vec<P>,
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
}

impl<P: Pixel> Buffer<P> {
    pub fn new(width: u32, height: u32) -> Self {
        Buffer {
            width,
            height,
            data: vec![Default::default(); (width * height) as usize],
        }
    }

    pub fn pixels_mut(&mut self) -> impl Iterator<Item = &mut P> {
        self.data.iter_mut()
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
