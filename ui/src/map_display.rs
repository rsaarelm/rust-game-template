use engine::prelude::*;
use navni::prelude::*;
use rand::Rng;
use util::srng;

use navni::X256Color as X;

#[rustfmt::skip]
const SHARP_CORNERS: [char; 16] = [
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
fn wallform(r: &Runtime, p: IVec2) -> Option<usize> {
    //     701
    //     6.2
    //     543
    //
    // Bordering cell angle positions

    if !tile(r, p).is_wall() {
        return None;
    }

    let open: Vec<bool> =
        DIR_8.iter().map(|&v| !tile(r, p + v).is_edge()).collect();

    if !open.iter().any(|&a| a) {
        // Entirely within wall mass, do not draw.
        return None;
    }

    let mut ret = 0;

    let n = |a| tile(r, p + DIR_8[(a % 8) as usize]);

    // Go through the 4 neighbors.
    for a in [0, 2, 4, 6] {
        if n(a + 6).is_edge()
            && n(a + 7).is_edge()
            && n(a + 1).is_edge() & n(a + 2).is_edge()
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

pub fn terrain_cell(r: &Runtime, wide_loc_pos: IVec2) -> CharCell {
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
            const GRASS_SPARSITY: usize = 3;
            if is_centered
                && srng(&wide_loc_pos).gen_range(0..GRASS_SPARSITY) == 0
            {
                CharCell::c(',').col(X::GREEN)
            } else {
                CharCell::c(' ')
            }
        }
        Tile::LowWall => {
            if let Some(i) = wallform(r, wide_loc_pos) {
                CharCell::c(SHARP_CORNERS[i])
            } else {
                CharCell::c(' ')
            }
        }
        Tile::Door => {
            if let Some(i) = wallform(r, wide_loc_pos) {
                CharCell::c(CROSSED[i])
            } else {
                CharCell::c(' ')
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

fn tile(r: &Runtime, wide_loc_pos: IVec2) -> Tile {
    let p = wide_loc_pos;

    if p.x % 2 == 0 {
        Location::fold_wide(p).tile(r)
    } else {
        Location::fold_wide(p - ivec2(1, 0))
            .tile(r)
            .mix(Location::fold_wide(p + ivec2(1, 0)).tile(r))
    }
}
