use std::{
    str::FromStr,
    sync::{LazyLock, Mutex},
};

use glam::IVec2;
use navni::Rgba;
use regex::Regex;
use util::{HashMap, v2};

use crate::{Buffer, Pixel, Rect};

pub trait Field<P> {
    fn get(&self, pos: [i32; 2]) -> P;
}

impl<P: Pixel> Field<P> for &'_ Buffer<P> {
    fn get(&self, pos: [i32; 2]) -> P {
        let area = self.area();

        if area.contains(pos) {
            self.data[area.idx(pos)]
        } else {
            Default::default()
        }
    }
}

impl<P: Pixel, F: Fn([i32; 2]) -> P> Field<P> for F {
    fn get(&self, pos: [i32; 2]) -> P {
        self(pos)
    }
}

#[derive(Copy, Clone)]
pub struct Image<P, F> {
    field: F,
    bounds: Rect,
    phantom: std::marker::PhantomData<P>,
}

pub type SubImage<P> = Image<P, &'static Buffer<P>>;

impl<P: Pixel> std::ops::Add<usize> for SubImage<P> {
    type Output = SubImage<P>;

    fn add(self, rhs: usize) -> Self::Output {
        let basis = self.bounds.dim();
        let lattice = self.field.area().enclosed_lattice(basis);
        let idx = lattice.idx_using(basis, self.bounds.min());
        let bounds =
            Rect::cell(basis, lattice.get((idx + rhs) % lattice.len()));

        Image {
            field: self.field,
            bounds,
            phantom: Default::default(),
        }
    }
}

impl FromStr for SubImage<Rgba> {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        type Params = (String, i32, i32, i32);

        /// Parse path string to tile w, tile h and requested tile index.
        fn parse_path(path: &str) -> Option<Params> {
            static RE: LazyLock<Regex> = LazyLock::new(|| {
                Regex::new(r"^(.+-(\d+)x(\d+))#(\d+)$").unwrap()
            });

            let cap = RE.captures(path)?;

            let name = cap.get(1).unwrap().as_str().parse().unwrap();

            let w = cap.get(2).unwrap().as_str().parse().unwrap();
            let h = cap.get(3).unwrap().as_str().parse().unwrap();

            if w == 0 || h == 0 {
                return None;
            }

            let n = cap.get(4).unwrap().as_str().parse().unwrap();

            Some((name, w, h, n))
        }

        // Parsing is expensive, so keep already parsed names in a cache.
        static CACHE: LazyLock<Mutex<HashMap<String, Params>>> =
            LazyLock::new(Default::default);

        let mut cache = CACHE.lock().unwrap();
        let &(ref name, w, h, n) = {
            if let Some(params) = cache.get(s) {
                params
            } else {
                let Some(params) = parse_path(s) else {
                    return Err(());
                };
                cache.insert(s.to_owned(), params);
                &cache[s]
            }
        };

        let buf = <&'static Buffer<Rgba>>::from_str(name)?;

        let pitch = buf.width() / w;
        let rect = Rect::sized([w, h]) + [w * (n % pitch), h * (n / pitch)];
        if !buf.area().contains_other(&rect) {
            return Err(());
        }

        Ok(SubImage::new(buf, rect))
    }
}

impl<P: Pixel, F: Field<P>> Image<P, F> {
    pub fn new(field: F, bounds: Rect) -> Image<P, F> {
        Image {
            field,
            bounds,
            phantom: Default::default(),
        }
    }

    pub fn get(&self, pos: impl Into<[i32; 2]>) -> P {
        self.field
            .get((v2(pos.into()) + v2(self.bounds.min())).into())
    }

    pub fn dim(&self) -> IVec2 {
        self.bounds.dim().into()
    }

    pub fn width(&self) -> i32 {
        self.bounds.width()
    }

    pub fn height(&self) -> i32 {
        self.bounds.height()
    }

    pub fn area(&self) -> Rect {
        Rect::sized(self.dim())
    }

    pub fn column(&self, x: i32) -> impl Iterator<Item = P> + '_ {
        (0..self.dim().y).map(move |y| self.get([x, y]))
    }

    pub fn row(&self, y: i32) -> impl Iterator<Item = P> + '_ {
        (0..self.dim().x).map(move |x| self.get([x, y]))
    }

    pub fn set_area(&mut self, rect: Rect) {
        self.bounds = rect + self.bounds.min();
    }

    pub fn trim_right(&mut self) {
        let w = self.width()
            - (0..self.width())
                .rev()
                .take_while(|&x| self.column(x).all(|p| p.is_transparent()))
                .count() as i32;

        self.set_area(Rect::sized([w, self.height()]));
    }
}
