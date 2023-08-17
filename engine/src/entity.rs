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
    pub(crate) fn get<T>(&self, r: &Runtime) -> T
    where
        T: Component + Clone + Default,
    {
        r.ecs
            .get::<&T>(**self)
            .map(|c| (*c).clone())
            .unwrap_or_default()
    }

    pub(crate) fn set<T>(&self, r: &mut Runtime, val: T)
    where
        T: Component + Default + PartialEq,
    {
        if val == T::default() {
            // Remove default values, abstraction layer assumes components are
            // always present but defaulted.
            //
            // Will give an error if the component wasn't there to begin with,
            // just ignore that.
            let _ = r.ecs.remove_one::<T>(**self);
        } else {
            r.ecs.insert_one(**self, val).expect("Entity::set failed");
        }
    }

    // XXX: with and with_mut used to be used with the old, stack-like Orders
    // component. They don't currently have any use. If new complex components
    // don't turn up, maybe remove them entirely.

    /// Access a component using a closure.
    ///
    /// Use for complex components that aren't just atomic values.
    #[allow(dead_code)]
    pub(crate) fn with<T: Component + Default, U>(
        &self,
        r: &Runtime,
        f: impl Fn(&T) -> U,
    ) -> U {
        let scratch = T::default();
        if let Ok(c) = r.ecs.get::<&T>(**self) {
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
        r: &mut Runtime,
        mut f: impl FnMut(&mut T) -> U,
    ) -> U {
        let mut delete = false;
        let mut insert = false;
        let ret;

        let mut scratch = T::default();
        if let Ok(query) = r.ecs.query_one_mut::<&mut T>(**self) {
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
            let _ = r.ecs.remove_one::<T>(**self);
        } else if insert {
            // Scratch component became a valid value.
            r.ecs
                .insert_one(**self, scratch)
                .expect("Entity::with_mut failed to set entity");
        }

        ret
    }

    pub fn loc(&self, r: &Runtime) -> Option<Location> {
        r.placement.entity_pos(self)
    }

    pub fn place(&self, r: &mut Runtime, loc: Location) {
        r.placement.insert(loc, *self);
        self.post_move_hook(r);
    }

    /// Place an item near `loc`, deviating to avoid similar entities.
    ///
    /// Items will avoid other items, mobs will avoid other mobs.
    pub fn place_on_open_spot(&self, r: &mut Runtime, loc: Location) {
        // If no open position is found, just squeeze the thing right where it
        // was asked to go.
        let mut place_loc = loc;
        for loc in r.perturbed_fill_positions(loc) {
            if self.can_enter(r, loc) {
                place_loc = loc;
                break;
            }
        }

        self.place(r, place_loc);
    }

    fn post_move_hook(&self, r: &mut Runtime) {
        self.scan_fov(r);
        if let (true, Some(loc)) = (self.is_player(r), self.loc(r)) {
            if let Some(item) = loc.item_at(r) {
                self.take(r, &item);
            }
        }
    }

    /// Return the type of terrain the entity is expected to spawn in.
    pub fn preferred_tile(&self, _c: &Runtime) -> Tile {
        // Return a different tile if entity is aquatic or another weird type.
        Tile::Ground
    }

    pub fn icon(&self, r: &Runtime) -> char {
        match self.get::<Icon>(r) {
            Icon('\0') => '�',
            Icon(c) => c,
        }
    }

    pub fn draw_layer(&self, r: &Runtime) -> i32 {
        if self.is_mob(r) {
            return 1;
        }
        0
    }

    pub fn is_alive(&self, r: &Runtime) -> bool {
        self.loc(r).is_some()
    }

    /// Return capitalized name of an entity.
    ///
    /// This will probably get deprecated by a string templating system later.
    #[allow(non_snake_case)]
    pub fn Name(&self, r: &Runtime) -> String {
        let name = self.name(r);
        // XXX: ASCII only
        name[..1].to_uppercase() + &name[1..]
    }

    pub fn name(&self, r: &Runtime) -> String {
        let nickname = self.get::<Nickname>(r).0;
        let name = self.get::<Name>(r).0;
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

    pub fn can_enter(&self, r: &Runtime, loc: Location) -> bool {
        if !loc.is_walkable(r) {
            return false;
        }
        if self.is_mob(r) && loc.mob_at(r).is_some() {
            return false;
        }
        if self.is_item(r) && loc.item_at(r).is_some() {
            return false;
        }

        true
    }

    /// Method called at the start of every frame.
    pub(crate) fn tick(&self, r: &mut Runtime) {
        if self.acts_this_frame(r) {
            // Clear momentum from previous turn at the start of the next one.
            self.set(r, Momentum::default());
        }
    }

    /// Movement direction along a given Dijkstra map for given location, if
    /// the map provides any valid steps.
    pub fn dijkstra_map_direction(
        &self,
        r: &Runtime,
        map: &HashMap<Location, usize>,
        loc: Location,
    ) -> Option<IVec2> {
        // Default to max, always prefer stepping from non-map to map.
        let start = map.get(&loc).copied().unwrap_or(usize::MAX);

        if let Some((best, n)) = loc
            .neighbors_4()
            .filter_map(|loc| {
                // Don't walk into enemies.
                if let Some(mob) = loc.mob_at(r) {
                    if self.is_enemy(r, &mob) {
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

    pub fn max_wounds(&self, r: &Runtime) -> i32 {
        ((self.get::<Level>(r).0 * 2) as f32).powf(1.25).round() as i32
    }

    pub fn wounds(&self, r: &Runtime) -> i32 {
        self.get::<Wounds>(r).0
    }

    pub fn damage(&self, r: &mut Runtime, amount: i32) {
        let mut wounds = self.wounds(r);
        wounds += amount;
        self.set(r, Wounds(wounds));
        if amount > 0 {
            send_msg(Msg::Hurt(*self));
        }
        if wounds >= self.max_wounds(r) {
            self.die(r);
        }
    }

    pub fn die(&self, r: &mut Runtime) {
        if let Some(loc) = self.loc(r) {
            send_msg(Msg::Death(loc));
        }
        // TODO 2023-01-17 Visual effect for mob death
        if let Some(loc) = self.loc(r) {
            let splat: Vec<Location> =
                r.perturbed_fill_positions(loc).take(6).collect();
            for loc in splat {
                loc.set_tile(r, Tile::Gore);
            }
        }
        self.destroy(r);

        if r.player == Some(*self) {
            // Field promote a minion.
            let npc = r.live_entities().find(|e| e.is_player_aligned(r));
            if let Some(npc) = npc {
                r.player = Some(npc);
            } else {
                // No minions found, game over.
                r.player = None;
            }
        }
    }

    pub fn destroy(&self, r: &mut Runtime) {
        r.placement.remove(self);
    }

    pub(crate) fn vec_towards(
        &self,
        r: &Runtime,
        other: &Entity,
    ) -> Option<IVec2> {
        let (Some(a), Some(b)) = (self.loc(r), other.loc(r)) else {
            return None;
        };
        a.vec_towards(&b)
    }

    pub fn contents<'a>(
        &self,
        r: &'a Runtime,
    ) -> impl Iterator<Item = Entity> + 'a {
        r.placement.entities_in(self)
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