//! Entity component system boilerplate for saving games etc.

use std::{cell::RefCell, collections::BTreeMap, fmt};

use content::{EquippedAt, ItemKind, Power};
use derive_more::{Deref, DerefMut};
use hecs::{
    serialize::row::{self, SerializeContext},
    EntityBuilder, EntityRef,
};
use serde::{
    de::{DeserializeSeed, MapAccess, Visitor},
    ser::SerializeMap,
    Deserialize, Deserializer, Serialize, Serializer,
};
use util::InString;

use crate::{power::PowerState, prelude::*, Buff};

macro_rules! components {
    {
        $($attrname:ident,)+
    } => {
        // Discriminator type that duplicates component names.
        #[derive(Serialize, Deserialize)]
        #[serde(rename_all = "kebab-case")]
        enum ComponentId {
            $($attrname,)+
        }

        // Switchboard statment using the discriminator.
        pub struct Context;

        // Live entity serialization and deserialization.
        impl SerializeContext for Context {
            fn serialize_entity<S>(
                &mut self,
                entity: hecs::EntityRef<'_>,
                mut map: S,
            ) -> Result<S::Ok, S::Error>
            where
                S: serde::ser::SerializeMap,
            {
                $(
                row::try_serialize::<$attrname, _, _>(
                    &entity, &ComponentId::$attrname, &mut map)?;
                )+
                map.end()
            }
        }

        impl DeserializeContext for Context {
            fn deserialize_entity<'de, M>(
                &mut self,
                mut map: M,
                entity: &mut hecs::EntityBuilder,
            ) -> Result<(), M::Error>
            where
                M: serde::de::MapAccess<'de>,
            {
                while let Some(key) = map.next_key()? {
                    match key {
                        $(
                            ComponentId::$attrname => {
                                entity.add::<$attrname>(map.next_value()?);
                            }
                        )+
                    }
                }
                Ok(())
            }
        }

        // Make HECS builders from prototype data.
        impl Context {
            pub fn deserialize_prototype<'de, M>(
                &mut self,
                mut map: M,
                prototype: &mut hecs::EntityBuilderClone,
            ) -> Result<(), M::Error>
            where
                M: serde::de::MapAccess<'de>,
            {
                while let Some(key) = map.next_key()? {
                    match key {
                        $(
                            ComponentId::$attrname => {
                                prototype.add::<$attrname>(map.next_value()?);
                            }
                        )+
                    }
                }
                Ok(())
            }
        }

        /// Create an `EntityBuilder` that clones an existing entity.
        pub(crate) fn clone_builder(world: &hecs::World, e: hecs::Entity) -> hecs::EntityBuilder {
            let mut builder = hecs::EntityBuilder::new();
            $(
                if let Ok(comp) = world.get::<&$attrname>(e) {
                    builder.add((&*comp).clone());
                }
            )+
            builder
        }
    }
}

// Component order here is reflected in save files, order by rough relevance
// (name first, obscure bookkeeping cache values last).
components! {
    Name,
    Nickname,
    Count,
    Icon,
    ItemKind,
    Powers,
    ItemPower,
    EquippedAt,
    Stats,
    Buffs,
    Speed,
    Wounds,
    Cash,
    NumDeaths,
    IsMob,
    Voice,
    IsFriendly,
    Goal,
    ActsNext,
    Momentum,
    IsEphemeral,
}

/// Time when the mob can act next.
#[derive(
    Copy, Clone, Debug, Eq, PartialEq, Default, Serialize, Deserialize,
)]
pub struct ActsNext(pub Instant);

/// Status effects on mob and their expiry times.
#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Default,
    Deref,
    DerefMut,
    Serialize,
    Deserialize,
)]
pub struct Buffs(BTreeMap<Buff, Instant>);

/// Stacking value, value 0 means there's one item but it does not stack.
#[derive(
    Copy, Clone, Debug, Eq, PartialEq, Default, Serialize, Deserialize,
)]
pub struct Count(pub i32);

#[derive(
    Copy, Clone, Debug, Eq, PartialEq, Default, Serialize, Deserialize,
)]
pub struct Icon(pub char);

/// Entities with this flag will be destroyed when the player respawns.
#[derive(
    Copy, Clone, Debug, Eq, PartialEq, Default, Serialize, Deserialize,
)]
pub struct IsEphemeral(pub bool);

#[derive(
    Copy, Clone, Debug, Eq, PartialEq, Default, Serialize, Deserialize,
)]
pub struct IsFriendly(pub bool);

#[derive(
    Copy, Clone, Debug, Eq, PartialEq, Default, Serialize, Deserialize,
)]
pub struct IsMob(pub bool);

#[derive(Clone, Debug, Eq, PartialEq, Default, Serialize, Deserialize)]
pub struct ItemPower(pub Option<Power>);

/// Used by AI movement, moving sets momentum and you can't displace a mob
/// that moved this turn against its momentum.
#[derive(
    Copy, Clone, Debug, Eq, PartialEq, Default, Serialize, Deserialize,
)]
pub struct Momentum(pub IVec2);

#[derive(Clone, Debug, Eq, PartialEq, Default, Serialize, Deserialize)]
pub struct Name(pub InString);

#[derive(Clone, Debug, Eq, PartialEq, Default, Serialize, Deserialize)]
pub struct Nickname(pub String);

/// How many times has the player died.
#[derive(Clone, Debug, Eq, PartialEq, Default, Serialize, Deserialize)]
pub struct NumDeaths(pub i32);

#[derive(Clone, Debug, Eq, PartialEq, Default, Serialize, Deserialize)]
pub struct Powers(pub BTreeMap<Power, PowerState>);

#[derive(
    Copy, Clone, Debug, Eq, PartialEq, Default, Serialize, Deserialize,
)]
pub struct Speed(pub i8);

