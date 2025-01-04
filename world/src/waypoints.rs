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

use std::collections::BinaryHeap;

use serde::{Deserialize, Serialize};
use util::{HashMap, HashSet};

use crate::{Level, World, LEVEL_BASIS};

impl World {
    /// Return a list of sectors that will have changes permanently applied to
    /// them when the player started from waypoint `a` and stops at waypoint
    /// `b`. The waypoints must correspond to valid altars and be different,
    /// or the result will be empty.
    ///
    /// If the set of sectors where both waypoints are within the
    /// second-closest distance to that sector is non-empty, the waypoints are
    /// considered to be connected and this set is returned as the result.
    /// Otherwise, the result will be the union of the affected areas of all
    /// pairs of connected waypoints that form the shortest paths between `a`
    /// and `b`.
    pub fn area_between_waypoints(&self, a: Level, b: Level) -> HashSet<Level> {
        self.shortest_paths_between_waypoints(a, b)
            .into_iter()
            .flat_map(|p| &self.segment_cover[&p])
            .copied()
            .collect()
    }

    /// Return all the adjacent waypoint to waypoint connections that for
    /// every shortest path between waypoints a and b.
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
                for ((a, _), (b, _)) in n.into_iter().zip(n.into_iter().skip(1))
                {
                    ret.insert(WaypointPair::new(a, b));
                }
            }
        }

        ret
    }

    fn compute_segment_cover(&self) -> HashMap<WaypointPair, Vec<Level>> {
        /// Special structure for iterating neighbors of each waypoint only up
        /// until they get blocked by other waypoints' domains. The cover
        /// areas will get collected in the structure.
        #[derive(Default)]
        struct CoverMap {
            // XXX: Can't use Level type as BinaryHeap value since it isn't
            // Ord. Use the min corner array instead.
            //
            // Store negative distance values (sign-flipped) so that the top
            // of the heap will have the smallest distances.
            cover: HashMap<Level, BinaryHeap<(isize, [i32; 3])>>,
            // XXX: Need to replicate distances here since bfs won't pass them
            // to the neighbors function.
            distances: HashMap<(Level, Level), isize>,
        }

        impl CoverMap {
            /// Return the maximum distance from which a node can reach the
            /// given position.
            fn max_dist(&self, pos: Level) -> isize {
                // The heap must have at least two items and the maximum
                // distance for adding new items is always the distance of the
                // second item. Either it has two items at the same distance
                // or it has one at the minimum distance and one or more at
                // the second closest distance.
                if let Some(&(dist, _)) =
                    self.cover.get(&pos).and_then(|elts| elts.iter().nth(1))
                {
                    -dist
                } else {
                    isize::MAX
                }
            }

            pub fn neighbors(
                &mut self,
                world: &World,
                origin: Level,
                pos: Level,
            ) -> Vec<(Level, Level)> {
                let my_dist = *self.distances.get(&(origin, pos)).unwrap_or(&0);
                let mut ret = Vec::new();

                // Thing that can be inserted into BinaryHeap.
                let origin_key = origin.min();

                // Get the neighbors to current node.
                for lev in world.level_neighbors(pos) {
                    if self.distances.contains_key(&(origin, lev)) {
                        // Skip nodes we've already visited.
                        continue;
                    }

                    let dist = my_dist + 1;
                    // How distant nodes can affect this waypoint?
                    let max_dist = self.max_dist(lev);

                    if dist <= max_dist {
                        // We're close enough to make a difference, add origin
                        // to the node.
                        self.cover
                            .entry(lev)
                            .or_default()
                            .push((-dist, origin_key));
                        self.distances.insert((origin, lev), dist);
                        // Progress wasn't blocked, so this counts as a
                        // neighbor.
                        ret.push((origin, lev));
                    }
                }
                ret
            }
        }

        let waypoints: Vec<Level> = self
            .skeleton
            .iter()
            .filter_map(|(lev, seg)| seg.has_waypoint().then_some(lev))
            .copied()
            .collect();

        let mut cover = CoverMap::default();

        for _ in util::bfs(
            |&(origin, pos)| cover.neighbors(self, origin, pos),
            waypoints.iter().map(|&lev| (lev, lev)),
        ) {
            // Do nothing, just run the bfs. The data we want will be
            // collected as a side effect of the neighbors function in cover.
        }

        // Strip out distances in cover.cover that are further out than the
        // second-shortest one. (I'm not sure if this can actually happen
        // because of the node expansion order, but let's be paranoid and
        // cover it anyway.)
        for heap in cover.cover.values_mut() {
            let min_dist = heap
                .iter()
                .nth(1)
                .map(|&(dist, _)| dist)
                .unwrap_or(isize::MIN);
            heap.retain(|&(dist, _)| dist >= min_dist);
        }

        let mut ret: HashMap<WaypointPair, Vec<Level>> = HashMap::default();

        for (level, waypoints) in cover.cover {
            let waypoints = waypoints.into_vec();
            for i in 0..waypoints.len() {
                for j in i + 1..waypoints.len() {
                    // Reconstruct the level values we had to mangle into
                    // arrays to make them Ord.
                    let a = Level::sized(LEVEL_BASIS) + waypoints[i].1;
                    let b = Level::sized(LEVEL_BASIS) + waypoints[j].1;

                    ret.entry(WaypointPair::new(a, b)).or_default().push(level);
                }
            }
        }

        ret
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

        log::info!("Constructed waypoint geometry, {total_waypoints} waypoints covering {covered_segments} / {total_segments} segments.");

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
.....
...*.
.....
.#...
.....",
            "\
.....
..##.
.....
.#...
.....",
            "\
..##.
#####
*##*#
*###.
.##..",
            "\
.....
#..#.
.....
.....
.....",
            "\
..*#*
#.###
#.*#*
*....
.....",
        ]
        .iter()
        .enumerate()
        {
            let z = -(z as i32) + 2;
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
        for z in (-2..=2).rev() {
            writeln!(s, "{z}");
            for y in 0..5 {
                for x in 0..5 {
                    let lev = Level::level_at([x, y, z]).min();

                    let mut connected_to: String = effecters
                        .get(&lev)
                        .unwrap_or(&BTreeSet::default())
                        .iter()
                        .map(|w| chars[&w])
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

        assert_eq!(
            s.trim(),
            "\
2
       -       -       -       -       -
       -       -       -      FG       -
       -       -       -       -       -
       -      AC       -       -       -
       -       -       -       -       -

1
       -       -       -       -       -
       -       -      FG      FG       -
       -       -       -       -       -
       -      AC       -       -       -
       -       -       -       -       -

0
       -       -     AFG      FG       -
      AC     ACG     AFG      FG      FG
      AC     ACG      AG     AFG     AFG
      AC      AC      CG      CG       -
       -      AC      CG       -       -

-1
       -       -       -       -       -
     ABC       -       -  DEFGHI       -
       -       -       -       -       -
       -       -       -       -       -
       -       -       -       -       -

-2
       -       -     DEH      DH     DHI
      AB       -      DE    DEHI      HI
      AB       -     DEI      EI     EHI
      AB       -       -       -       -
       -       -       -       -       -"
        );
    }
}
