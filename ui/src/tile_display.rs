use content::{Coordinates, Tile};
use engine::prelude::*;
use navni::prelude::*;
use rand::Rng;
use util::{s8, srng};

use navni::X256Color as X;

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

/// Return 4-bit wallform connectivity shape for center cell.
/// Return none if the wall cell shouldn't be shown at all.
fn wallform(r: &impl AsRef<Runtime>, p: IVec2) -> Option<usize> {
    //     701
    //     6.2
    //     543
    //
    // Bordering cell angle positions

    if !tile(r, p).is_wall() {
        return None;
    }

    // Low walls block other low walls but not high walls.
    // High walls block everything else.
    let self_height = tile(r, p).self_height();
    let blocks =
        |h: usize, a: usize| tile(r, p + s8::DIR[a % 8]).edge_height() >= h;

    if (0..8).all(|a| blocks(self_height, a)) {
        // Entirely within wall mass, do not draw.
        return None;
    }

    let mut ret = 0;

    let n = |a| tile(r, p + s8::DIR[a % 8]);

    // Go through the 4 neighbors.
    for a in [0, 2, 4, 6] {
        let h = tile(r, p + s8::DIR[a]).self_height();

        if blocks(h, a + 6)
            && blocks(h, a + 7)
            && blocks(h, a + 1)
            && blocks(h, a + 2)
        {
            // Neighbor is fully merged in edge mass, does not get drawn. Do
            // not shape towards it.
            continue;
        }
        if n(a).is_wall() {
            ret += 1 << (a / 2);
        }
    }

    Some(ret)
}

pub fn terrain_cell(
    r: &impl AsRef<Runtime>,
    wide_loc_pos: impl Into<IVec2>,
) -> CharCell {
    let wide_loc_pos = wide_loc_pos.into();
    let r = r.as_ref();

    // XXX: This calls the same functions many times, could use a memoizing
    // cache.

    if let Some(loc) = Location::fold_wide(wide_loc_pos) {
        match loc.tile(r) {
            Tile::Void => CharCell::c('░'),
            Tile::Solid(_) => {
                if let Some(connectivity) = util::wallform_mask(
                    |p: IVec2| (loc + p.extend(0)).is_wall_tile(r),
                    [0, 0],
                ) {
                    CharCell::c(DOUBLE_LINE[connectivity])
                } else {
                    Default::default()
                }
            }

            Tile::Floor {
                z, connectivity, ..
            } => {
                if connectivity != 0 {
                    if z == -1 {
                        CharCell::c(DOWN_SLOPE[connectivity])
                    } else if z == 1 {
                        CharCell::c(UP_SLOPE[connectivity])
                    } else {
                        panic!("Nonzero connectivity at z=0");
                    }
                }
                // Ghost wallforms for cliffy edges
                else if let Some(mask) = loc.cliff_form(r) {
                    CharCell::c(SINGLE_LINE[mask]).col(X::BROWN)
                } else {
                    Default::default()
                }
            }
        }
    } else {
        let (a, b) = (
            terrain_cell(r, wide_loc_pos - ivec2(1, 0)),
            terrain_cell(r, wide_loc_pos + ivec2(1, 0)),
        );
        let (c, d) = (
            std::char::from_u32(a.c as u32).unwrap(),
            std::char::from_u32(b.c as u32).unwrap(),
        );

        // Wall connectivity
        if "═╚╔╠╩╦╬".contains(c) && "═╝╩╗╣╦╬".contains(d)
        {
            CharCell::c('═').col(a.foreground)
        } else if "─└┌├┴┼".contains(c) && "─┘┴┐┤┬┼".contains(d)
        {
            CharCell::c('─').col(a.foreground)
        } else if c == '░' && d == '░' {
            CharCell::c('░')
        } else {
            Default::default()
        }
    }
}

/// Show the interpolated and shaped map terrain cell in the given wide
/// unfolded coordinate position.
pub fn flat_terrain_cell(
    r: &impl AsRef<Runtime>,
    wide_loc_pos: impl Into<IVec2>,
) -> CharCell {
    let wide_loc_pos = wide_loc_pos.into();

    let is_centered = wide_loc_pos.x % 2 == 0;

    match tile(r, wide_loc_pos) {
        // TODO wallforming
        Tile2D::Wall => {
            if let Some(i) = wallform(r, wide_loc_pos) {
                CharCell::c(DOUBLE_LINE[i])
            } else {
                CharCell::c(' ')
            }
        }
        Tile2D::Ground => CharCell::c(' '),
        Tile2D::Grass => {
            const GRASS_SPARSENESS: usize = 3;
            if is_centered
                && srng(&wide_loc_pos).gen_range(0..GRASS_SPARSENESS) == 0
            {
                CharCell::c(',').col(X::GREEN)
            } else {
                CharCell::c(' ')
            }
        }
        Tile2D::LowWall => {
            if let Some(i) = wallform(r, wide_loc_pos) {
                CharCell::c(SINGLE_LINE[i])
            } else if is_centered {
                CharCell::c('∙')
            } else {
                CharCell::c(' ')
            }
        }
        Tile2D::Door => {
            if let Some(i) = wallform(r, wide_loc_pos) {
                CharCell::c(CROSSED[i])
            } else {
                CharCell::c('+')
            }
        }
        Tile2D::Water => CharCell::c(if is_centered { '~' } else { ' ' })
            .col(X::NAVY)
            .inv(),
        Tile2D::Magma => CharCell::c(if is_centered { '~' } else { ' ' })
            .col(X::MAROON)
            .inv(),
        Tile2D::Upstairs => {
            if is_centered {
                CharCell::c('↑')
            } else {
                CharCell::c(' ')
            }
        }
        Tile2D::Downstairs => {
            if is_centered {
                CharCell::c('↓')
            } else {
                CharCell::c(' ')
            }
        }
        Tile2D::Gore => {
            CharCell::c(match srng(&wide_loc_pos).gen_range(0..=10) {
                d if d < 4 => ',',
                d if d < 7 => '\'',
                8 => ';',
                9 => '*',
                _ => '§',
            })
            .col(X::MAROON)
        }
        Tile2D::Exit => CharCell::c('░'),
    }
}

fn tile(r: &impl AsRef<Runtime>, wide_loc_pos: IVec2) -> Tile2D {
    let p = wide_loc_pos;

    if p.x % 2 == 0 {
        Location::fold_wide(p).unwrap().assumed_tile(r)
    } else {
        Location::fold_wide(p - ivec2(1, 0))
            .unwrap()
            .assumed_tile(r)
            .mix(
                Location::fold_wide(p + ivec2(1, 0))
                    .unwrap()
                    .assumed_tile(r),
            )
    }
}
