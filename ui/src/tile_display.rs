use engine::prelude::*;
use navni::prelude::*;
use rand::Rng;
use util::{s8, srng};

use navni::X256Color as X;

#[rustfmt::skip]
pub(crate) const SHARP_CORNERS: [char; 16] = [
    '│', '╵', '╶', '└', '╷', '│', '┌', '├',
    '╴', '┘', '─', '┴', '┐', '┤', '┬', '┼',
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
        Tile::Wall => {
            if let Some(i) = wallform(r, wide_loc_pos) {
                CharCell::c(DOUBLE_LINE[i])
            } else {
                CharCell::c(' ')
            }
        }
        Tile::Ground => CharCell::c(' '),
        Tile::Grass => {
            const GRASS_SPARSENESS: usize = 3;
            if is_centered
                && srng(&wide_loc_pos).gen_range(0..GRASS_SPARSENESS) == 0
            {
                CharCell::c(',').col(X::GREEN)
            } else {
                CharCell::c(' ')
            }
        }
        Tile::LowWall => {
            if let Some(i) = wallform(r, wide_loc_pos) {
                CharCell::c(SHARP_CORNERS[i])
            } else if is_centered {
                CharCell::c('∙')
            } else {
                CharCell::c(' ')
            }
        }
        Tile::Door => {
            if let Some(i) = wallform(r, wide_loc_pos) {
                CharCell::c(CROSSED[i])
            } else {
                CharCell::c('+')
            }
        }
        Tile::Water => CharCell::c(if is_centered { '~' } else { ' ' })
            .col(X::NAVY)
            .inv(),
        Tile::Magma => CharCell::c(if is_centered { '~' } else { ' ' })
            .col(X::MAROON)
            .inv(),
        Tile::Upstairs => {
            if is_centered {
                CharCell::c('↑')
            } else {
                CharCell::c(' ')
            }
        }
        Tile::Downstairs => {
            if is_centered {
                CharCell::c('↓')
            } else {
                CharCell::c(' ')
            }
        }
        Tile::Gore => {
            CharCell::c(match srng(&wide_loc_pos).gen_range(0..=10) {
                d if d < 4 => ',',
                d if d < 7 => '\'',
                8 => ';',
                9 => '*',
                _ => '§',
            })
            .col(X::MAROON)
        }
        Tile::Exit => CharCell::c('░'),
    }
}

fn tile(r: &impl AsRef<Runtime>, wide_loc_pos: IVec2) -> Tile {
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
