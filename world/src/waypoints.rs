//! Logic for determining which waypoints affect which levels of a world.
//!
//! The core waypoint mechanics:
//!
//! If you die, you respawn at the last waypoint you rested at, with all the
//! regular enemies you damaged or killed restored to full health. Bosses are
//! exceptions that stay dead no matter what after they've been defeated the
//! first time.
//!
//! If you return to rest at the same waypoint you last rested at, enemies
//! will respawn similarly as if you had died. This is to prevent the player
//! from clearing areas by slow attrition where they go back to rest after
//! killing each individual enemy and never need to face the area at full
//! strength while minding their own limited resources.
//!
//! If you start at one waypoint and rest at a different one though, any
//! changes made *in the area between the two waypoints* will be permanent. So
//! it is possible to eventually clear up areas, as long as you're able to
//! actually travel through them without resting.
//!
//! This module is about the logic to figure out just how the "area between
//! two waypoints" is determined.

use std::{cmp::Reverse, collections::BinaryHeap};

use itertools::Itertools;
use serde::{Deserialize, Serialize};
use util::{HashMap, HashSet, v3};

use crate::{Level, World};

impl World {
    /// Return a list of sectors that will have changes permanently applied to
    /// them when the player started from waypoint `a` and stops at waypoint
    /// `b`. The waypoints must correspond to valid altars and be different,
    /// or the result will be empty.
    ///
    /// If there is a nonempty set of levels directly covered by the two
    /// waypoints given, the two waypoints are considered connected and this
    /// set will be returned as the result. Otherwise the result will the the
    /// union of the level sets of all the of connected waypoint pairs that
    /// form every shortest path between `a` and `b`.
    pub fn area_between_waypoints(&self, a: Level, b: Level) -> HashSet<Level> {
        self.shortest_paths_between_waypoints(a, b)
            .into_iter()
            .flat_map(|p| &self.segment_cover[&p])
            .copied()
            .collect()
    }

    /// Return all the adjacent waypoint to waypoint connections for every
    /// shortest path between waypoints a and b.
    fn shortest_paths_between_waypoints(
        &self,
        a: Level,
        b: Level,
    ) -> HashSet<WaypointPair> {
        // Start from set of waypoint_connections for a.
        // If b is found in this set, just return the pair.
        //
        // Return all pairs that are on a path from a to be such that no
        // shorter path exists connecting a and b.

        let mut ret = HashSet::default();

        let mut finishing_distance = usize::MAX;
        for n in util::dijkstra_search(
            |&p| {
                self.waypoint_graph
                    .get(&p)
                    .cloned()
                    .unwrap_or_default()
                    .into_iter()
                    .map(|p| (p, 1))
            },
            &a,
        ) {
            // We're past the shortest distance that reached target, exit.
            if n.total_cost() > finishing_distance {
                break;
            }

            // While we haven't gone over the shortest distance, add the nodes
            // of every path that reaches the target to the result set. If
            // there are multiple equally long shortest paths to target, we
            // need to cover them all.
            if n.item() == &b {
                finishing_distance = n.total_cost();
                for ((a, _), (b, _)) in n.iter().zip(n.iter().skip(1)) {
                    ret.insert(WaypointPair::new(a, b));
                }
            }
        }

        ret
    }

