use std::fmt::Debug;

use num_traits::{AsPrimitive, Euclid, FromPrimitive};

use crate::{AxisBox, Element, HashMap, HashSet};

pub trait GridItem<T, const N: usize> {
    fn as_bounds(&self) -> &AxisBox<T, N>;
}

impl<T, const N: usize> GridItem<T, N> for AxisBox<T, N> {
    fn as_bounds(&self) -> &AxisBox<T, N> {
        self
    }
}

/// A spatial index container for storing a large number of objects in space
/// and efficiently retrieving ones that intersect a local volume.
#[derive(Clone)]
pub struct Grid<T, const N: usize, U> {
    /// Grid cell size, never changes after construction.
    cell: [T; N],

    /// Bounding boxes of items in grid, position index is item identity.
    items: Vec<Option<U>>,

    /// Reusable indices in `items` left from `remove` operations.
    reusable_indices: Vec<usize>,

    /// Map from grid cells to items overlapping cell.
    grid: HashMap<[i32; N], Vec<usize>>,
}

impl<T, const N: usize, U: GridItem<T, N>> Grid<T, N, U>
where
    T: Element + Euclid + AsPrimitive<i32> + FromPrimitive + PartialEq + Debug,
{
    /// Creates a new grid with the given cell size.
    ///
    /// Cell size affects grid performance, queries are faster when cell size
    /// is not much larger than the average query volume, and larger cell
    /// sizes mean less cell churn in updates where the position of an object
    /// doesn't change much.
    pub fn new(cell: impl Into<[T; N]>) -> Self {
        let cell = cell.into();
        assert!(
            cell.iter().fold(T::one(), |a, b| a * *b) != T::zero(),
            "zero volume grid cell"
        );
        Self {
            cell,
            items: Default::default(),
            reusable_indices: Default::default(),
            grid: Default::default(),
        }
    }

    /// Iterate through all objects stored in the grid.
    pub fn iter(&self) -> impl Iterator<Item = (usize, &U)> {
        self.items
            .iter()
            .enumerate()
            .filter_map(|(i, b)| b.as_ref().map(|b| (i, b)))
    }

    /// Get a given item if it is present.
    pub fn get(&self, id: usize) -> Option<&U> {
        self.items.get(id).and_then(|a| a.as_ref())
    }

    /// Insert a new item to grid, returns an identifier that can be used to
    /// refer to the item.
    pub fn insert(&mut self, item: U) -> usize {
        let bounds = *item.as_bounds();

        let id = if let Some(reuse) = self.reusable_indices.pop() {
            self.items[reuse] = Some(item);
            reuse
        } else {
            self.items.push(Some(item));
            self.items.len() - 1
        };

        for c in bounds.intersecting_lattice(self.cell) {
            self.add(c, id)
        }

        id
    }

    /// Remove an item from the grid. Panics if the item does not exist in the
    /// grid.
    pub fn remove(&mut self, id: usize) {
        self.reusable_indices.push(id);

        let item = self.items[id].take().expect("Grid: Accessing removed item");
        for c in item.as_bounds().intersecting_lattice(self.cell) {
            self.rm(c, id);
        }
    }

    /// Update the volume of an item.
    pub fn update(&mut self, id: usize, mut update_f: impl FnMut(&mut U)) {
        let mut old_bounds;
        let mut new_bounds;

        {
            let item = self.items[id]
                .as_mut()
                .expect("Grid: Accessing removed item");
            old_bounds = item
                .as_bounds()
                .intersecting_lattice(self.cell)
                .into_iter()
                .peekable();

            update_f(item);

            new_bounds = item
                .as_bounds()
                .intersecting_lattice(self.cell)
                .into_iter()
                .peekable();
        }

        // Do a clever iteration so that only changed parts of the grid cover
        // are updated to grid. This relies on lattice iteration yielding
        // cells in lexical sort order.
        loop {
            match (old_bounds.peek(), new_bounds.peek()) {
                // No more content.
                (None, None) => break,
                // Old bounds has parts before new bounds, remove.
                (Some(old), Some(new)) if old < new => {
                    self.rm(*old, id);
                    old_bounds.next();
                }
                (Some(old), None) => {
                    self.rm(*old, id);
                    old_bounds.next();
                }
                // New bounds has parts before old bounds
                (Some(old), Some(new)) if old > new => {
                    self.add(*new, id);
                    new_bounds.next();
                }
                (None, Some(new)) => {
                    self.add(*new, id);
                    new_bounds.next();
                }
                // Bounds cells match, no action needed.
                (Some(old), Some(new)) => {
                    debug_assert!(old == new);
                    old_bounds.next();
                    new_bounds.next();
                }
            }
        }
    }

    /// Iterate items that intersect the given volume.
    ///
    /// Result is a tuple of grid index and object value reference.
    pub fn intersecting<'a>(
        &'a self,
        bounds: &'a AxisBox<T, N>,
    ) -> impl Iterator<Item = (usize, &U)> {
        let mut seen = HashSet::default();
        bounds
            .intersecting_lattice(self.cell)
            .into_iter()
            .filter_map(|c| self.grid.get(&c))
            .flatten()
            .filter_map(move |&i| {
                if !seen.contains(&i) {
                    seen.insert(i);
                    let item = self.items[i]
                        .as_ref()
                        .expect("Grid: Missing item in grid");
                    if bounds.intersects(item.as_bounds()) {
                        Some((i, item))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
    }

    fn add(&mut self, cell: [i32; N], id: usize) {
        self.grid.entry(cell).or_default().push(id);
    }

    fn rm(&mut self, cell: [i32; N], id: usize) {
        let bin = self.grid.get_mut(&cell).expect("Grid: Missing cell");
        let i = bin
            .iter()
            .position(|&x| x == id)
            .expect("Grid: Object not in cell");
        bin.swap_remove(i);

        // If the removal emptied the cell, remove the cell entirely from the
        // grid.
        if bin.is_empty() {
            self.grid.remove(&cell);
        }
    }
}
