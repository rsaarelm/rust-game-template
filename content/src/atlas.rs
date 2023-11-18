use std::collections::BTreeMap;

use glam::{ivec2, IVec2, IVec3};
use serde::{Deserialize, Serialize};
use util::{v2, v3, HashMap, HashSet};

use crate::{LocExt, Rect};

// Use [i32; 3] as key since it implements Ord, unlike IVec3.

/// Type for representing a space as a set of terrain patches.
///
/// Intended for serialization.
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct Atlas(BTreeMap<[i32; 3], String>);

impl<A: Into<char> + Default + Eq, K: Into<IVec3>> FromIterator<(K, A)>
    for Atlas
{
    fn from_iter<T: IntoIterator<Item = (K, A)>>(iter: T) -> Self {
        let mut points: HashMap<IVec3, HashMap<IVec2, char>> =
            HashMap::default();
        // Collect character clouds into sector slice bins.
        let nil = A::default();
        for (loc, c) in iter.into_iter().filter(|(_, c)| *c != nil) {
            let loc = loc.into();
            let c: char = c.into();
            let slice = loc.sector_snap_2d();
            let bin = points.entry(slice).or_default();
            // 2D position of point inside its bin.
            let pos = (loc - slice).truncate();
            bin.insert(pos, c);
        }

        // Build string patches from the point clouds and map them by
        // location.
        let mut bins = BTreeMap::new();
        for (slice, bin) in points {
            let bounds = Rect::from_points_inclusive(bin.keys().copied());

            let mut s = String::new();
            for y in bounds.min()[1]..bounds.max()[1] {
                for x in bounds.min()[0]..bounds.max()[0] {
                    // Use NBSP as the filler char so whitespace in the left
                    // side of the map doesn't read as indentation to IDM.
                    let c =
                        bin.get(&ivec2(x, y)).copied().unwrap_or('\u{00a0}');
                    s.push(c);
                }
                s.push('\n');
            }
            let origin = slice + v2(bounds.min()).extend(0);
            bins.insert(origin.into(), s);
        }

        Atlas(bins)
    }
}

impl Atlas {
    pub fn iter(&self) -> impl Iterator<Item = (IVec3, char)> + '_ {
        self.0.iter().flat_map(move |(loc, text)| {
            text.lines().enumerate().flat_map(move |(y, line)| {
                line.chars()
                    .enumerate()
                    .filter(|(_, c)| !c.is_whitespace())
                    .map(move |(x, c)| {
                        (v3(*loc) + ivec2(x as i32, y as i32).extend(0), c)
                    })
            })
        })
    }
}

/// Type for representing a space as a set of bit fields.
///
/// Intended for serialization.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BitAtlas(BTreeMap<[i32; 3], String>);

impl FromIterator<IVec3> for BitAtlas {
    fn from_iter<T: IntoIterator<Item = IVec3>>(iter: T) -> Self {
        let mut points: HashMap<IVec3, HashSet<IVec2>> = HashMap::default();
        // Collect character clouds.
        for loc in iter.into_iter() {
            let slice = loc.sector_snap_2d();
            let bin = points.entry(slice).or_default();
            bin.insert((loc - slice).truncate());
        }

        // Build string patches from the point clouds and map them by
        // location.
        let mut bins = BTreeMap::new();
        for (slice, bin) in points {
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
            let origin = slice + v2(bounds.min()).extend(0);
            bins.insert(origin.into(), s);
        }

        BitAtlas(bins)
    }
}

impl BitAtlas {
    /// Iterate a compressed bitmap atlas that encodes points in braille
    /// pseudopixels.
    pub fn iter(&self) -> impl Iterator<Item = IVec3> + '_ {
        self.0.iter().flat_map(move |(loc, text)| {
            let loc = v3(*loc);
            text.lines().enumerate().flat_map(move |(y, line)| {
                line.chars()
                    .enumerate()
                    .filter(|(_, c)| !c.is_whitespace())
                    .flat_map(move |(x, c)| {
                        let origin =
                            loc + ivec2(x as i32 * 2, y as i32 * 4).extend(0);
                        let c = c as u32;
                        let bits = if (0x2800..0x2900).contains(&c) {
                            c - 0x2800
                        } else {
                            0xff
                        };
                        (0..8).filter_map(move |p| {
                            (bits & (1 << p) != 0).then_some(
                                origin + BRAILLE_OFFSETS[p as usize].extend(0),
                            )
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
    use glam::ivec3;

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
            let points: HashSet<IVec3> = BOB
                .lines()
                .enumerate()
                .flat_map(move |(y, line)| {
                    line.chars().enumerate().flat_map(move |(x, c)| {
                        (!c.is_whitespace()).then_some(ivec3(
                            x as i32 + dx,
                            y as i32 + dy,
                            0,
                        ))
                    })
                })
                .collect();

            let atlas = BitAtlas::from_iter(points.iter().cloned());
            eprint!("{}", idm::to_string(&atlas).unwrap());
            let roundtrip: HashSet<IVec3> = atlas.iter().collect();
            assert!(points == roundtrip);
        }
    }
}
