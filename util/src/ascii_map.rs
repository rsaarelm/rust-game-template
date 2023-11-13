use glam::{ivec2, IVec2};
use serde::{Deserialize, Serialize};

use crate::IndexMap;

/// Ascii maps that can be deserialized from an IDM format where the map data
/// is not indented.
#[derive(Clone, Default, Debug, Serialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct AsciiMap<T> {
    map: String,
    legend: IndexMap<char, T>,
}

// You could make an explicit iterator type here so you can impl IntoIterator
// for AsciiMap<T> and then do `for x in &map` instead of `for x in
// map.iter()`, but cranking out the string walking into an explicit iterator
// would be annoying so I'll just go with this for now.

impl<T> AsciiMap<T> {
    /// Iterate the points and legend entries (if present) on the map.
    pub fn iter(&self) -> impl Iterator<Item = (IVec2, char, Option<&T>)> + '_ {
        self.map
            .trim()
            .lines()
            .enumerate()
            .flat_map(move |(y, line)| {
                line.chars()
                    .enumerate()
                    .filter(|(_, c)| !c.is_whitespace())
                    .map(move |(x, c)| {
                        (ivec2(x as i32, y as i32), c, self.legend.get(&c))
                    })
            })
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for AsciiMap<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let data = <((SerAsciiMap<T>,), String)>::deserialize(deserializer)?;
        Ok(AsciiMap::from(data))
    }
}

impl<T> From<((SerAsciiMap<T>,), String)> for AsciiMap<T> {
    fn from(((value,), map): ((SerAsciiMap<T>,), String)) -> Self {
        let mut ret = AsciiMap::from(value);

        if !map.trim().is_empty() && ret.map.trim().is_empty() {
            ret.map = map;
        }

        ret
    }
}

impl<T> From<SerAsciiMap<T>> for AsciiMap<T> {
    fn from(value: SerAsciiMap<T>) -> Self {
        AsciiMap {
            map: value.map,
            legend: value.legend,
        }
    }
}

#[derive(Deserialize)]
#[serde(default, rename_all = "kebab-case")]
struct SerAsciiMap<T> {
    map: String,
    legend: IndexMap<char, T>,
}

// XXX: derive(Default) didn't work without the unnecessary assertion that T
// is Default.
impl<T> Default for SerAsciiMap<T> {
    fn default() -> Self {
        SerAsciiMap {
            map: Default::default(),
            legend: Default::default(),
        }
    }
}
