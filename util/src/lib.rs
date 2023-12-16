//! Unopinionated standalone utilities.
#![feature(lazy_cell)]

mod axis_box;
pub use axis_box::{AxisBox, Cube, Element, IntegerBox, Rect};

mod bits;
pub use bits::{
    compact_u32_by_2, compact_u64_by_2, spread_u32_by_2, spread_u64_by_2,
};

mod cloud;
pub use cloud::Cloud;

mod geom;
pub use geom::{
    bresenham_line, reverse_dir_mask_4, s4, s8, s_hex, v2, v3, wallform_mask,
    Neighbors2D, Neighbors3D, PlottedPoint, PolyLineIter, Sdf, VecExt,
    AXIS_DIRS,
};

mod grammar;
pub use grammar::{Noun, Sentence};

mod grid;
pub use grid::Grid;

mod idm;
pub use idm::{_String, dash_option, dir_to_idm, IncrementalOutline, Outline};

mod interned_string;
pub use interned_string::InString;

mod keyboard_layout;
pub use keyboard_layout::Layout;

mod lazy_res;
pub use lazy_res::LazyRes;

mod logos;
pub use logos::Logos;

mod path;
pub use path::{astar_path, dijkstra_map, within_range};

mod rng;
pub use rng::{srng, Odds, RngExt};

mod sync;
pub use sync::SameThread;

mod sys;
pub use sys::{panic_handler, KeyboardLayout};

pub mod text;

mod unchecked_write;

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
