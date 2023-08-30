//! Unopinionated standalone utilities.
#![feature(lazy_cell)]

mod axis_box;
pub use axis_box::{AxisBox, Cube, Element, LatticeBox, Rect};

mod bits;
pub use bits::{
    compact_u32_by_2, compact_u64_by_2, spread_u32_by_2, spread_u64_by_2,
};

mod geom;
pub use geom::{
    bresenham_line, s4, s6, s8, scroll_offset, v2, PlottedPoint, VecExt,
};

mod grid;
pub use grid::Grid;

mod idm;
pub use idm::{_String, dash_option, directory_to_idm};

mod keyboard_layout;
pub use keyboard_layout::Layout;

mod lazy_res;
pub use lazy_res::LazyRes;

mod mung;
pub use mung::{mung, unmung};

mod path;
pub use path::{astar_path, dijkstra_map, flood_fill_4, within_range};

mod rng;
pub use rng::{srng, Logos, Odds, RngExt};

mod sys;
pub use sys::KeyboardLayout;

pub mod text;

mod unchecked_write;

pub type FastHasher = rustc_hash::FxHasher;

/// Map with an efficient hash function.
pub use rustc_hash::FxHashMap as HashMap;

/// Set with an efficient hash function.
pub use rustc_hash::FxHashSet as HashSet;

type DefaultHashBuilder = std::hash::BuildHasherDefault<rustc_hash::FxHasher>;

/// Insertion order preserving map with an efficient hash function.
pub type IndexMap<K, V> = indexmap::IndexMap<K, V, DefaultHashBuilder>;

/// Insertion order preserving set with an efficient hash function.
pub type IndexSet<V> = indexmap::IndexSet<V, DefaultHashBuilder>;

pub mod hash_map {
    pub type Entry<'a, A, B> = std::collections::hash_map::Entry<'a, A, B>;
}

/// The "I don't care, just make it work" error type.
pub type Error = Box<dyn std::error::Error>;

/// Good default concrete rng.
pub type GameRng = rand_xorshift::XorShiftRng;
