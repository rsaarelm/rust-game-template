use std::fmt::Write;

use engine::{prelude::*, EquippedAt};
use navni::prelude::*;
use ui::prelude::*;
use util::{s4, text, v2, write, writeln};

use navni::X256Color as X;

pub fn run(b: &mut dyn Backend, n: u32, g: &mut Game) {
    g.tick(b);

    let win = Window::from(&g.s);
    if let Some(player) = g.current_active() {
        // Launch a different UI mode based on what the current partial
        // command needs next.
        if g.cmd.needs_item() {
            inventory_mode(b, n, g, win, player);
        } else if g.cmd.needs_equipment() {
            equipment_mode(b, n, g, win, player);
        } else if g.cmd.needs_direction() {
            aim_mode(b, n, g, win, player);
        } else {
            // No command in progress, default mode.
            move_mode(b, n, g, win, player);
        }
    } else {
        // No player can be found, just show the map.
        attract_mode(b, n, g, win);
    }

    // Execute completed commands.
    if let Some(cmd) = g.cmd.pop() {
        g.act(cmd)
    }

    g.draw(b);
}

/// Show when there's no player.
fn attract_mode(b: &mut dyn Backend, n: u32, g: &mut Game, win: Window) {
    win.clear(&mut g.s);
    let main = win;
    let mouse = b.mouse_state();
    draw_main(g, n, &main, mouse);

    let mut cur = Cursor::new(&mut g.s, main);
    for m in g.msg.iter() {
        writeln!(cur, "{m}");
    }

    // INPUT
    if let Some(&a) = g.input_map.get(&b.keypress()) {
        use ui::InputAction::*;

        match a {
            QuitGame => b.quit(),
            Quickload => g.load(crate::GAME_NAME),
            _ => {}
        }
    }
}

/// The default mode, player is exploring map with a status panel.
fn move_mode(
    b: &mut dyn Backend,
    n: u32,
    g: &mut Game,
    win: Window,
    player: Entity,
) {
    win.clear(&mut g.s);
    let (panel, main) = win.split_left(26);
    status_panel(g, b, &panel, player);

    let mouse = b.mouse_state();
    draw_main(g, n, &main, mouse);

    let mut cur = Cursor::new(&mut g.s, main);
    for m in g.msg.iter() {
        writeln!(cur, "{m}");
    }

    // INPUT
    if let Some(&a) = g.input_map.get(&b.keypress()) {
        use ui::InputAction::*;
        g.process_action(a);

        match a {
            QuitGame => b.quit(),
            Quicksave => g.save(crate::GAME_NAME),
            Quickload => g.load(crate::GAME_NAME),
            _ => {}
        }
    }
}

/// Status panel shows inventory.
fn inventory_mode(
    b: &mut dyn Backend,
    n: u32,
    g: &mut Game,
    win: Window,
    player: Entity,
) {
    win.clear(&mut g.s);
    let (panel, main) = win.split_left(26);

    let keys = "abcdefghijklmnopqrstuvwxyz";
    let items: Vec<Entity> = player
        .contents(&g.r)
        .filter(|e| !e.is_equipped(&g.r) && g.cmd.matches_item(&g.r, *e))
        .collect();

    let mouse = b.mouse_state();
    let mut cur = Cursor::new(&mut g.s, panel);

    for (k, e) in keys.chars().zip(items.into_iter()) {
        if cur.print_button(&b.mouse_state(), &format!("{k}) {}", e.name(&g.r)))
            || b.keypress().key() == Key::Char(k)
        {
            g.cmd.add_item(&g.r, e);
        }
        writeln!(cur);
    }

    // TODO: Disable mouse on main when running modes (maybe draw_main can just return hover coords?)
    draw_main(g, n, &main, mouse);

    if let Some(&a) = g.input_map.get(&b.keypress()) {
        use ui::InputAction::*;

        match a {
            Cancel => g.cmd.reset(),
            _ => {}
        }
    }
}

/// Status panel shows equipment.
fn equipment_mode(
    b: &mut dyn Backend,
    n: u32,
    g: &mut Game,
    win: Window,
    player: Entity,
) {
    win.clear(&mut g.s);
    let (panel, main) = win.split_left(26);

    // TODO: Show stable set of equipment slots
    // TODO: Selecting an empty slot prompts for item.

    let keys = "abcdefghijklmnopqrstuvwxyz";
    let items: Vec<Entity> = player
        .contents(&g.r)
        .filter(|e| e.is_equipped(&g.r) && g.cmd.matches_item(&g.r, *e))
        .collect();

    let mouse = b.mouse_state();
    let mut cur = Cursor::new(&mut g.s, panel);

    for (k, e) in keys.chars().zip(items.into_iter()) {
        if cur.print_button(&b.mouse_state(), &format!("{k}) {}", e.name(&g.r)))
            || b.keypress().key() == Key::Char(k)
        {
            g.cmd = Action::Unequip(e).into();
        }
        writeln!(cur);
    }

    draw_main(g, n, &main, mouse);

    if let Some(&a) = g.input_map.get(&b.keypress()) {
        use ui::InputAction::*;

        match a {
            Cancel => g.cmd.reset(),
            _ => {}
        }
    }
}

