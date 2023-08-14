//! Generic entity logic.
use std::{fmt, str::FromStr};

use derive_deref::Deref;
use hecs::Component;
use serde_with::{DeserializeFromStr, SerializeDisplay};

use crate::{ecs::*, prelude::*};

// Dummy wrapper so we can write impls for it directly instead of deriving a
// trait for hecs::Entity and writing every fn signature twice.
/// Game entity identifier datatype. All the actual contents live in the ECS.
#[derive(
    Copy,
    Clone,
    Hash,
    Eq,
    Ord,
    PartialEq,
    PartialOrd,
    Debug,
    Deref,
    SerializeDisplay,
    DeserializeFromStr,
)]
pub struct Entity(pub(crate) hecs::Entity);

impl Entity {
    pub(crate) fn get<T>(&self, c: &Core) -> T
    where
        T: Component + Clone + Default,
    {
        c.ecs
            .get::<&T>(**self)
            .map(|c| (*c).clone())
            .unwrap_or_default()
    }

    pub(crate) fn set<T>(&self, c: &mut Core, val: T)
    where
        T: Component + Default + PartialEq,
    {
        if val == T::default() {
            // Remove default values, abstraction layer assumes components are
            // always present but defaulted.
            //
            // Will give an error if the component wasn't there to begin with,
            // just ignore that.
            let _ = c.ecs.remove_one::<T>(**self);
        } else {
            c.ecs.insert_one(**self, val).expect("Entity::set failed");
        }
    }

    // XXX: with and with_mut used to be used with the old, stack-like Orders
    // component. They don't currently have any use. If new complex components
    // don't turn up, maybe remove them entirely.

