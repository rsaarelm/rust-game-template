use std::{fmt, ops::Deref, str::FromStr, sync::OnceLock};

use serde::Deserialize;

/// Lazily initialized resource handle.
///
/// Resource items are stored in gamedata and may reference other gamedata
/// values, so lazy initialization is needed to allow gamedata to deserialize
/// fully before resource loading starts.
///
/// Failing to parse the resource causes a runtime panic.
#[derive(Clone)]
pub struct Res<T>(String, OnceLock<T>);

impl<T: FromStr> Deref for Res<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.1.get_or_init(|| {
            T::from_str(&self.0).ok().expect("failed to parse resource")
        })
    }
}

impl<T> fmt::Debug for Res<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl<T> PartialEq for Res<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<T> Eq for Res<T> {}

impl<'de, T: FromStr> Deserialize<'de> for Res<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let path = String::deserialize(deserializer)?;

        Ok(Res(path, Default::default()))
    }
}