/// Player needs to aim.
fn aim_mode(
    b: &dyn Backend,
    n: u32,
    g: &mut Game,
    win: Window,
    player: Entity,
) {
    win.clear(&mut g.s);

    let (panel, main) = win.split_left(26);
    status_panel(g, b, &panel, player);

    let mouse = b.mouse_state();
    draw_main(g, n, &main, mouse);

    let mut cur = Cursor::new(&mut g.s, main);
    writeln!(cur, "Direction?");

    // TODO Allow mouse aiming as well.

    if let Some(&a) = g.input_map.get(&b.keypress()) {
        use ui::InputAction::*;

        match a {
            North | FireNorth => g.cmd.add_direction(s4::DIR[0]),
            East | FireEast => g.cmd.add_direction(s4::DIR[1]),
            South | FireSouth => g.cmd.add_direction(s4::DIR[2]),
            West | FireWest => g.cmd.add_direction(s4::DIR[3]),
            Cancel => g.cmd.reset(),
            _ => {}
        }
    }
}

fn status_panel(g: &mut Game, b: &dyn Backend, win: &Window, player: Entity) {
    use InputAction::*;

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
            format!("--: {name}")
        };
        if cur.print_button(&b.mouse_state(), &s) {
            actions2.push(action);
        }
    };

    writeln!(cur, "{}", player.name(&g.r));
    let max_hp = player.max_wounds(&g.r);
    let hp = max_hp - player.wounds(&g.r).min(max_hp);
    writeln!(cur, "{hp} / {max_hp}");

    writeln!(cur);
    writeln!(cur, "------- Controls -------");

    // Only show help for the second set of directions when it does something.
    let show_gun = player.equipment_at(&g.r, EquippedAt::GunHand).is_some();

    write!(cur, "  LMB/run");
    if show_gun {
        write!(cur, "      RMB/gun");
    }
    writeln!(cur);
    write!(cur, "    ");
    command_key(&mut cur, North);

    if show_gun {
        write!(cur, "          ");
        command_key(&mut cur, FireNorth);
    }

    writeln!(cur);

    write!(cur, " ");
    command_key(&mut cur, West);
    command_key(&mut cur, South);
    command_key(&mut cur, East);

    if show_gun {
        write!(cur, "    ");
        command_key(&mut cur, FireWest);
        command_key(&mut cur, FireSouth);
        command_key(&mut cur, FireEast);
    }
    writeln!(cur);

    writeln!(cur);

    let has_inventory = player.inventory(&g.r).next().is_some();
    let has_usables = player.inventory(&g.r).any(|e| e.can_be_used(&g.r));
    let has_equipment = player.equipment(&g.r).next().is_some();

    if !player.is_threatened(&g.r) {
        command_help(&mut cur, Roam, "roam");
    } else {
        command_help(&mut cur, Roam, "rumble");
    }
    cur.pos.x = win.width() / 2;
    if has_usables {
        command_help(&mut cur, Use, "use");
    }
    writeln!(cur);

    if has_inventory {
        command_help(&mut cur, Inventory, "inventory");
    }
    if has_equipment {
        cur.pos.x = win.width() / 2;
        command_help(&mut cur, Equipment, "equipment");
    }
    writeln!(cur);

    if has_inventory {
        command_help(&mut cur, Drop, "drop");
        cur.pos.x = win.width() / 2;
        command_help(&mut cur, Throw, "throw");
    }
    writeln!(cur);

    command_help(&mut cur, Cycle, "cycle");
    cur.pos.x = win.width() / 2;
    command_help(&mut cur, Pass, "wait");
    writeln!(cur);
    command_help(&mut cur, Cancel, "cancel");

    for a in actions.into_iter().chain(actions2) {
        g.process_action(a);
    }
}