#[derive(
    Copy, Clone, Debug, Eq, PartialEq, Default, Serialize, Deserialize,
)]
pub struct Stats {
    /// General power
    pub level: i32,
    /// Debican odds for landing an attack.
    ///
    /// This is a bonus field that can be left zero, the starting value is
    /// `level` for mobs.
    pub hit: i32,
    /// Deciban odds for evading an attack.
    pub ev: i32,
    /// Damage done with a successful attack.
    pub dmg: i32,
}

impl std::ops::Add for Stats {
    type Output = Stats;

    fn add(mut self, rhs: Self) -> Self::Output {
        self += rhs;
        self
    }
}

impl std::ops::AddAssign for Stats {
    fn add_assign(&mut self, rhs: Self) {
        self.level += rhs.level;
        self.hit += rhs.hit;
        self.ev += rhs.ev;
        self.dmg += rhs.dmg;
    }
}

impl fmt::Display for Stats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "hit: {} ev: {} dmg: {}", self.hit, self.ev, self.dmg)
    }
}

#[derive(
    Copy, Clone, Default, Debug, Eq, PartialEq, Serialize, Deserialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum Voice {
    #[default]
    Silent,
    Shout,
    Hiss,
    Gibber,
    Roar,
}

/// Reversed value, zero is healthy, high values are bad.
///
/// Go with the principle that default value is default state.
#[derive(
    Copy, Clone, Debug, Eq, PartialEq, Default, Serialize, Deserialize,
)]
pub struct Wounds(pub i32);

/// Fungible money carried by the mob (generally just the player). Treated
/// differently from a regular item so it gets its own component.
#[derive(
    Copy, Clone, Debug, Eq, PartialEq, Default, Serialize, Deserialize,
)]
pub struct Cash(pub i32);

////////////////////////////////

/// Entity component system. Stores all the data of game entities.
#[derive(Default, Deref, DerefMut)]
pub(crate) struct Ecs(pub(crate) hecs::World);

impl Ecs {
    pub(crate) fn iter(&self) -> impl Iterator<Item = Entity> + '_ {
        (&self.0).into_iter().map(|he| Entity(he.entity()))
    }
}

impl Serialize for Ecs {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serialize(&self.0, &mut Context, serializer)
    }
}

impl<'de> Deserialize<'de> for Ecs {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        Ok(Ecs(deserialize(&mut Context, deserializer)?))
    }
}

////////////////////////////////
//
// Row machinery copy-pasted from HECS source just so I can serialize my
// engine::Entities instead of hecs::Entities and have them do the
// pretty-print serialization encoding for the HECS save.

fn serialize<C, S>(
    world: &hecs::World,
    context: &mut C,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    C: SerializeContext,
    S: Serializer,
{
    let mut seq = serializer.serialize_map(Some(world.len() as usize))?;

    // Force entities to serialize in order so we get same savefile text for
    // same world state every time.
    let mut refs: Vec<_> = world.into_iter().collect();
    refs.sort_by_key(|a| a.entity());

    for e in refs {
        seq.serialize_key(&Entity(e.entity()))?;
        seq.serialize_value(&SerializeComponents(RefCell::new((
            context,
            Some(e),
        ))))?;
    }
    seq.end()
}

struct SerializeComponents<'a, C>(RefCell<(&'a mut C, Option<EntityRef<'a>>)>);

impl<'a, C: SerializeContext> Serialize for SerializeComponents<'a, C> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut this = self.0.borrow_mut();
        let entity = this.1.take().unwrap();
        let map = serializer.serialize_map(this.0.component_count(entity))?;
        this.0.serialize_entity(entity, map)
    }
}

/// Deserialize a [`World`] with a [`DeserializeContext`] and a [`Deserializer`]
pub fn deserialize<'de, C, D>(
    context: &mut C,
    deserializer: D,
) -> Result<hecs::World, D::Error>
where
    C: DeserializeContext,
    D: Deserializer<'de>,
{
    deserializer.deserialize_map(WorldVisitor(context))
}

pub trait DeserializeContext {
    /// Deserialize a single entity
    fn deserialize_entity<'de, M>(
        &mut self,
        map: M,
        entity: &mut EntityBuilder,
    ) -> Result<(), M::Error>
    where
        M: MapAccess<'de>;
}

struct WorldVisitor<'a, C>(&'a mut C);

impl<'de, 'a, C> Visitor<'de> for WorldVisitor<'a, C>
where
    C: DeserializeContext,
{
    type Value = hecs::World;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a world")
    }

    fn visit_map<A>(self, mut map: A) -> Result<hecs::World, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut world = hecs::World::new();
        let mut builder = EntityBuilder::new();
        while let Some(e) = map.next_key::<Entity>()? {
            map.next_value_seed(DeserializeComponents(self.0, &mut builder))?;
            world.spawn_at(e.0, builder.build());
        }
        Ok(world)
    }
}

struct DeserializeComponents<'a, C>(&'a mut C, &'a mut EntityBuilder);

impl<'de, 'a, C> DeserializeSeed<'de> for DeserializeComponents<'a, C>
where
    C: DeserializeContext,
{
    type Value = ();

    fn deserialize<D>(self, deserializer: D) -> Result<(), D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(ComponentsVisitor(self.0, self.1))
    }
}

struct ComponentsVisitor<'a, C>(&'a mut C, &'a mut EntityBuilder);

impl<'de, 'a, C> Visitor<'de> for ComponentsVisitor<'a, C>
where
    C: DeserializeContext,
{
    type Value = ();

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("an entity's components")
    }

    fn visit_map<A>(self, map: A) -> Result<(), A::Error>
    where
        A: MapAccess<'de>,
    {
        self.0.deserialize_entity(map, self.1)
    }
}