    /// Compute the closest waypoints covering individual levels.
    ///
    /// This is the trickiest part of the system. It tries to find the two
    /// nearest waypoints (along traversable level paths, not direct distance)
    /// to each level and pick these as the waypoints affecting this level. If
    /// there are more than two waypoints at equal distances, they must all be
    /// picked. Also, waypoints that are behind other waypoints are avoided in
    /// favor of unoccluded waypoints, even if the unoccluded waypoints are
    /// much further away.
    fn compute_segment_cover(&self) -> HashMap<WaypointPair, HashSet<Level>> {
        // Return whether waypoint a occludes waypoint b when viewed from the
        // view position `pos`.
        fn occludes(a: Level, b: Level, pos: Level) -> bool {
            // Convert to float vectors.
            let (a, b, pos) = (
                v3(a.min()).as_vec3(),
                v3(b.min()).as_vec3(),
                v3(pos.min()).as_vec3(),
            );

            // a occludes b if b is behind a plane that goes through a and
            // whose normal points from a to viewpoint.
            (pos - a).dot(b - a) < 0.0
        }

        let mut waypoints: Vec<Level> = self
            .skeleton
            .iter()
            .filter_map(|(lev, seg)| seg.has_waypoint().then_some(lev))
            .copied()
            .collect();
        // Remove randomness from iterating the skeleton HashMap just in case.
        waypoints.sort_by_key(|a| a.min());

        // Compute Dijkstra map for distance from each waypoint.
        let mut distance_maps: HashMap<Level, Vec<usize>> = HashMap::default();
        for ((idx, lev), n) in util::bfs(
            |&(idx, lev)| self.level_neighbors(lev).map(move |n| (idx, n)),
            waypoints.iter().copied().enumerate(),
        ) {
            distance_maps
                .entry(lev)
                .or_insert_with(|| vec![usize::MAX; waypoints.len()])[idx] = n;
        }

        let mut cover: HashMap<Level, Vec<WaypointPair>> = HashMap::default();

        // Determine most relevant waypoints for every level.
        for &lev in self.skeleton.keys() {
            // Find the closest waypoints to current level, preferring
            // unoccluded ones. A far-away unoccluded waypoint is better than
            // a nearby occluded one.
            let mut closest = BinaryHeap::new();
            for (idx, &a) in waypoints.iter().enumerate() {
                let dist = distance_maps[&lev][idx];
                let is_occluded = a != lev
                    && waypoints.iter().enumerate().any(|(j, &b)| {
                        // Extra check, because we're measuring path distance
                        // instead of direct distance to nodes, a node can
                        // show as occluding even if the path to it is longer
                        // than the node it occludes. Reject the occluder as
                        // irrelevant in this case.
                        distance_maps[&lev][j] < dist && occludes(b, a, lev)
                    });
                closest.push((Reverse(is_occluded), Reverse(dist), idx));
            }

            // Get two or more valid waypoints from the closest list. Always
            // take the first one, then keep taking more as long as you keep
            // getting non-occluded ones that are not further than the second
            // one one you got.

            let mut connected_waypoints =
                vec![waypoints[closest.pop().unwrap().2]];
            let mut dist = usize::MAX;
            while let Some((Reverse(occluded), Reverse(d), idx)) = closest.pop()
            {
                if d <= dist && (connected_waypoints.len() < 2 || !occluded) {
                    connected_waypoints.push(waypoints[idx]);
                    dist = d;
                } else {
                    break;
                }
            }

            // Use Itertools to iterate all pairs fron connected_waypoints
            // and insert them into the cover map as WaypointPair values.
            for (a, b) in connected_waypoints.iter().tuple_combinations() {
                cover
                    .entry(lev)
                    .or_default()
                    .push(WaypointPair::new(*a, *b));
            }
        }

        let mut result: HashMap<WaypointPair, HashSet<Level>> =
            HashMap::default();
        for (lev, ps) in cover {
            for pair in ps {
                result.entry(pair).or_default().insert(lev);
            }
        }

        // Make sure are pairs cover the corresponding waypoint levels.
        for (pair, levs) in result.iter_mut() {
            levs.insert(pair.0);
            levs.insert(pair.1);
        }

        result
    }

