//! Generic field-of-view computation.

mod fov;
pub use crate::fov::{Fov, Geometry, State};

mod square;
pub use square::SquareGeometry;
pub type Square<T, V> = Fov<SquareGeometry<V>, T>;