/// Draw main game area.
fn draw_main(g: &mut Game, n_updates: u32, win: &Window, mouse: MouseState) {
    if let Some(loc) = g.current_active().and_then(|p| p.loc(&g.r)) {
        if g.camera != loc {
            // Clear path whenever player moves.
            g.clear_projected_path();
        }
        g.camera = loc;
    }

    let wide_sector_bounds = wide_unfolded_sector_bounds(g.camera);
    let offset = util::scroll_offset(
        &win.area(),
        g.camera.unfold_wide(),
        &wide_sector_bounds,
    );

    let screen_to_wide_pos =
        |screen_pos: [i32; 2]| v2(screen_pos) - v2(win.bounds().min()) + offset;

    let screen_to_loc_pos = |screen_pos: [i32; 2]| {
        // Get wide location pos corresponding to screen space pos.
        let wide_pos = screen_to_wide_pos(screen_pos);
        // Snap to cell.
        ivec2(wide_pos.x.div_euclid(2), wide_pos.y)
    };

    // Get a click target, preferring cells with mobs in them.
    let click_target = |g: &Game, wide_pos: IVec2| -> Location {
        let (a, b) = Location::fold_wide_sides(wide_pos);
        // Prefer left cell unless right has a mob and left doesn't.
        if b.mob_at(&g.r).is_some() && a.mob_at(&g.r).is_none() {
            b
        } else {
            a
        }
    };

    // Solid background for off-sector extra space.
    win.fill(&mut g.s, CharCell::c('█').col(X::BROWN));
    // Constrain sub-window to current sector only.
    let sector_win = win.sub(wide_sector_bounds - offset);
    // Adjust offset for sub-window position.
    let offset = v2(wide_sector_bounds.min()).max(offset);
    draw_map(g, &sector_win, offset);
    g.draw_anims(n_updates, &sector_win, offset);
    draw_fog(g, &sector_win, offset);

    if win.contains(mouse) && g.current_active().is_some() {
        // Only operate within the currently visible sector.
        let sector_bounds = g.camera.expanded_sector_bounds();

        match mouse {
            MouseState::Hover(p) => {
                let a = screen_to_loc_pos(p);

                if sector_bounds.contains(a) {
                    g.project_path_to(Location::fold(a));
                }
            }

            MouseState::Drag(p, q, MouseButton::Left) if win.contains(q) => {
                let (a, b) = (screen_to_loc_pos(q), screen_to_loc_pos(p));

                // Draw inverted marquee box.
                if sector_bounds.contains(a) && sector_bounds.contains(b) {
                    for a in Rect::from_points([p, q]) - win.bounds().min() {
                        if let Some(c) = win.get_mut(&mut g.s, a) {
                            *c = c.inv();
                        }
                    }
                }
            }

            MouseState::Release(p, q, MouseButton::Left) => {
                // Was this a local click or the end result of a drag?
                let (a, b) = (screen_to_wide_pos(q), screen_to_wide_pos(p));
                if a == b {
                    // Left click.
                    let loc = click_target(g, a);

                    match loc.mob_at(&g.r) {
                        Some(npc) if npc.is_player(&g.r) => {
                            // Select player.
                            g.clear_selection();
                        }
                        Some(npc) if npc.is_player_aligned(&g.r) => {
                            // Select NPC.
                            g.set_selection(vec![npc]);
                        }
                        Some(_enemy) if g.player_is_selected() => {
                            // Player group gets a move command that gets
                            // transformed into autofight when near enough.
                            g.act(Goal::GoTo(loc));
                        }
                        Some(enemy) => {
                            // NPCs get a direct kill task instead.
                            g.act(Goal::Attack(enemy));
                        }
                        None => {
                            // Move to location.
                            g.act(Goal::GoTo(loc));
                        }
                    }
                } else {
                    // A drag ended. Collect covered friendly units into
                    // selection.
                    if wide_sector_bounds.contains(a)
                        && wide_sector_bounds.contains(b)
                    {
                        let (a, b) =
                            (screen_to_wide_pos(q), screen_to_wide_pos(p));
                        let mut selection = Vec::new();
                        for e in Rect::from_points([a, b])
                            .into_iter()
                            .filter_map(|p| {
                                Location::fold_wide(p)
                                    .and_then(|loc| loc.mob_at(&g.r))
                            })
                        {
                            if e.is_player_aligned(&g.r) {
                                selection.push(e);
                            }
                        }
                        g.set_selection(selection);
                    }
                }
            }

            MouseState::Release(p, q, MouseButton::Right) => {
                let (a, b) = (screen_to_wide_pos(q), screen_to_wide_pos(p));
                if a == b {
                    // Right click.
                    let loc = click_target(g, a);

                    match loc.mob_at(&g.r) {
                        Some(npc) if npc.is_player_aligned(&g.r) => {
                            npc.become_player(&mut g.r);
                        }
                        Some(enemy) => {
                            // Attack enemy.
                            g.act(Goal::Attack(enemy));
                        }
                        None => {
                            // TODO: Shoot in direction
                        }
                    }
                }
            }

            _ => {}
        }
    }
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

                    if g.selected().any(|a| a == e) {
                        cell = cell.inv();
                    }
                }
                win.put(&mut g.s, draw_pos, cell);
            } else if let Some(e) = loc.item_at(&g.r) {
                win.put(&mut g.s, draw_pos, CharCell::c(e.icon(&g.r)));
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
