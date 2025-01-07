//! Generic entity logic.
use std::{fmt, str::FromStr};

use derive_more::Deref;
use hecs::Component;
use serde_with::{DeserializeFromStr, SerializeDisplay};
use util::{Noun, Silo, StrExt};
use world::{Block, Data};

use crate::{ecs::*, placement::Place, prelude::*};

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
    pub(crate) fn get<T>(&self, r: &impl AsRef<Runtime>) -> T
    where
        T: Component + Clone + Default,
    {
        let r = r.as_ref();
        r.ecs
            .get::<&T>(**self)
            .map(|c| (*c).clone())
            .unwrap_or_default()
    }

    pub(crate) fn set<T>(&self, r: &mut impl AsMut<Runtime>, val: T)
    where
        T: Component + Default + PartialEq,
    {
        let r = r.as_mut();
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

    /// Access a component using a closure.
    ///
    /// Use for complex components that aren't just atomic values.
    pub(crate) fn with<T: Component + Default, U>(
        &self,
        r: &impl AsRef<Runtime>,
        f: impl Fn(&T) -> U,
    ) -> U {
        let r = r.as_ref();
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
    pub(crate) fn with_mut<T: Component + Default + Eq, U>(
        &self,
        r: &mut impl AsMut<Runtime>,
        mut f: impl FnMut(&mut T) -> U,
    ) -> U {
        let r = r.as_mut();
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

    /// Spawn an exact copy of an entity.
    pub(crate) fn spawn_clone(&self, r: &mut impl AsMut<Runtime>) -> Entity {
        let r = r.as_mut();

        let mut cloner = crate::ecs::clone_builder(&r.ecs, self.0);
        Entity(r.ecs.spawn(cloner.build()))
    }

    pub fn loc(&self, r: &impl AsRef<Runtime>) -> Option<Location> {
        let r = r.as_ref();
        r.placement.entity_pos(self)
    }

    pub fn place(&self, r: &mut impl AsMut<Runtime>, place: impl Into<Place>) {
        let r = r.as_mut();
        let place = place.into();
        if Some(place) != r.placement.get(self) {
            self.detach(r);
            // Try to merge stacks in the new place, if successful will
            // consume self to grow a stack. Otherwise move self.
            if self.try_merge_in(r, place) {
                return;
            }

            r.placement.insert(place, *self);
            self.post_move_hook(r);
        }
    }

    pub fn spawn_origin(&self, r: &impl AsRef<Runtime>) -> Option<Location> {
        r.as_ref()
            .samsara
            .iter()
            .find_map(|(&loc, &(e2, _))| (*self == e2).then_some(loc))
    }

    /// Look for an entity at target place to merge into.
    ///
    /// If merging was succesful, destroy this entity, grow the target by this
    /// entity's count and
    fn try_merge_in(
        &self,
        r: &mut impl AsMut<Runtime>,
        place: impl Into<Place>,
    ) -> bool {
        let r = r.as_mut();

        match self.get::<Count>(r).0 {
            // Not a stackable item.
            0 => return false,
            x if x < 0 => {
                panic!("Entity::merge_at: Entity has negative count {x}");
            }
            _ => {}
        }

        for e in r.entities(place).collect::<Vec<_>>() {
            // Just in case.
            if e == *self {
                continue;
            }

            // Match found, merge self with it and exit.
            if self.can_stack_with(r, &e) {
                let count = self.get::<Count>(r).0 + e.get::<Count>(r).0;
                e.set(r, Count(count));
                self.destroy(r);
                return true;
            }
        }

        false
    }

    pub(crate) fn post_move_hook(&self, r: &mut impl AsMut<Runtime>) {
        self.scan_fov(r);
    }

    /// Return the type of terrain the entity is expected to spawn in.
    pub fn preferred_block(&self, _c: &impl AsRef<Runtime>) -> Block {
        // Return a different block if entity is aquatic or another weird type.
        Block::Stone
    }

    pub fn icon(&self, r: &impl AsRef<Runtime>) -> char {
        match self.get::<Icon>(r) {
            Icon('\0') => '�',
            Icon(c) => c,
        }
    }

    pub fn draw_layer(&self, r: &impl AsRef<Runtime>) -> i32 {
        if self.is_mob(r) {
            return 1;
        }
        0
    }

    pub fn is_alive(&self, r: &impl AsRef<Runtime>) -> bool {
        self.loc(r).is_some()
    }

    /// Return whether this is the sort of entity that only one of it should
    /// exist in the game world.
    pub fn is_unique(&self, r: &impl AsRef<Runtime>) -> bool {
        // Add more cases here as needed. Currently we're using the convention
        // that unique items have Proper Nouns and non-uniques have a generic
        // name.
        self.desc(r).is_capitalized()
    }

    pub fn is_ephemeral(&self, r: &impl AsRef<Runtime>) -> bool {
        self.get::<IsEphemeral>(r).0
    }

    /// Description without modifiers like count or nicknames.
    pub fn base_desc(&self, r: &impl AsRef<Runtime>) -> String {
        self.get::<Name>(r).0.to_string()
    }

    /// Description string of the entity.
    pub fn desc(&self, r: &impl AsRef<Runtime>) -> String {
        let nickname = self.get::<Nickname>(r).0;

        let count = self.count(r);
        let name = if count > 1 {
            format!(
                "{count} {}",
                self.base_desc(r).pluralize(&Data::get().plurals)
            )
        } else {
            self.base_desc(r)
        };

        let is_proper = name.chars().next().map_or(false, |c| c.is_uppercase());

        if !nickname.is_empty() {
            if is_proper {
                // Fully rename proper-named entities.
                nickname.to_string()
            } else if self.is_mob(r) {
                format!("{nickname} the {name}")
            } else {
                format!("{name} called {nickname}")
            }
        } else {
            name
        }
    }

    /// Get the noun for this entity that is used in grammar templating.
    pub fn noun(&self, r: &impl AsRef<Runtime>) -> Noun {
        if self.is_player(r) {
            Noun::You
        } else if self.count(r) > 1 {
            Noun::Plural(self.desc(r))
        } else {
            Noun::It(self.desc(r))
        }
    }

    pub fn can_enter(&self, r: &impl AsRef<Runtime>, loc: Location) -> bool {
        let r = r.as_ref();

        if !loc.can_be_stood_in(r) {
            return false;
        }
        if self.is_mob(r) && loc.mob_at(r).is_some() {
            return false;
        }

        true
    }

    /// Method called at the start of every frame.
    pub(crate) fn tick(&self, r: &mut impl AsMut<Runtime>) {
        let r = r.as_mut();

        if self.acts_this_frame(r) {
            // Clear momentum from previous turn at the start of the next one.
            self.set(r, Momentum::default());
        }
    }

    pub fn destroy(&self, r: &mut impl AsMut<Runtime>) {
        let r = r.as_mut();
        r.placement.remove(self);
    }

    /// Like destroy, but only decrements one item from stack if it's a stack
    /// of multiple.
    pub(crate) fn consume(&self, r: &mut impl AsMut<Runtime>) {
        let r = r.as_mut();

        let count = self.count(r);
        if count > 1 {
            self.set(r, Count(count - 1));
        } else {
            self.destroy(r);
        }
    }

    /// Split one item off from a stack.
    pub(crate) fn split_off_one(&self, r: &mut impl AsMut<Runtime>) -> Entity {
        let r = r.as_mut();

        let count = self.count(r);
        if count > 1 {
            self.set(r, Count(count - 1));
            let ret = self.spawn_clone(r);
            ret.set(r, Count(1));
            ret
        } else {
            *self
        }
    }

    pub fn contents<'a>(
        &self,
        r: &'a impl AsRef<Runtime>,
    ) -> impl Iterator<Item = Entity> + 'a {
        let r = r.as_ref();
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
        write!(f, "#{}", Silo::from(a | b).value())
    }
}

impl FromStr for Entity {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if !s.starts_with('#') || s.len() < 2 {
            return Err("bad entity");
        }
        let v = u64::from(&Silo::new(&s[1..]));
        let a = util::compact_u64_by_2(v);
        let b = util::compact_u64_by_2(v >> 1);
        let u = a | (b << 32);
        Ok(Entity(hecs::Entity::from_bits(u).ok_or("bad entity")?))
    }
}
