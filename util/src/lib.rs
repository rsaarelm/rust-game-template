//! Unopinionated standalone utilities.
#![feature(lazy_cell)]

mod axis_box;
pub use axis_box::{AxisBox, Cube, Element, LatticeBox, Rect};

mod bits;
pub use bits::{
    compact_u32_by_2, compact_u64_by_2, spread_u32_by_2, spread_u64_by_2,
};

mod geom;
pub use geom::{bresenham_line, PlottedPoint, VecExt, DIR_4, DIR_8};

mod grid;
pub use grid::Grid;

mod idm;
pub use idm::directory_to_idm;

mod mung;
pub use mung::{mung, unmung};

mod path;
pub use path::{astar_path, dijkstra_map, flood_fill_4, within_range};

mod rng;
pub use rng::{srng, Odds};

mod sys;
pub use sys::KeyboardLayout;

pub mod text;

pub use rustc_hash::FxHashMap as HashMap;
pub use rustc_hash::FxHashSet as HashSet;
