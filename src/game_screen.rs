use engine::prelude::*;
use gfx::v2;
use navni::prelude::*;
use ui::prelude::*;
use ui::Game;

use navni::X256Color as X;

pub fn run(
    g: &mut Game,
    b: &mut dyn Backend,
    _n: u32,
) -> Option<StackOp<Game>> {
    let win = Window::from(&g.s);

    // TODO: Handle missing player entity.
    let player = g.r.player().unwrap();
    let loc = player.loc(&g.r).unwrap();
    draw_map(g, &win, loc);

    win.write(&mut g.s, [2, 35], "Hello, world!");

    g.draw(b);

    None
}

fn draw_map(g: &mut Game, win: &Window, loc: Location) {
    let sector_bounds = wide_unfolded_sector_bounds(loc);
    let offset =
        util::scroll_offset(&win.area(), loc.unfold_wide(), &sector_bounds);

    for draw_pos in win.area().into_iter().map(v2) {
        let p = draw_pos + offset;
        if !sector_bounds.contains(p) {
            win.put(&mut g.s, draw_pos, CharCell::c('█').col(X::BROWN));
            continue;
        }

        win.put(&mut g.s, draw_pos, ui::terrain_cell(&g.r, p));

        if let Some(loc) = Location::fold_wide(p) {
            if let Some(e) = loc.mob_at(&g.r) {
                let mut icon = e.icon(&g.r);
                if g.r.player() == Some(e) {
                    icon = '@';
                }
                win.put(&mut g.s, draw_pos, CharCell::c(icon));
            }
        }
    }
}

/// Rectangle containing cells of location's sector plus one-cell rim of
/// adjacent sectors projected into wide unfolded space.
fn wide_unfolded_sector_bounds(loc: Location) -> Rect {
    // Get sector area with the rim to adjacent sectors.
    let bounds = loc.expanded_sector_bounds();

    // Convert to wide space.
    let p1 = IVec2::from(bounds.min()) * ivec2(2, 1);
    let mut p2 = IVec2::from(bounds.max()) * ivec2(2, 1);

    // Trim out the part that would be in-between cells for cells that don't
    // belong in the original set.
    p2.x = 0.max(p2.x - 1);

    Rect::new(p1, p2)
}
