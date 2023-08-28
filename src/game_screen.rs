use std::fmt::Write;

use engine::prelude::*;
use navni::prelude::*;
use ui::prelude::*;
use util::{text, v2, write, writeln};

use navni::X256Color as X;

pub fn run(g: &mut Game, b: &mut dyn Backend, n: u32) -> Option<StackOp<Game>> {
    g.tick(b);

    // DISPLAY
    let win = Window::from(&g.s);
    // Only show sidebar if there's an active player.
    let main = if let Some(player) = g.current_active() {
        let (panel, main) = win.split_left(26);
        draw_panel(g, b, &panel, player);
        main
    } else {
        win
    };

    draw_main(g, n, &main);

    let mut cur = Cursor::new(&mut g.s, main);
    for m in g.msg.iter() {
        writeln!(cur, "{m}");
    }

    g.draw(b);

    // INPUT
    if let Some(&a) = g.input_map.get(&b.keypress()) {
        use ui::InputAction::*;
        g.process_action(a);

        if a == QuitGame {
            return Some(StackOp::Pop);
        }
    }

    None
}

fn draw_panel(g: &mut Game, b: &dyn Backend, win: &Window, player: Entity) {
    use InputAction::*;

    win.clear(&mut g.s);
    let mut cur = Cursor::new(&mut g.s, *win);
    // Two of these just so that both closures below get one to borrow.
    // They all get merged into one output at the end.
    let mut actions = Vec::new();
    let mut actions2 = Vec::new();

    // Print help for a key, also have it act as a button that dispatches the
    // action when clicked.
    let mut command_key = |cur: &mut Cursor<'_>, action| {
        let s = if let Some(k) = g.input_map.key_for(action) {
            // These are supposed to always be single-char, snip to one
            // character here just in case they're something weird
            let k = k.to_string();
            if k.len() == 1 {
                format!("[{k}]")
            } else {
                format!("[?]")
            }
        } else {
            format!("[ ]")
        };
        if cur.print_button(&b.mouse_state(), &s) {
            actions.push(action);
        }
    };

    // Print a named command for key, also have the text act as a button.
    let mut command_help = |cur: &mut Cursor<'_>, action, name| {
        let s = if let Some(k) = g.input_map.key_for(action) {
            text::input_help_string(&k.to_string(), name)
        } else {
            format!("n/a: {name}")
        };
        if cur.print_button(&b.mouse_state(), &s) {
            actions2.push(action);
        }
        writeln!(cur);
    };

    writeln!(cur, "{}", player.name(&g.r));
    let max_hp = player.max_wounds(&g.r);
    let hp = max_hp - player.wounds(&g.r).min(max_hp);
    writeln!(cur, "{hp} / {max_hp}");

    writeln!(cur);
    writeln!(cur, "------- Controls -------");

    writeln!(cur, "    LMB          RMB");
    write!(cur, "    ");
    command_key(&mut cur, North);
    write!(cur, "          ");
    command_key(&mut cur, FireNorth);
    writeln!(cur);

    write!(cur, " ");
    command_key(&mut cur, West);
    command_key(&mut cur, South);
    command_key(&mut cur, East);
    write!(cur, "    ");
    command_key(&mut cur, FireWest);
    command_key(&mut cur, FireSouth);
    command_key(&mut cur, FireEast);
    writeln!(cur);
    writeln!(cur, "    run          gun");
    writeln!(cur);

    writeln!(cur);
    command_help(&mut cur, Cancel, "cancel orders");
    command_help(&mut cur, Cycle, "cycle NPCs");
    if !player.is_threatened(&g.r) {
        command_help(&mut cur, Autoexplore, "autoexplore");
    } else {
        command_help(&mut cur, Autoexplore, "autofight");
    }
    writeln!(cur, "Ctrl-C) quit");
    // TODO: Command help formatter
    //  - Highlight letter if possible, d)rop, d(r)op, x) drop
    //  - Make the whole word clickable

    // TODO selection helps
    // LMB) select NPC
    // Tab) cycle NPCs
    // Esc) clear selection

    for a in actions.into_iter().chain(actions2) {
        g.process_action(a);
    }
}

/// Draw main game area.
fn draw_main(g: &mut Game, n_updates: u32, win: &Window) {
    if let Some(loc) = g.current_active().and_then(|p| p.loc(&g.r)) {
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
                let mut cell = CharCell::c(e.icon(&g.r));
                if e.is_player_aligned(&g.r) {
                    if g.r.player() == Some(e) {
                        cell.set_c('@');
                    } else if !e.can_be_commanded(&g.r) {
                        // Friendly mob out of moves.
                        cell = cell.col(X::GRAY);
                    } else if e.goal(&g.r) != Goal::FollowPlayer {
                        // Frindly mob out on a mission.
                        cell = cell.col(X::GREEN);
                    } else if e.acts_before_next_player_frame(&g.r) {
                        // Friendly mob ready for next command
                        cell = cell.col(X::AQUA);
                    } else {
                        // Friendly mob still building up it's actions.
                        cell = cell.col(X::TEAL);
                    }

                    if g.current_active() == Some(e) {
                        cell = cell.inv();
                    }
                }
                win.put(&mut g.s, draw_pos, cell);
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
