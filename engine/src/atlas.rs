use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{prelude::*, Rect};

/// Type for representing a space as a set of terrain patches.
///
/// Intended for serialization.
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct Atlas(BTreeMap<Location, String>);

impl<A: Into<char> + Default + Eq> FromIterator<(Location, A)> for Atlas {
    fn from_iter<T: IntoIterator<Item = (Location, A)>>(iter: T) -> Self {
        let mut points: HashMap<Location, HashMap<IVec2, char>> =
            HashMap::default();
        // Collect character clouds.
        let nil = A::default();
        for (loc, c) in iter.into_iter().filter(|(_, c)| *c != nil) {
            let c: char = c.into();
            let sector = loc.sector();
            let bin = points.entry(sector).or_default();
            bin.insert((loc - sector).truncate(), c);
        }

        // Build string patches from the point clouds and map them by
        // location.
        let mut bins = BTreeMap::new();
        for (sector, bin) in points {
            let bounds = Rect::from_points_inclusive(bin.keys().copied());

            let mut s = String::new();
            for y in bounds.min()[1]..bounds.max()[1] {
                for x in bounds.min()[0]..bounds.max()[0] {
                    // Use NBSP as the filler char.
                    let c =
                        bin.get(&ivec2(x, y)).copied().unwrap_or('\u{00a0}');
                    s.push(c);
                }
                s.push('\n');
            }
            bins.insert(sector + IVec2::from(bounds.min()), s);
        }

        Atlas(bins)
    }
}

impl Atlas {
    pub fn iter(&self) -> impl Iterator<Item = (Location, char)> + '_ {
        self.0.iter().flat_map(move |(loc, text)| {
            text.lines().enumerate().flat_map(move |(y, line)| {
                line.chars()
                    .enumerate()
                    .filter(|(_, c)| !c.is_whitespace())
                    .map(move |(x, c)| (*loc + ivec2(x as i32, y as i32), c))
            })
        })
    }
}

/// Type for representing a space as a set of bit fields.
///
/// Intended for serialization.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BitAtlas(BTreeMap<Location, String>);

impl FromIterator<Location> for BitAtlas {
    fn from_iter<T: IntoIterator<Item = Location>>(iter: T) -> Self {
        let mut points: HashMap<Location, HashSet<IVec2>> = HashMap::default();
        // Collect character clouds.
        for loc in iter.into_iter() {
            let sector = loc.sector();
            let bin = points.entry(sector).or_default();
            bin.insert((loc - sector).truncate());
        }

        // Build string patches from the point clouds and map them by
        // location.
        let mut bins = BTreeMap::new();
        for (sector, bin) in points {
            let bounds = Rect::from_points_inclusive(bin.iter().copied());
            let pixel_w = (bounds.width() + 1) / 2;
            let pixel_h = (bounds.height() + 3) / 4;

            let mut s = String::new();
            for v in 0..pixel_h {
                for u in 0..pixel_w {
                    let origin =
                        ivec2(u * 2, v * 4) + IVec2::from(bounds.min());
                    let mut bits = 0;
                    for p in Rect::sized([2, 4]) {
                        let p = IVec2::from(p);
                        if bin.contains(&(origin + p)) {
                            let bit = BRAILLE_OFFSETS
                                .iter()
                                .position(|&i| i == p)
                                .unwrap();
                            bits |= 1 << bit;
                        }
                    }
                    s.push(char::from_u32(0x2800 + bits).unwrap());
                }
                s.push('\n');
            }
            bins.insert(sector + IVec2::from(bounds.min()), s);
        }

        BitAtlas(bins)
    }
}

impl BitAtlas {
    /// Iterate a compressed bitmap atlas that encodes points in braille
    /// pseudopixels.
    pub fn iter(&self) -> impl Iterator<Item = Location> + '_ {
        self.0.iter().flat_map(move |(loc, text)| {
            text.lines().enumerate().flat_map(move |(y, line)| {
                line.chars()
                    .enumerate()
                    .filter(|(_, c)| !c.is_whitespace())
                    .flat_map(move |(x, c)| {
                        let origin = *loc + ivec2(x as i32 * 2, y as i32 * 4);
                        let c = c as u32;
                        let bits = if (0x2800..0x2900).contains(&c) {
                            c - 0x2800
                        } else {
                            0xff
                        };
                        (0..8).filter_map(move |p| {
                            (bits & (1 << p) != 0)
                                .then_some(origin + BRAILLE_OFFSETS[p as usize])
                        })
                    })
            })
        })
    }
}

const BRAILLE_OFFSETS: [IVec2; 8] = [
    ivec2(0, 0),
    ivec2(0, 1),
    ivec2(0, 2),
    ivec2(1, 0),
    ivec2(1, 1),
    ivec2(1, 2),
    ivec2(0, 3),
    ivec2(1, 3),
];

#[cfg(test)]
mod test {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn braille_atlas() {
        const BOB: &str = "\
#########                    ##########
#######      #  #  ## # # # #   #######
#####                  #          #####
####                        ###    ####
####     #########    #########  #  ###
####     ######################  #  ###
####     ####################### #  ###
####     ####################### # ####
####            #######        #    ###
####              ###        # ##  ####
####    #######   #####    ### ##  ####
####   ########## ###############  ####
##### ## #######  #####################
########  ###   ######    #### # ######
########             #####     ########
######## # ##  #       #  ####  #######
######### # ##   ###### #####  ########
##########  #     ##########  #########
###########   #   ## ######  ##########
### ##           ########  ############
##   #     ###           ##############
###      ##############################
#######################################";

        for (dx, dy) in [(10, 10), (-20, -10)] {
            let points: BTreeSet<Location> = BOB
                .lines()
                .enumerate()
                .flat_map(move |(y, line)| {
                    line.chars().enumerate().flat_map(move |(x, c)| {
                        (!c.is_whitespace()).then_some(Location::new(
                            x as i16 + dx,
                            y as i16 + dy,
                            0,
                        ))
                    })
                })
                .collect();

            let atlas = BitAtlas::from_iter(points.iter().cloned());
            eprint!("{}", idm::to_string(&atlas).unwrap());
            let roundtrip: BTreeSet<Location> = atlas.iter().collect();
            assert!(points == roundtrip);
        }
    }
}
