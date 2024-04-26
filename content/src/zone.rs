use glam::{ivec3, IVec3};
use util::{v3, Neighbors3D};

use crate::{Cube, Location, LEVEL_BASIS, SECTOR_HEIGHT, SECTOR_WIDTH};

/// Trait for the various volumes of game space.
///
/// Terminology:
///
/// - Sector lattice: A 2D grid at any z level with `(SECTOR_WIDTH,
///   SECTOR_HEIGHT)` sized cells and a corner snapping to origin.
/// - Sector: A 1 z level thick box that fills a single sector lattice cell.
/// - Fat sector: A sector expanded to three z levels in thickness. Represents
///   the on-screen vertically visible space with the voxel display system.
/// - Wide sector: A sector expanded with a 1 cell wide horizontal rim in
///   every direction. Represents the on-screen horizontally visible area that
///   shows a single tile from neighboring sectors.
/// - Level: Unit of world construction, a `LEVEL_DEPTH` thick stack of
///   sectors with the z origin at an integer multiple of `LEVEL_DEPTH`.
pub trait Zone: Neighbors3D + Clone + Sized {
    /// Construct 1 z level thick sector zone from location.
    fn sector_from(loc: Location) -> Self;

    fn level_from(loc: Location) -> Self {
        Self::sector_from(loc).level()
    }

    /// Return a level at given lattice coordinates.
    fn level_at(lattice_pos: impl Into<[i32; 3]>) -> Self;

    fn offset(&self, lattice_delta: IVec3) -> Self;

    /// Make regular-width sector into "wide sector", grow a rim of one cell.
    ///
    /// Onscreen map display shows wide sectors, the contents of the main
    /// sector plus one tile from each adjacent sector.
    ///
    /// Should only be applied to regular width sector.
    fn wide(&self) -> Self;

    /// Make thin sector into "fat sector", grow one Z layer up and down.
    ///
    /// Fat sectors are the zone in which voxels are shown as regular floor on
    /// the display.
    ///
    /// Should only be applied to sector of depth 1.
    fn fat(&self) -> Self;

    /// Snap a regular sector into two levels tall level zone.
    ///
    /// Sectors at z = 0 and z = 1 both snap into the level at z = 0..2.
    /// Should only be applied to regular-width sectors that are either thin
    /// or already a level. Applying on a level is a no-op.
    fn level(&self) -> Self;

    /// Return whether zone has standard sector width and height and snaps to
    /// the sector lattice.
    fn is_regular_width(&self) -> bool;

    fn above(&self) -> Self {
        self.offset(ivec3(0, 0, 1))
    }

    fn below(&self) -> Self {
        self.offset(ivec3(0, 0, -1))
    }

    /// Return the level neighborhood which should have maps generated for it
    /// when the central level is being set up as an active play area.
    fn cache_volume(&self) -> impl Iterator<Item = Self> {
        Some(self.clone()).into_iter().chain(self.clone().ns_10())
    }

    fn east(&self) -> Self {
        self.offset(ivec3(1, 0, 0))
    }

    fn south(&self) -> Self {
        self.offset(ivec3(0, 1, 0))
    }

    /// Return the bottom layer of the zone.
    ///
    /// Levels usually have their interesting walkable area around the floor.
    fn floor(&self) -> Self;
}

impl Zone for Cube {
    fn sector_from(loc: Location) -> Self {
        let origin = ivec3(
            loc.x.div_floor(SECTOR_WIDTH) * SECTOR_WIDTH,
            loc.y.div_floor(SECTOR_HEIGHT) * SECTOR_HEIGHT,
            loc.z,
        );

        Cube::new(origin, origin + ivec3(SECTOR_WIDTH, SECTOR_HEIGHT, 1))
    }

    fn level_at(lattice_pos: impl Into<[i32; 3]>) -> Self {
        Cube::cell(LEVEL_BASIS, lattice_pos)
    }

    fn offset(&self, lattice_delta: IVec3) -> Self {
        *self + (lattice_delta * v3(self.dim()))
    }

    fn wide(&self) -> Self {
        self.grow([1, 1, 0], [1, 1, 0])
    }

    fn fat(&self) -> Self {
        self.grow([0, 0, 1], [0, 0, 1])
    }

    fn level(&self) -> Self {
        let level = Cube::cell_containing(LEVEL_BASIS, self.center());
        if !level.contains_other(self) {
            panic!("Zone::level: Invalid level seed zone {self:?}");
        }

        level
    }

    fn is_regular_width(&self) -> bool {
        let origin = v3(self.min());
        self.width() == SECTOR_WIDTH
            && self.height() == SECTOR_HEIGHT
            && origin.x % SECTOR_WIDTH == 0
            && origin.y % SECTOR_HEIGHT == 0
    }

    fn floor(&self) -> Self {
        self.border([0, 0, -1])
    }
}
