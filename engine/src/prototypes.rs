use std::fmt;

use derive_deref::{Deref, DerefMut};
use serde::{
    de::{DeserializeSeed, MapAccess, Visitor},
    Deserialize, Deserializer,
};

use crate::{ecs, prelude::*};

/// Named prototypes for constructing new things like "coyote" or
/// "broadsword".
#[derive(Clone, Default, Deref, DerefMut)]
pub struct Prototypes(HashMap<String, hecs::BuiltEntityClone>);

impl<'de> Deserialize<'de> for Prototypes {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        deserializer.deserialize_map(PrototypeVisitor)
    }
}

// XXX: Giant boiler-plate copy-pasted from HECS. Is there a better way to do
// this?

struct PrototypeVisitor;

impl<'de> Visitor<'de> for PrototypeVisitor {
    type Value = Prototypes;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a prototype map")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Prototypes, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut prototypes = Prototypes::default();
        while let Some(id) = map.next_key::<String>()? {
            let mut builder = hecs::EntityBuilderClone::new();

            // Opinionation: Write map key directly into name component.
            builder.add(ecs::Name(id.clone()));

            map.next_value_seed(DeserializeComponents(&mut builder))?;
            prototypes.insert(id, builder.build());
        }
        Ok(prototypes)
    }
}

struct DeserializeComponents<'a>(&'a mut hecs::EntityBuilderClone);

impl<'de, 'a> DeserializeSeed<'de> for DeserializeComponents<'a> {
    type Value = ();

    fn deserialize<D>(self, deserializer: D) -> Result<(), D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(ComponentsVisitor(self.0))
    }
}

struct ComponentsVisitor<'a>(&'a mut hecs::EntityBuilderClone);

impl<'de, 'a> Visitor<'de> for ComponentsVisitor<'a> {
    type Value = ();

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("an entity's components")
    }

    fn visit_map<A>(self, map: A) -> Result<(), A::Error>
    where
        A: MapAccess<'de>,
    {
        ecs::Context.deserialize_prototype(map, self.0)
    }
}