    /// Construct the cached segment cover and waypoint graph from current
    /// skeleton.
    pub(crate) fn construct_waypoint_geometry(&mut self) {
        // Build waypoint cover for individual levels.
        self.segment_cover = self.compute_segment_cover();

        // Build connected waypoint graph based on which pairs of waypoints
        // are seen influencing the same level.
        self.waypoint_graph = HashMap::default();
        for k in self.segment_cover.keys() {
            self.waypoint_graph.entry(k.0).or_default().push(k.1);
            self.waypoint_graph.entry(k.1).or_default().push(k.0);
        }

        // Print some diagnostics.
        let total_segments = self.skeleton.len();
        let total_waypoints = self
            .segment_cover
            .keys()
            .flat_map(|p| [p.0, p.1])
            .collect::<HashSet<_>>()
            .len();
        let covered_segments = self
            .segment_cover
            .values()
            .flatten()
            .collect::<HashSet<_>>()
            .len();

        log::info!(
            "Constructed waypoint geometry, {total_waypoints} waypoints covering {covered_segments} / {total_segments} segments."
        );

        // Print more detailed diagnostics.
        if log::log_enabled!(log::Level::Debug) {
            for (pair, levs) in &self.segment_cover {
                log::debug!(
                    "{:?} x {:?} covers {:?}",
                    pair.0.lattice_point(),
                    pair.1.lattice_point(),
                    levs.iter().map(|a| a.lattice_point()).collect::<Vec<_>>()
                );
            }
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct WaypointPair(Level, Level);

impl WaypointPair {
    pub fn new(a: Level, b: Level) -> Self {
        // Normalize the order of the points.
        if a.min() < b.min() {
            WaypointPair(a, b)
        } else {
            WaypointPair(b, a)
        }
    }
}

#[cfg(test)]
mod test {
    use std::collections::BTreeSet;

    use util::StrExt;

    use crate::{MapGenerator, Zone};

    use super::*;

    struct DummyGenerator(bool);

    impl MapGenerator for DummyGenerator {
        fn run(
            &self,
            _rng: &mut dyn rand::RngCore,
            _lot: &crate::Lot,
        ) -> anyhow::Result<crate::Patch> {
            // No-op, these won't be actually run.
            Ok(Default::default())
        }

        fn has_waypoint(&self) -> bool {
            // We only care about them for marking waypoints in the skeleton.
            self.0
        }
    }

    fn test_world() -> World {
        let mut ret = World::default();

        for (z, layer) in [
            "\
*.*#.
#.##*
#.###
#.###
*.##.
*##..",
            "\
.....
#..#.
.....
.....
.....
.....",
            "\
...#*
#.###
#.*#.
#....
*....
.....",
        ]
        .iter()
        .enumerate()
        {
            let z = -(z as i32);
            for (p, c) in layer.char_grid() {
                let lev = Level::level_at(p.extend(z));
                match c {
                    '*' => {
                        ret.skeleton.insert(lev, DummyGenerator(true).into());
                    }
                    '#' => {
                        ret.skeleton.insert(lev, DummyGenerator(false).into());
                    }
                    _ => {}
                }
            }
        }

        ret.construct_waypoint_geometry();

        ret
    }

    #[test]
    fn waypoint_cover() {
        use pretty_assertions::assert_eq;
        use std::fmt::Write;
        use util::{write, writeln};

        let world = test_world();

        // Figure out how to visualize this mess...
        let mut waypoints: Vec<[i32; 3]> =
            world.waypoint_graph.keys().map(|a| a.min()).collect();
        waypoints.sort();

        let chars = waypoints.iter().zip('A'..).collect::<HashMap<_, _>>();

        let mut effecters: HashMap<[i32; 3], BTreeSet<[i32; 3]>> =
            HashMap::default();
        for (pair, levs) in &world.segment_cover {
            for lev in levs {
                let lev = lev.min();
                effecters.entry(lev).or_default().insert(pair.0.min());
                effecters.entry(lev).or_default().insert(pair.1.min());
            }
        }

        let mut s = String::new();
        for z in (-2..=0).rev() {
            writeln!(s, "{z}");
            for y in 0..6 {
                for x in 0..5 {
                    let lev = Level::level_at([x, y, z]).min();

                    let mut connected_to: String = effecters
                        .get(&lev)
                        .unwrap_or(&BTreeSet::default())
                        .iter()
                        .map(|w| {
                            if w == &lev {
                                chars[&w]
                            } else {
                                chars[&w].to_ascii_lowercase()
                            }
                        })
                        .collect();

                    if connected_to.is_empty() {
                        connected_to = "-".to_string();
                    }

                    write!(s, "{connected_to:>8}");
                }
                writeln!(s);
            }
            writeln!(s);
        }

        eprintln!("{s}");

        assert_eq!(
            s.trim(),
            "\
0
     Abc       -  cdEfgh      eh       -
      ac       -      eh      eh   defgH
      ac       -      eh      eh      fh
      ac       -     deh      eh      dh
   abCde       -     cde      dh       -
    cDeh      cd      cd       -       -

-1
       -       -       -       -       -
     abc       -       -    efgh       -
       -       -       -       -       -
       -       -       -       -       -
       -       -       -       -       -
       -       -       -       -       -

-2
       -       -       -      fg    efGh
      ab       -      fg      fg      fg
      ab       -    eFgh      fg       -
      ab       -       -       -       -
     aBc       -       -       -       -
       -       -       -       -       -"
        );
    }
}
