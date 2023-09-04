use std::{
    borrow::Borrow,
    fs::{self, File},
    io::{self, prelude::*},
    path::Path,
};

use anyhow::Result;
use derive_deref::Deref;
use serde::{Deserialize, Serialize};

/// Dump a directory tree into a single IDM expression.
pub fn directory_to_idm(path: impl AsRef<Path>) -> Result<String> {
    use std::fmt::Write;

    // If pointed at a file, just read the file.
    if path.as_ref().is_file() {
        return Ok(fs::read_to_string(path)?);
    }

    let mut ret = String::new();
    for e in walkdir::WalkDir::new(path)
        .sort_by_key(|a| a.file_name().to_ascii_lowercase())
    {
        let e = e.expect("read_path failed");
        let depth = e.depth();
        if depth == 0 {
            // The root element, do not print out.
            continue;
        }
        for _ in 1..depth {
            write!(ret, "  ")?;
        }
        let is_dir = e.file_type().is_dir();
        if is_dir {
            writeln!(ret, "{}", e.file_name().to_string_lossy())?;
        } else {
            let path = Path::new(e.file_name());

            if !matches!(
                path.extension()
                    .map(|a| a.to_str().unwrap_or(""))
                    .unwrap_or(""),
                "idm"
            ) {
                // Only read IDM files.
                continue;
            }

            let name = path
                .file_stem()
                .expect("read_path failed")
                .to_string_lossy();
            writeln!(ret, "{}", name)?;

            // Print lines
            let file =
                File::open(e.path()).expect("read_path: Open file failed");
            for line in io::BufReader::new(file).lines() {
                let line = line.expect("read_path failed");
                let mut ln = &line[..];
                let mut depth = depth;
                // Turn tab indentation into spaces.
                while ln.starts_with('\t') {
                    depth += 1;
                    ln = &ln[1..];
                }
                for _ in 1..(depth + 1) {
                    write!(ret, "  ")?;
                }
                writeln!(ret, "{ln}")?;
            }
        }
    }

    Ok(ret)
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
