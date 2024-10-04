use std::{collections::VecDeque, hash::Hash};

use crate::HashSet;

// I could pretty much use the Dijkstra stuff from crate pathfinding, but this
// one has the one difference that it lets you do a start set of multiple
// nodes, which is pretty useful when generating autoexploration pathing.

/// Generate a shortest paths map on a grid according to a neighbors function.
pub fn dijkstra_map<'a, T, I>(
    mut neighbors: impl FnMut(&T) -> I + 'a,
    starts: impl IntoIterator<Item = T>,
) -> impl Iterator<Item = (T, usize)> + 'a
where
    T: Clone + Eq + Hash + 'a,
    I: IntoIterator<Item = T>,
{
    let mut edge: VecDeque<(T, usize)> =
        starts.into_iter().map(|s| (s, 0)).collect();
    let mut seen = HashSet::default();

    std::iter::from_fn(move || {
        // Candidates are in a queue and consumed first-in, first-out. This
        // should guarantee that the first time a node is popped from the queue
        // it shows the shortest path length from start to that node.

        while let Some((node, len)) = edge.pop_front() {
            if !seen.contains(&node) {
                seen.insert(node.clone());
                for n in neighbors(&node) {
                    edge.push_back((n, len + 1));
                }
                return Some((node, len));
            }
        }
        None
    })
}