    /// Access a component using a closure.
    ///
    /// Use for complex components that aren't just atomic values.
    #[allow(dead_code)]
    pub(crate) fn with<'a, T: Component + Default, U>(
        &self,
        c: &Core,
        f: impl Fn(&T) -> U,
    ) -> U {
        let scratch = T::default();
        if let Ok(c) = c.ecs.get::<&T>(**self) {
            f(&*c)
        } else {
            f(&scratch)
        }
    }

    /// Access and mutate a component using a closure.
    ///
    /// Use for complex components that aren't just atomic values.
    #[allow(dead_code)]
    pub(crate) fn with_mut<T: Component + Default + Eq, U>(
        &self,
        c: &mut Core,
        mut f: impl FnMut(&mut T) -> U,
    ) -> U {
        let mut delete = false;
        let mut insert = false;
        let ret;

        let mut scratch = T::default();
        if let Ok(query) = c.ecs.query_one_mut::<&mut T>(**self) {
            ret = f(&mut *query);
            // We created a default value once, reuse it here.
            if *query == scratch {
                delete = true;
            }
        } else {
            ret = f(&mut scratch);
            if scratch != T::default() {
                insert = true;
            }
        }

        if delete {
            // Component became default value, remove from ECS.
            let _ = c.ecs.remove_one::<T>(**self);
        } else if insert {
            // Scratch component became a valid value.
            c.ecs
                .insert_one(**self, scratch)
                .expect("Entity::with_mut failed to set entity");
        }

        ret
    }

    pub fn loc(&self, c: &Core) -> Option<Location> {
        c.placement.entity_pos(self)
    }

    pub fn place(&self, c: &mut Core, loc: Location) {
        c.placement.insert_at(loc, *self);
        self.post_move_hook(c);
    }

    /// Place an item near `loc`, deviating to avoid similar entities.
    ///
    /// Items will avoid other items, mobs will avoid other mobs.
    pub fn place_on_open_spot(&self, c: &mut Core, loc: Location) {
        // If no open position is found, just squeeze the thing right where it
        // was asked to go.
        let mut place_loc = loc;
        for loc in c.perturbed_fill_positions(loc) {
            if self.can_enter(c, loc) {
                place_loc = loc;
                break;
            }
        }

        self.place(c, place_loc);
    }

    fn post_move_hook(&self, c: &mut Core) {
        self.scan_fov(c);
        if let (true, Some(loc)) = (self.is_player(c), self.loc(c)) {
            if let Some(item) = loc.item_at(c) {
                self.take(c, &item);
            }
        }
    }

    /// Return the type of terrain the entity is expected to spawn in.
    pub fn preferred_tile(&self, _c: &Core) -> Tile {
        // Return a different tile if entity is aquatic or another weird type.
        Tile::Ground
    }

    pub fn icon(&self, c: &Core) -> char {
        match self.get::<Icon>(c) {
            Icon('\0') => '�',
            Icon(c) => c,
        }
    }

    pub fn draw_layer(&self, c: &Core) -> i32 {
        if self.is_mob(c) {
            return 1;
        }
        0
    }

    pub fn is_alive(&self, c: &Core) -> bool {
        self.loc(c).is_some()
    }

    /// Return capitalized name of an entity.
    ///
    /// This will probably get deprecated by a string templating system later.
    #[allow(non_snake_case)]
    pub fn Name(&self, c: &Core) -> String {
        let name = self.name(c);
        // XXX: ASCII only
        name[..1].to_uppercase() + &name[1..]
    }

    pub fn name(&self, c: &Core) -> String {
        let nickname = self.get::<Nickname>(c).0;
        let name = self.get::<Name>(c).0;
        let is_proper = name.chars().next().map_or(false, |c| c.is_uppercase());

        if !nickname.is_empty() {
            if is_proper {
                // Fully rename proper-named entities.
                format!("{}", nickname)
            } else {
                format!("{} the {}", nickname, name)
            }
        } else {
            name
        }
    }

    pub fn can_enter(&self, c: &Core, loc: Location) -> bool {
        if !loc.is_walkable(c) {
            return false;
        }
        if self.is_mob(c) && loc.mob_at(c).is_some() {
            return false;
        }
        if self.is_item(c) && loc.item_at(c).is_some() {
            return false;
        }

        true
    }

    /// Method called at the start of every frame.
    pub(crate) fn tick(&self, c: &mut Core) {
        if self.acts_this_frame(c) {
            // Clear momentum from previous turn at the start of the next one.
            self.set(c, Momentum::default());
        }
    }

    /// Movement direction along a given Dijkstra map for given location, if
    /// the map provides any valid steps.
    pub fn dijkstra_map_direction(
        &self,
        c: &Core,
        map: &HashMap<Location, usize>,
        loc: Location,
    ) -> Option<IVec2> {
        // Default to max, always prefer stepping from non-map to map.
        let start = map.get(&loc).copied().unwrap_or(usize::MAX);

        if let Some((best, n)) = loc
            .neighbors_4()
            .filter_map(|loc| {
                // Don't walk into enemies.
                if let Some(mob) = loc.mob_at(c) {
                    if self.is_enemy(c, &mob) {
                        return None;
                    }
                    // Friendlies are okay, assume they can be displaced.
                }
                map.get(&loc).map(|u| (loc, u))
            })
            .min_by_key(|(_, u)| *u)
        {
            if *n < start {
                debug_assert!(best.z == loc.z);
                let a = loc.to_vec();
                let b = best.to_vec();
                return Some(a.dir4_towards(&b));
            }
        }
        None
    }

    pub fn max_wounds(&self, c: &Core) -> i32 {
        ((self.get::<Level>(c).0 * 2) as f32).powf(1.25).round() as i32
    }

    pub fn wounds(&self, c: &Core) -> i32 {
        self.get::<Wounds>(c).0
    }

    pub fn damage(&self, c: &mut Core, amount: i32) {
        let mut wounds = self.wounds(c);
        wounds += amount;
        self.set(c, Wounds(wounds));
        if amount > 0 {
            send_msg(Msg::Hurt(*self));
        }
        if wounds >= self.max_wounds(c) {
            self.die(c);
        }
    }

    pub fn die(&self, c: &mut Core) {
        if let Some(loc) = self.loc(c) {
            send_msg(Msg::Death(loc));
        }
        // TODO 2023-01-17 Visual effect for mob death
        if let Some(loc) = self.loc(c) {
            let splat: Vec<Location> =
                c.perturbed_fill_positions(loc).take(6).collect();
            for loc in splat {
                loc.set_tile(c, Tile::Gore);
            }
        }
        self.destroy(c);

        if c.player == Some(*self) {
            // Field promote a minion.
            let npc = c.live_entities().find(|e| e.is_player_aligned(c));
            if let Some(npc) = npc {
                c.player = Some(npc);
            } else {
                // No minions found, game over.
                c.player = None;
            }
        }
    }

    pub fn destroy(&self, c: &mut Core) {
        c.placement.remove(self);
    }

    pub(crate) fn vec_towards(
        &self,
        c: &Core,
        other: &Entity,
    ) -> Option<IVec2> {
        let (Some(a), Some(b)) = (self.loc(c), other.loc(c)) else {
            return None;
        };
        a.vec_towards(&b)
    }

    pub fn contents<'a>(
        &self,
        c: &'a Core,
    ) -> impl Iterator<Item = Entity> + 'a {
        c.placement.entities_in(self)
    }
}

// Convert entities into nice dense opaque identifiers instead of having noisy
// integers like 4294967296 show up in savefiles.

impl fmt::Display for Entity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Entities are made of two u32s, both of which start with low values.
        // Naively turning this into a single u64 gives us annoying values
        // with lots of zeroes. Let's instead interleave the low bits to get
        // much nicer combined values.
        let u = self.0.to_bits().get();
        let a = util::spread_u64_by_2(u);
        let b = util::spread_u64_by_2(u >> 32) << 1;
        write!(f, "#{}", util::mung(a | b))
    }
}

impl FromStr for Entity {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if !s.starts_with('#') || s.len() < 2 {
            return Err("bad entity");
        }
        let v = util::unmung(&s[1..]);
        let a = util::compact_u64_by_2(v);
        let b = util::compact_u64_by_2(v >> 1);
        let u = a | (b << 32);
        Ok(Entity(hecs::Entity::from_bits(u).ok_or("bad entity")?))
    }
}
