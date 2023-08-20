use engine::prelude::*;
use gfx::v2;
use navni::prelude::*;
use ui::prelude::*;
use ui::Game;

use navni::X256Color as X;

pub fn run(g: &mut Game, b: &mut dyn Backend, n: u32) -> Option<StackOp<Game>> {
    g.tick(b);

    // DISPLAY
    let win = Window::from(&g.s);
    let (panel, main) = win.split_left(26);

    draw_panel(g, &panel);
    draw_main(g, n, &main);

    // TODO cursoring
    for (y, m) in g.msg.iter().enumerate() {
        main.write(&mut g.s, [0, y as i32], m);
    }

    g.draw(b);

    // INPUT
    if let Some(e) = g.input_map.get(&b.keypress()) {
        use ui::InputAction::*;
        let player = g.r.player().unwrap();
        match e {
            North => player.execute(&mut g.r, Action::Bump(DIR_4[0])),
            East => player.execute(&mut g.r, Action::Bump(DIR_4[1])),
            South => player.execute(&mut g.r, Action::Bump(DIR_4[2])),
            West => player.execute(&mut g.r, Action::Bump(DIR_4[3])),
            QuitGame => return Some(StackOp::Pop),
            _ => {}
        }
    }

    None
}

fn draw_panel(g: &mut Game, win: &Window) {
    win.write(&mut g.s, [0, 0], "Hello, world!");
}

/// Draw main game area.
fn draw_main(g: &mut Game, n_updates: u32, win: &Window) {
    if let Some(loc) = g.r.player().and_then(|p| p.loc(&g.r)) {
        g.camera = loc;
    }

    let sector_bounds = wide_unfolded_sector_bounds(g.camera);
    let offset = util::scroll_offset(
        &win.area(),
        g.camera.unfold_wide(),
        &sector_bounds,
    );

    // Solid background for off-sector extra space.
    win.fill(&mut g.s, CharCell::c('█').col(X::BROWN));
    // Constrain sub-window to current sector only.
    let sector_win = win.sub(sector_bounds - offset);
    // Adjust offset for sub-window position.
    let offset = v2(sector_bounds.min()).max(offset);
    draw_map(g, &sector_win, offset);
    g.draw_anims(n_updates, &sector_win, offset);
    draw_fog(g, &sector_win, offset);
}

fn draw_map(g: &mut Game, win: &Window, offset: IVec2) {
    for draw_pos in win.area().into_iter().map(v2) {
        let p = draw_pos + offset;

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

fn draw_fog(g: &mut Game, win: &Window, offset: IVec2) {
    for draw_pos in win.area().into_iter().map(v2) {
        if g.r.wide_pos_is_shrouded(draw_pos + offset) {
            win.put(&mut g.s, draw_pos, CharCell::c('░').col(X::BROWN));
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
