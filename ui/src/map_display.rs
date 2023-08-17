use engine::prelude::*;
use navni::prelude::*;
use rand::Rng;
use util::srng;

use navni::X256Color as X;

pub fn terrain_cell(r: &Runtime, wide_loc_pos: IVec2) -> CharCell {
    let is_centered = wide_loc_pos.x % 2 == 0;
    let tile = tile(r, wide_loc_pos);

    match tile {
        // TODO wallforming
        Tile::Wall => CharCell::c('#'),
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
        // TODO wallforming
        Tile::LowWall => CharCell::c('%'),
        Tile::Door => CharCell::c('+'),
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
