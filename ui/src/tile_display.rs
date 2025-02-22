use derive_more::Deref;
use engine::prelude::*;
use glam::{IVec3, ivec3};
use navni::prelude::*;
use rand::Rng;
use world::{Block, Tile, Zone};

use navni::X256Color as X;
use util::reverse_dir_mask_4;

use crate::prelude::*;

/// Projection from a location into an on-screen wide-tile display space.
#[derive(Copy, Clone, Default, Eq, PartialEq, Debug, Deref)]
pub struct SectorView(IVec3);

impl SectorView {
    /// Determine the view transformation into `view_size` sized viewport.
    ///
    /// The projection starts out centered on `loc` and is then shifted to
    /// maximize the area of the sector of `loc` shown on the viewport.
    pub fn new(view_size: impl Into<IVec2>, loc: Location) -> Self {
        let view_size = view_size.into();

        // The location's (expanded) sector. We want to clip the draw to show
        // only the sector area. The sector is expanded since the on-screen
        // sector should show a single-tile rim from adjacent sectors as an
        // exit zone.
        let mut map_rect = loc.sector().wide().flatten();

        // Widen the map rect into 2x and trim the right edge, the center
        // cells of both edges should go right on the map rect border.
        map_rect.p0[0] *= 2;
        map_rect.p1[0] = map_rect.p1[0] * 2 - 1;

        // Wide space projection of center location, initially placed at the
        // center of the view area.
        let mut offset = loc.truncate() * ivec2(2, 1) - view_size / ivec2(2, 2);

        // Map rect moved so the location is at origin.
        let offset_rect = map_rect - offset;

        // Adjust offset to maximize area of map_rect visible on screen.
        for d in 0..2 {
            if offset_rect.dim()[d] < view_size[d] {
                // View is big enough (along this axis) to fit the whole arena.
                // Just center the arena rect then.
                offset[d] =
                    map_rect.min()[d] + (map_rect.dim()[d] - view_size[d]) / 2;
            } else if offset_rect.min()[d] > 0 {
                // Snap inside inner edge of the view.
                offset[d] += offset_rect.min()[d];
            } else if offset_rect.max()[d] < view_size[d] {
                // Snap inside outer edge of the view.
                offset[d] -= view_size[d] - offset_rect.max()[d];
            }
        }

        SectorView(offset.extend(loc.z))
    }

    /// Project location to screen.
    pub fn project(&self, loc: impl Into<Location>) -> IVec2 {
        let loc = loc.into();
        (loc * ivec3(2, 1, 1) - self.0).truncate()
    }

    /// Project widened location to screen.
    pub fn project_wide(&self, wide_loc: IVec3) -> IVec2 {
        (wide_loc - self.0).truncate()
    }

    /// Project screen to location, snapping to the left.
    ///
    /// This is the preferred unprojection variant that matches the "extend to
    /// the right" convention for the tiles.
    pub fn unproject_1(&self, pos: impl Into<IVec2>) -> Location {
        let mut pos = pos.into().extend(0) + self.0;
        pos.x = pos.x.div_euclid(2);
        pos
    }

    /// Project screen to location, snapping to the right.
    ///
    /// Secondary unprojection variant, use `unproject_1` instead if you have
    /// no specific reason to use this.
    pub fn unproject_2(&self, pos: impl Into<IVec2>) -> Location {
        self.unproject_1(pos.into() + ivec2(1, 0))
    }

    /// Iterate locations covered by the on-screen rectangle (inclusively)
    /// spanned by the two points.
    pub fn view_rect_locations(
        &self,
        p1: impl Into<IVec2>,
        p2: impl Into<IVec2>,
    ) -> impl Iterator<Item = Location> {
        let (p1, p2) = (p1.into(), p2.into());
        let rect = Rect::from_points_inclusive([p1, p2]);
        Cube::new(
            self.unproject_2(rect.min()),
            // XXX: Why do I need to use unproject_2 instead of _1 for the
            // second one as well?
            self.unproject_2(rect.max()) + ivec3(0, 0, 1),
        )
        .into_iter()
        .map(Location::from)
    }

    /// Return the location at the center of the view.
    ///
    /// The location may have changed from the initial parameter given to
    /// `new` when the view was snapped to the current sector.
    pub fn center(&self, view_size: impl Into<IVec2>) -> Location {
        self.unproject_1(view_size.into() / ivec2(2, 2))
    }

