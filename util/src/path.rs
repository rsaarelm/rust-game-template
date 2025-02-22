use std::{
    cmp::{Ordering, Reverse},
    collections::{BinaryHeap, VecDeque},
    hash::Hash,
    ops::{Add, Sub},
    rc::Rc,
};

use crate::{HashMap, HashSet};
use derive_more::Deref;

// I could pretty much use the Dijkstra stuff from crate pathfinding, but this
// one has the one difference that it lets you do a start set of multiple
// nodes, which is pretty useful when generating autoexploration pathing.

/// Generate a shortest paths map on a grid according to a neighbors function.
pub fn bfs<'a, T, I>(
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

/// Pathfinding result value that lets you trace the path that led to this
/// node.
#[derive(Clone, Eq, PartialEq, Deref)]
pub struct PathNode<T, N>(Rc<(T, N, Option<PathNode<T, N>>)>);

impl<T, N> PathNode<T, N> {
    pub fn new(item: T) -> Self
    where
        N: Default,
    {
        PathNode(Rc::new((item, Default::default(), None)))
    }

    pub fn extend(&self, item: T, cost: N) -> Self
    where
        T: Clone,
        N: Add<Output = N> + Copy,
    {
        PathNode(Rc::new((
            item,
            self.total_cost() + cost,
            Some(self.clone()),
        )))
    }

    pub fn item(&self) -> &T {
        &self.0.0
    }

    pub fn total_cost(&self) -> N
    where
        N: Copy,
    {
        self.0.1
    }

    pub fn parent(&self) -> Option<Self>
    where
        T: Clone,
        N: Copy,
    {
        self.0.2.clone()
    }

    pub fn iter(&self) -> impl Iterator<Item = (T, N)> + '_
    where
        T: Clone,
        N: Copy + Default + Sub<Output = N>,
    {
        let mut node = Some(self.clone());
        std::iter::from_fn(move || {
            let n = node.take()?;
            let ret = (
                n.0.0.clone(),
                n.0.1 - n.0.2.as_ref().map_or_else(Default::default, |p| p.0.1),
            );
            node = n.0.2.clone();
            Some(ret)
        })
    }
}

impl<T: Eq + PartialEq, N: Copy + PartialOrd + Ord> Ord for PathNode<T, N> {
    // Ordering for BinaryHeap, smallest cost comes first.
    fn cmp(&self, other: &Self) -> Ordering {
        Reverse(self.total_cost()).cmp(&Reverse(other.total_cost()))
    }
}

impl<T: Eq + PartialEq, N: Copy + PartialOrd + Ord> PartialOrd
    for PathNode<T, N>
{
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Do expanding search on a weighted graph, keeping track of the paths the
/// search nodes traversed. This is particularly useful if you need to find
/// multiple equal paths between points.
pub fn dijkstra_search<'a, T, I, N>(
    neighbors: impl Fn(&T) -> I + 'a,
    start: &T,
) -> impl Iterator<Item = PathNode<T, N>> + 'a
where
    T: Clone + Eq + Hash + 'a,
    I: IntoIterator<Item = (T, N)>,
    N: Default
        + Sub<Output = N>
        + Add<Output = N>
        + Copy
        + PartialOrd
        + Ord
        + 'a,
{
    let mut seen = HashMap::default();
    let mut edge = BinaryHeap::from([PathNode::new(start.clone())]);
    std::iter::from_fn(move || {
        while let Some(node) = edge.pop() {
            if matches!(seen.get(node.item()), Some(&cost) if cost < node.total_cost())
            {
                continue;
            }
            seen.insert(node.item().clone(), node.total_cost());

            for (item, cost) in neighbors(node.item()).into_iter() {
                edge.push(node.extend(item, cost));
            }
            return Some(node);
        }
        None
    })
}
