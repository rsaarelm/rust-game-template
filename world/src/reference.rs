use std::{fmt, ops, str::FromStr, sync::OnceLock};

use serde_with::{DeserializeFromStr, SerializeDisplay};

use crate::{data::GetReference, Data};

/// An indirect reference to element stored in data.
///
/// Serialized as just the ID, not the value. Lazily resolved, since
/// references can be embedded in data and can only resolve after data has
/// been fully loaded.
///
/// How ids are resolved into data is defined on a type-by-type basis by
/// implementing `GetReference` trait on `Data`.
#[derive(Clone, Default, DeserializeFromStr, SerializeDisplay)]
pub struct Reference<T: Default + 'static> {
    id: String,
    inner: OnceLock<&'static T>,
}

impl<T: Default> Reference<T> {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            ..Default::default()
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }
}

impl<T: Default> ops::Deref for Reference<T>
where
    Data: GetReference<T>,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.inner.get_or_init(|| {
            Data::get()
                .get_reference(&self.id)
                .expect("Failed to resolve reference")
        })
    }
}

impl<T: Default> fmt::Display for Reference<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.id)
    }
}

impl<T: Default> FromStr for Reference<T> {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::new(s.to_string()))
    }
}