    /// Iterate through all locations that will cover the given window.
    pub fn iter(
        &self,
        view_size: impl Into<[i32; 2]>,
    ) -> impl Iterator<Item = (IVec2, Location)> + '_ {
        Rect::sized(view_size)
            .grow([1, 0], [1, 0])
            .into_iter()
            .filter_map(|p| {
                // Filter result to positions that are at cell center. The pos is
                // at center when both unprojections agree.
                let (a, b) = (self.unproject_1(p), self.unproject_2(p));
                (a == b).then_some((p.into(), a))
            })
    }
}

#[derive(Copy, Clone, Default, Eq, PartialEq, Debug)]
pub struct DisplayTile {
    /// Center display cell.
    c0: CharCell,
    /// Right-of-center display cell.
    c1: CharCell,

    /// Location of the tile's floor level in space.
    ///
    /// For floor tiles, the Z coordinate is offset to bring the location to
    /// the empty space on top of the visible floor. For wall and void spaces,
    /// the z coordinate is the display plane for which the tile was
    /// constructed.
    loc: Location,
}

impl DisplayTile {
    pub fn new(r: &impl AsRef<Runtime>, mut loc: Location) -> Self {
        use Block::*;
        use Tile::*;

        let r = r.as_ref();

        let mut c0 = Default::default();
        let mut c1 = Default::default();

        let left = loc.tile(r);
        let right = (loc + ivec3(1, 0, 0)).tile(r);

        // Do c1 here, overwrite later if needed...

        // Display tiles in-between two central tiles, simpler than the
        // central tiles that have complex shaping.
        match (left, right) {
            // Merge same.
            (Surface(loc, a), Surface(_, b)) if a == b => {
                let mut rng = util::srng(&loc);
                c1 = floor_cell(&mut rng, a, false);
            }
            // Magma overrides water.
            // Fluids stick to walls.
            (Wall(_), Surface(loc, Magma))
            | (Surface(loc, Magma), Wall(_))
            | (Surface(loc, Water), Surface(_, Magma))
            | (Surface(loc, Magma), Surface(_, Water)) => {
                let mut rng = util::srng(&loc);
                c1 = floor_cell(&mut rng, Magma, false);
            }
            (Wall(_), Surface(loc, Water)) | (Surface(loc, Water), Wall(_)) => {
                let mut rng = util::srng(&loc);
                c1 = floor_cell(&mut rng, Water, false);
            }
            // Chasms stick to walls.
            (Void, Wall(_)) | (Wall(_), Void) | (Void, Void) => {
                c1 = CharCell::c('▒');
            }
            _ => {}
        }

        // Central display tiles, correspond to actual logical tiles.
        match left {
            Surface(loc_2, block) => {
                if let Some(mask) = loc.cliff_form(r) {
                    // Surface is a cliff next to a lower-down but also
                    // visible surface, draw the cliff form tile.

                    c0 = CharCell::c(SINGLE_LINE[mask]).col(X::BROWN);
                    let connect_right = (mask & 0b10) != 0;
                    if connect_right {
                        c1 = CharCell::c(SINGLE_LINE[0b10]).col(X::BROWN);
                    }
                } else {
                    let mut rng = util::srng(&loc_2);
                    c0 = floor_cell(&mut rng, block, true);
                }

                if loc_2.z > loc.z {
                    let a = loc_2.high_connectivity(r);
                    // Try to make a tighter mask by requiring there to be a
                    // corresponding path from the other direction. If this
                    // gets us a specific direction, use that.
                    let b = a & reverse_dir_mask_4(loc_2.low_connectivity(r));
                    let a = if b != 0 { b } else { a };
                    if a != 0 {
                        c0 = CharCell::c(UP_SLOPE[a]);
                    }
                } else if loc_2.z < loc.z {
                    let a = loc_2.low_connectivity(r);
                    let b = a & reverse_dir_mask_4(loc_2.high_connectivity(r));
                    let a = if b != 0 { b } else { a };
                    if a != 0 {
                        c0 = CharCell::c(DOWN_SLOPE[a]);
                    }
                }

                loc = loc_2;
            }
            Wall(block) if !loc.is_interior_wall(r) => {
                // Create a connection mask from the visible neighboring wall
                // tiles.
                //
                // Assume unrevealed tiles are not walls so as not to
                // reveal details of unexplored structures. Walls fully in the
                // interior aren't drawn with an edge even if they're next to
                // unexplored terrain though.

                if let Some(mask) = util::wallform_mask(
                    |loc: Location| {
                        (loc.is_explored(r) && loc.tile(r).is_wall())
                            || loc.is_interior_wall(r)
                    },
                    loc,
                ) {
                    let tileset: &dyn Wallform = match block {
                        Door => &CROSSED,
                        Glass => &SINGLE_LINE,
                        Rubble => &'%',
                        Altar => &'=',
                        _ => &DOUBLE_LINE,
                    };
                    c0 = CharCell::c(tileset.idx(mask));

                    let connect_right = (mask & 0b10) != 0;
                    if connect_right {
                        // Adjacent windows form continuous pane, adjacent
                        // doors don't.
                        //
                        // Rough tiles make rough lines.
                        let tileset_2: &dyn Wallform = match right {
                            Wall(Glass) if block == Glass => &SINGLE_LINE,
                            Wall(Rubble) if block == Rubble => &'%',
                            Wall(Altar) if block == Altar => &'=',
                            _ => &DOUBLE_LINE,
                        };
                        c1 = CharCell::c(tileset_2.idx(0b10));
                    }
                }
            }
            Wall(_) => {
                // Interior walls don't show as anything.
            }
            Void => {
                c0 = CharCell::c('▒');
            }
        }

        DisplayTile { c0, c1, loc }
    }

