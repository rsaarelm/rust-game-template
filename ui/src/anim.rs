use crate::prelude::*;
use engine::prelude::*;
use glam::Vec2;
use navni::{prelude::*, X256Color as X};
use rand::Rng;
use util::{v2, PlottedPoint};

pub trait Anim {
    fn render(
        &mut self,
        r: &Runtime,
        n_updates: u32,
        win: &Window,
        draw_offset: IVec2,
    ) -> bool;
}

impl<F: FnMut(&Runtime, u32, &Window, IVec2) -> bool> Anim for F {
    fn render(
        &mut self,
        r: &Runtime,
        n_updates: u32,
        win: &Window,
        draw_offset: IVec2,
    ) -> bool {
        (self)(r, n_updates, win, draw_offset)
    }
}

/// Where an animation is anchored.
#[derive(Copy, Clone, Debug)]
pub enum Anchor {
    /// A fixed location.
    Location(Location),
    /// An entity that can move around while the animation runs.
    ///
    /// Animation will always be drawn relative to the entity.
    /// If the entity disappears, the animation will as well.
    Entity(Entity),

    WidePos(IVec2),
}

impl From<Location> for Anchor {
    fn from(value: Location) -> Self {
        Anchor::Location(value)
    }
}

impl From<Entity> for Anchor {
    fn from(value: Entity) -> Self {
        Anchor::Entity(value)
    }
}

impl Anchor {
    fn to_wide_vec(self, r: &Runtime) -> Option<IVec2> {
        match self {
            Anchor::Location(loc) => Some(loc.unfold_wide()),
            Anchor::Entity(e) => e.loc(r).map(|loc| loc.unfold_wide()),
            Anchor::WidePos(p) => Some(p),
        }
    }
}

/// Helper function for the standard animation countdown logic.
///
/// Decrement `lifetime` `n_updates` times and return whether lifetime stayed
/// above zero.
pub fn countdown(n_updates: u32, lifetime: &mut usize) -> bool {
    debug_assert!(n_updates > 0);
    for _ in 0..n_updates {
        if *lifetime == 0 {
            return false;
        }
        *lifetime -= 1;
    }
    true
}

pub struct Particle {
    origin: Anchor,
    cell: CharCell,
    lifetime: usize,
    pos: PlottedPoint,
    velocity: Vec2,
}

impl Particle {
    /// Create a new particle starting at location.
    pub fn new(at: impl Into<Anchor>, lifetime: usize) -> Particle {
        Particle {
            origin: at.into(),
            cell: CharCell::c('*'),
            lifetime,
            pos: Default::default(),
            velocity: Default::default(),
        }
    }

    /// Set velocity.
    pub fn v(mut self, v: Vec2) -> Self {
        self.velocity = v;
        self
    }

    /// Set character.
    pub fn c(mut self, c: char) -> Self {
        self.cell.set_c(c);
        self
    }

    pub fn offset(mut self, v: impl Into<IVec2>) -> Self {
        self.pos += v.into().as_vec2();
        self
    }

    pub fn col(mut self, col: impl Into<X256Color>) -> Self {
        self.cell.foreground = col.into();
        self
    }
}

impl Anim for Particle {
    fn render(
        &mut self,
        r: &Runtime,
        n_updates: u32,
        win: &Window,
        draw_offset: IVec2,
    ) -> bool {
        // If origin is None, particle was attached to an entity that's gone
        // now and the particle should be gone as well.
        let Some(origin) = self.origin.to_wide_vec(r) else {
            return false;
        };

        win.put(origin + self.pos.as_ivec2() - draw_offset, self.cell);

        // Tick down lifetime and update position.
        self.pos += self.velocity * n_updates as f32;
        countdown(n_updates, &mut self.lifetime)
    }
}

pub struct Explosion {
    origin: Anchor,
    lifetime: usize,
}

impl Explosion {
    pub fn new(origin: impl Into<Anchor>) -> Self {
        let origin = origin.into();
        Explosion {
            origin,
            lifetime: 10,
        }
    }
}

impl Anim for Explosion {
    fn render(
        &mut self,
        r: &Runtime,
        n_updates: u32,
        win: &Window,
        draw_offset: IVec2,
    ) -> bool {
        let Some(origin) = self.origin.to_wide_vec(r).map(|p| p - draw_offset)
        else {
            return false;
        };
        let bounds = Rect::new(origin - ivec2(2, 1), origin + ivec2(3, 2));

        // Radius of outer cloud and inner void
        let (outer, inner) = match 10 - self.lifetime {
            x if x < 2 => (1, 0),
            x if x < 4 => (2, 0),
            x if x < 8 => (3, 1),
            _ => (3, 2),
        };

        for p in bounds {
            let mut v = v2(p) - origin;
            v.x /= 2;

            if v.taxi_len() < outer && v.taxi_len() >= inner {
                win.put(p, CharCell::c('*').col(X::YELLOW));
            }
        }

        countdown(n_updates, &mut self.lifetime)
    }
}

/// Lightning bolt hitting from the sky.
///
/// Yeah, it doesn't make any sense in a dungeon, but it was easier to make
/// than a jaggly directed line from A to B.
pub struct Lightning {
    origin: Anchor,
    lifetime: usize,
}

impl Lightning {
    pub fn new(origin: impl Into<Anchor>) -> Self {
        let origin = origin.into();
        Lightning {
            origin,
            lifetime: 10,
        }
    }
}

impl Anim for Lightning {
    fn render(
        &mut self,
        r: &Runtime,
        n_updates: u32,
        win: &Window,
        draw_offset: IVec2,
    ) -> bool {
        let Some(origin) = self.origin.to_wide_vec(r).map(|p| p - draw_offset)
        else {
            return false;
        };

        let mut x = origin.x;
        for y in (0..=origin.y).rev() {
            let p = ivec2(x, y);
            win.put(p, CharCell::c('|').col(X::AQUA));
            match util::srng(&(p, self.lifetime / 5)).gen_range(0..7) {
                0 if x >= origin.x => x -= 1,
                1 if x <= origin.x => x += 1,
                _ => {}
            }
        }

        countdown(n_updates, &mut self.lifetime)
    }
}
