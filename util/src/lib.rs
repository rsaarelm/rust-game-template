//! Unopinionated standalone utilities.

mod axis_box;
pub use axis_box::{AxisBox, Cube, Element, IntegerBox, Rect};

mod bits;
pub use bits::{
    compact_u32_by_2, compact_u64_by_2, spread_u32_by_2, spread_u64_by_2,
};

mod cloud;
pub use cloud::Cloud;

mod distribution;
pub use distribution::{PlottedDistribution, RangeDistribution};

mod geom;
pub use geom::{
    AXIS_DIRS, Neighbors2D, Neighbors3D, PlottedPoint, PolyLineIter, Sdf,
    VecExt, a3, bresenham_line, reverse_dir_mask_4, s_hex, s4, s8, v2, v3,
    wallform_mask,
};

mod grammar;
pub use grammar::{Noun, Sentence};

mod grid;
pub use grid::Grid;

mod idm;
pub use idm::{_String, IncrementalOutline, Outline, dash_option, dir_to_idm};

mod interned_string;
pub use interned_string::InString;

mod keyboard_layout;
pub use keyboard_layout::Layout;

mod lazy_res;
pub use lazy_res::LazyRes;

pub mod parse;

mod path;
pub use path::{bfs, dijkstra_search};

mod rng;
pub use rng::{Odds, RngExt, srng};

mod silo;
pub use silo::Silo;

mod str_ext;
pub use str_ext::{CharExt, StrExt};

mod sync;
pub use sync::SameThread;

mod sys;
pub use sys::{KeyboardLayout, can_quit_program, panic_handler, user_name};

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

/// Function to check if the game is being run in wizard mode for in-game
/// debugging. This is set by setting the environment variable
/// `WIZARD_MODE=1`.
#[memoize::memoize]
pub fn wizard_mode() -> bool {
    matches!(
        std::env::var("WIZARD_MODE").ok().and_then(|a: String| a.parse::<i32>().ok()),
        Some(val) if val > 0)
}
