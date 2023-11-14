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
    pub fn new(map: String, legend: IndexMap<char, T>) -> Self {
        // There might be white space messing with the where content starts,
        // so do some extra work and make sure everything snaps to
        let mut y_skip = 0;
        let mut x_skip = std::usize::MAX;

        let map = map.trim_end();

        if map.is_empty() {
            return AsciiMap {
                map: map.to_owned(),
                legend,
            };
        }

        for line in map.lines() {
            let line = line.trim_end();
            if line.is_empty() {
                y_skip += 1;
                continue;
            }

            x_skip = x_skip
                .min(line.chars().take_while(|c| c.is_whitespace()).count());
        }

        if y_skip > 0 || x_skip > 0 {
            let mut trimmed_map = String::new();
            for line in map.lines().skip(y_skip) {
                debug_assert!(!line.is_empty());

                for c in line.chars().skip(x_skip) {
                    trimmed_map.push(c);
                }
                trimmed_map.push('\n');
            }

            AsciiMap {
                map: trimmed_map,
                legend,
            }
        } else {
            AsciiMap {
                map: map.to_owned(),
                legend,
            }
        }
    }

    /// Iterate the points and legend entries (if present) on the map.
    ///
    /// If any values are returned, at least one is guaranteed to have a
    /// minimum y coordinate of 0 and at least one is guaranteed to have a
    /// minimum x coordinate of 0.
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
        let map = if value.map.trim().is_empty() && !map.trim().is_empty() {
            map
        } else {
            value.map
        };

        AsciiMap::new(map, value.legend)
    }
}

impl<T> From<SerAsciiMap<T>> for AsciiMap<T> {
    fn from(value: SerAsciiMap<T>) -> Self {
        AsciiMap::new(value.map, value.legend)
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
