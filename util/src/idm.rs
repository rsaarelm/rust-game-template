use std::{borrow::Borrow, fmt, fs, path::Path, str::FromStr};

use anyhow::{anyhow, Result};
use derive_more::Deref;
use serde::{Deserialize, Serialize};
use serde_with::{DeserializeFromStr, SerializeDisplay};

/// Read a directory tree into a single IDM outline.
///
/// Directory trees have natural `IncrementalOutline` semantics,
/// subdirectories are append headlines and file names are overwrite
/// headlines, so the output is an `IncrementalOutline`. Use `idm::transmute`
/// to change it into the type you want.
pub fn dir_to_idm(path: impl AsRef<Path>) -> Result<IncrementalOutline> {
    use IncrementalHeadline::*;

    // If pointed at a file, just read the file.
    if path.as_ref().is_file() {
        return Ok(idm::from_str(&fs::read_to_string(path)?)?);
    }

    let mut ret = Vec::new();
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let is_dir = entry.file_type()?.is_dir();
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| anyhow!("bad filename {:?}", entry.file_name()))?;

        if is_dir {
            // Recurse into all subdirectories, make them append headlines.
            ret.push(((Append(name),), dir_to_idm(entry.path())?));
        } else if let Some(base) = name.strip_suffix(".idm") {
            // Recurse into IDM files, make the overwrite headlines.
            ret.push((
                (Overwrite(base.to_string()),),
                dir_to_idm(entry.path())?,
            ));
        } else {
            // Skip non-IDM files.
            continue;
        }
    }

    Ok(IncrementalOutline(ret))
}

/// A wrapper type that converts underscores in serialization to spaces at
/// runtime.
///
/// This allows embedding strings with spaces in space-separate inline IDM
/// data.
///
/// ```notrust
/// wand_of_death  10  20
/// ```
///
/// Deserializes into
///
/// ```notrust
/// ("wand of death", 10, 20): (UnderscoreString, i32, i32)
/// ```
#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Deref)]
pub struct _String(pub String);

impl Borrow<str> for _String {
    fn borrow(&self) -> &str {
        self.0.borrow()
    }
}

impl Serialize for _String {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let escaped: String = self
            .0
            .chars()
            .map(|c| if c.is_whitespace() { '_' } else { c })
            .collect();
        escaped.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for _String {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let escaped = String::deserialize(deserializer)?;
        Ok(_String(
            escaped
                .chars()
                .map(|c| if c == '_' { ' ' } else { c })
                .collect(),
        ))
    }
}

/// The default simple IDM outline.
#[derive(
    Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize,
)]
pub struct Outline(pub Vec<((String,), Outline)>);

/// A special IDM outline for incremental composition.
///
/// A partial outline `B` can be applied to outline `A`, producing a patched
/// outline. Sections in `B` with overwrite headlines will replace any
/// corresponding section in `A`. Sections in `B` with append headlines will
/// have their contents appended to the corresponding section in `A`. Sections
/// in `B` without a corresponding section in `A` will be appended to the
/// parent block in `A`.
///
/// When a directory tree is read into an outline, the subdirectories are
/// treated as append headlines. In a single file, headlines that start with
/// `@` will be read in without the initial `@` and treated as append
/// headlines. All other headlines are considered overwrite headlines.
#[derive(
    Clone,
    Default,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
    Debug,
    Serialize,
    Deserialize,
)]
pub struct IncrementalOutline(
    Vec<((IncrementalHeadline,), IncrementalOutline)>,
);

impl From<IncrementalOutline> for Outline {
    fn from(value: IncrementalOutline) -> Self {
        Outline(
            value
                .0
                .into_iter()
                .map(|((head,), body)| {
                    ((head.as_ref().to_string(),), Outline::from(body))
                })
                .collect(),
        )
    }
}

/// Print the outline without the partial headline sigils.
impl fmt::Display for IncrementalOutline {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fn print(
            f: &mut fmt::Formatter<'_>,
            depth: usize,
            outline: &IncrementalOutline,
        ) -> fmt::Result {
            for ((h,), b) in &outline.0 {
                for _ in 0..depth {
                    write!(f, "  ")?;
                }
                writeln!(f, "{}", h.as_ref())?;
                print(f, depth + 1, b)?;
            }
            Ok(())
        }

        print(f, 0, self)
    }
}

impl std::ops::AddAssign<&IncrementalOutline> for Outline {
    fn add_assign(&mut self, rhs: &IncrementalOutline) {
        use IncrementalHeadline::*;

        for ((head,), body) in &rhs.0 {
            // Look for the head in current
            if let Some(i) = self
                .0
                .iter()
                .position(|((h,), _)| head.as_ref() == h.as_str())
            {
                if matches!(head, Overwrite(_)) {
                    self.0[i].1 = body.clone().into();
                } else {
                    self.0[i].1 .0.extend(Outline::from(body.clone()).0);
                }
            } else {
                self.0.push(((head.clone().into(),), body.clone().into()));
            }
        }
    }
}

#[derive(
    Clone,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
    Debug,
    SerializeDisplay,
    DeserializeFromStr,
)]
pub enum IncrementalHeadline {
    /// A headline whose contents will replace those found in the previous
    /// outline.
    Overwrite(String),
    /// A headline whose contents will be appended to those in the previous
    /// outline.
    Append(String),
}

impl FromStr for IncrementalHeadline {
    type Err = Box<dyn std::error::Error>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use IncrementalHeadline::*;

        if let Some(s) = s.strip_prefix('@') {
            Ok(Append(s.into()))
        } else {
            Ok(Overwrite(s.into()))
        }
    }
}

impl From<IncrementalHeadline> for String {
    fn from(value: IncrementalHeadline) -> Self {
        value.as_ref().to_owned()
    }
}

impl fmt::Display for IncrementalHeadline {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use IncrementalHeadline::*;

        match self {
            Overwrite(s) => write!(f, "{s}"),
            Append(s) => write!(f, "@{s}"),
        }
    }
}

/// Removes the glyph from append headlines.
impl AsRef<str> for IncrementalHeadline {
    fn as_ref(&self) -> &str {
        use IncrementalHeadline::*;

        match self {
            Overwrite(s) => s,
            Append(s) => s,
        }
    }
}

/// Functions to serialize an Option value so that "-" denotes `None`.
pub mod dash_option {
    use serde::{
        de::DeserializeOwned, Deserialize, Deserializer, Serialize, Serializer,
    };

    pub fn serialize<S, T>(
        val: &Option<T>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
        T: Serialize,
    {
        match val {
            Some(t) => t.serialize(serializer),
            None => "-".serialize(serializer),
        }
    }

    pub fn deserialize<'de, D, T>(
        deserializer: D,
    ) -> Result<Option<T>, D::Error>
    where
        D: Deserializer<'de>,
        T: DeserializeOwned,
    {
        let s = String::deserialize(deserializer)?;

        if s == "-" {
            Ok(None)
        } else {
            idm::from_str::<T>(&s)
                .map_err(serde::de::Error::custom)
                .map(Some)
        }
    }
}
