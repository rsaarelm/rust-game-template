use std::{fmt, ops::Deref, sync::OnceLock};

use serde::{Deserialize, Serialize, Serializer};

/// Lazily initialized resource handle.
///
/// Resource items are stored in gamedata and may reference other gamedata
/// values, so lazy initialization is needed to allow gamedata to deserialize
/// fully before resource loading starts.
///
/// Failing to parse the resource causes a runtime panic.
#[derive(Clone, Default)]
pub struct LazyRes<S, T>(S, OnceLock<T>);

impl<S, T> LazyRes<S, T> {
    pub fn new(seed: impl Into<S>) -> Self {
        LazyRes(seed.into(), Default::default())
    }
}

impl<S: Clone, T: TryFrom<S>> Deref for LazyRes<S, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.1.get_or_init(|| {
            T::try_from(self.0.clone())
                .ok()
                .expect("failed to parse resource")
        })
    }
}

impl<S: fmt::Debug, T> fmt::Debug for LazyRes<S, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "LazyRes({:?})", self.0)
    }
}

impl<S: fmt::Display, T> fmt::Display for LazyRes<S, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl<S: AsRef<U>, T, U> AsRef<U> for LazyRes<S, T> {
    fn as_ref(&self) -> &U {
        self.0.as_ref()
    }
}

impl<S: PartialEq, T> PartialEq for LazyRes<S, T> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<S: Eq, T> Eq for LazyRes<S, T> {}

impl<'de, S, T> Deserialize<'de> for LazyRes<S, T>
where
    S: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let seed = S::deserialize(deserializer)?;

        Ok(LazyRes(seed, Default::default()))
    }
}

impl<S: Serialize, T> Serialize for LazyRes<S, T> {
    fn serialize<R>(&self, serializer: R) -> Result<R::Ok, R::Error>
    where
        R: Serializer,
    {
        self.0.serialize(serializer)
    }
}