    pub fn render(&self, win: &Window, pos: impl Into<IVec2>) {
        let pos = pos.into();

        win.put(pos, self.c0);
        win.put(pos + ivec2(1, 0), self.c1);
    }
}

pub fn render_fog(
    r: &impl AsRef<Runtime>,
    win: &Window,
    pos: impl Into<IVec2>,
    loc: Location,
) {
    let r = r.as_ref();
    let pos = pos.into();

    let left = loc;
    let right = left + ivec3(1, 0, 0);

    let (cover_left, cover_right) =
        (!left.is_explored(r), !right.is_explored(r));
    let cover_middle = cover_left || cover_right;

    if cover_left {
        win.put(pos, CharCell::c('░').col(X::BROWN));
    }

    if cover_middle {
        win.put(pos + ivec2(1, 0), CharCell::c('░').col(X::BROWN));
    }
}

fn floor_cell(rng: &mut impl Rng, block: Block, is_center: bool) -> CharCell {
    use Block::*;
    match block {
        Grass if is_center => {
            const GRASS_SPARSENESS: usize = 3;
            if rng.random_range(0..GRASS_SPARSENESS) == 0 {
                CharCell::c(',').col(X::GREEN)
            } else {
                CharCell::c(' ')
            }
        }
        Stone | Glass | Altar | Door | Grass | Rubble => CharCell::c(' '),
        SplatteredStone => CharCell::c(match rng.random_range(0..=10) {
            d if d < 4 => ',',
            d if d < 7 => '\'',
            8 => ';',
            9 => '*',
            _ => '§',
        })
        .col(X::MAROON),
        Water => CharCell::c(if is_center { '~' } else { ' ' })
            .col(X::NAVY)
            .inv(),
        Magma => CharCell::c(if is_center { '~' } else { ' ' })
            .col(X::MAROON)
            .inv(),
    }
}

trait Wallform {
    fn idx(&self, i: usize) -> char;
}

// Sample from the array using bitmask if using array.
impl Wallform for [char; 16] {
    fn idx(&self, i: usize) -> char {
        self[i]
    }
}

// Show the same char for all connections if using single char.
impl Wallform for char {
    fn idx(&self, _: usize) -> char {
        *self
    }
}

#[rustfmt::skip]
pub(crate) const SINGLE_LINE: [char; 16] = [
    '│', '│', '─', '└', '│', '│', '┌', '├',
    '─', '┘', '─', '┴', '┐', '┤', '┬', '┼',
];

#[rustfmt::skip]
const _ROUNDED_CORNERS: [char; 16] = [
    '│', '╵', '╶', '╰', '╷', '│', '╭', '├',
    '╴', '╯', '─', '┴', '╮', '┤', '┬', '┼',
];

#[rustfmt::skip]
const DOUBLE_LINE: [char; 16] = [
    '║', '║', '═', '╚', '║', '║', '╔', '╠',
    '═', '╝', '═', '╩', '╗', '╣', '╦', '╬',
];

#[rustfmt::skip]
const CROSSED: [char; 16] = [
    '╫', '╫', '╪', '+', '╫', '╫', '+', '+',
    '╪', '+', '╪', '+', '+', '+', '+', '+',
];

// ▲▶▼◀
/// Slopes upwards from a high floor.
#[rustfmt::skip]
const UP_SLOPE: [char; 16] = [
    ' ', '▼', '◀', '◆', '▲', '◆', '◆', '◆',
    '▶', '◆', '◆', '◆', '◆', '◆', '◆', '◆',
];

/// Slopes downward from a low floor.
#[rustfmt::skip]
const DOWN_SLOPE: [char; 16] = [
    ' ', '▲', '▶', '●', '▼', '●', '●', '●',
    '◀', '●', '●', '●', '●', '●', '●', '●',
];
