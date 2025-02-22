use std::{
    fmt,
    ops::Deref,
    sync::{Arc, OnceLock},
};

use serde::{Deserialize, Serialize, Serializer, de::DeserializeOwned};

/// Lazily initialized resource handle.
///
/// Resource items are stored in gamedata and may reference other gamedata
/// values, so lazy initialization is needed to allow gamedata to deserialize
/// fully before resource loading starts.
///
/// The resource is instantiated by deserializing the cached string using IDM.
#[derive(Clone, Default)]
pub struct LazyRes<T>(String, Arc<OnceLock<T>>);

impl<T> LazyRes<T> {
    pub fn new(seed: String) -> Self {
        LazyRes(seed, Default::default())
    }
}

impl<T: DeserializeOwned> Deref for LazyRes<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.1.get_or_init(|| {
            idm::from_str(&self.0).expect("failed to parse resource")
        })
    }
}

impl<T> fmt::Debug for LazyRes<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "LazyRes({:?})", self.0)
    }
}

impl<T> fmt::Display for LazyRes<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl<T: DeserializeOwned> AsRef<T> for LazyRes<T> {
    fn as_ref(&self) -> &T {
        self.deref()
    }
}

impl<T> PartialEq for LazyRes<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<T> Eq for LazyRes<T> {}

impl<T> PartialOrd for LazyRes<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> Ord for LazyRes<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}

impl<'de, T> Deserialize<'de> for LazyRes<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(LazyRes(
            String::deserialize(deserializer)?,
            Default::default(),
        ))
    }
}

impl<T> Serialize for LazyRes<T> {
    fn serialize<R>(&self, serializer: R) -> Result<R::Ok, R::Error>
    where
        R: Serializer,
    {
        self.0.serialize(serializer)
    }
}
