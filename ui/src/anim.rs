use crate::prelude::*;
use engine::prelude::*;
use glam::Vec2;
use navni::prelude::*;
use util::PlottedPoint;

pub trait Anim {
    fn render(
        &mut self,
        r: &Runtime,
        s: &mut Buffer,
        n_updates: u32,
        win: &Window,
        draw_offset: IVec2,
    ) -> bool;
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
        }
    }
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
        s: &mut Buffer,
        n_updates: u32,
        win: &Window,
        draw_offset: IVec2,
    ) -> bool {
        // If origin is None, particle was attached to an entity that's gone
        // now and the particle should be gone as well.
        let Some(origin) = self.origin.to_wide_vec(r) else {
            return false;
        };

        win.put(s, origin + self.pos.as_ivec2() - draw_offset, self.cell);

        // Tick down lifetime and update position.
        for _ in 0..n_updates {
            if self.lifetime == 0 {
                return false;
            }
            self.pos += self.velocity;
            self.lifetime -= 1;
        }

        true
    }
}
