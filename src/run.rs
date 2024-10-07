use std::fmt::Write;

use content::{settings, DOWN, EAST, NORTH, SOUTH, UP, WEST};
use engine::prelude::*;
use navni::X256Color as X;
use ui::{ask, prelude::*};
use util::{wizard_mode, writeln};

use crate::{
    map_view::{view_map, MapAction::*},
    view,
};

pub async fn main_gameplay() {
    loop {
        game().tick().await;
        game().draw().await;

        let mut win = Window::root();
        win.foreground_col = X::BROWN;

        let mut main = win;
        let mut side = Window::default();
        let mut side_action = None;

        if let Some(p) = game().current_active() {
            (side, main) = win.box_border().box_split_left(26);
            side_action = view::StatusPanel(p).render(&side);
        }

        main.box_caption(&game().camera.region_name(game()));

        match view_map(&main) {
            Some(DirectCommand(act)) => game().act(act),
            Some(Order(goal)) => game().act(goal),
            Some(RepositionCamera(loc)) => game().camera = loc,
            Some(HoverOver(loc)) => {
                if loc.is_explored(game()) {
                    if let Some(desc) = loc.describe(game()) {
                        let (mut text_box, _) = main.split_bottom(1);
                        text_box.bounds += ivec2(0, 1);
                        text_box.write_center(&desc);
                    }
                }

                if let Some(orig) =
                    game().current_active().and_then(|p| p.loc(game()))
                {
                    let mouse_pos = navni::mouse_state().cursor_pos();

                    // Clear path when hovering over a friendly mob, clicks
                    // have different function then, otherwise try pathing to
                    // cell.
                    let cursor_on_friendly = matches!(
                        loc.mob_at(game()),
                        Some(npc) if npc.is_player_aligned(game()));
                    if cursor_on_friendly {
                        game().planned_path.clear();
                    } else {
                        game().planned_path.update(
                            game(),
                            orig,
                            loc.ui_path_destination(game()),
                            mouse_pos,
                        );
                    }
                }
            }
            Some(SelectActive(sel)) => game().set_selection(sel),
            Some(BecomePlayer(e)) => e.become_player(game()),
            Some(NextEntity) => game().select_next_commandable(false),
            None => {}
        }

        // Print messages.
        let mut cur = Cursor::new(main);
        for m in game().msg.iter() {
            writeln!(cur, "{m}");
        }

        // Print ambient message to bottom of view.
        if let Some(desc) = game()
            .current_active()
            .and_then(|p| p.loc(game()))
            .and_then(|loc| loc.ambient_description(game()))
        {
            if !desc.is_empty() {
                let mut cur = Cursor::new(main.split_bottom(1).0);
                writeln!(cur, "{desc}");
            }
        }

        if let Some(side_action) = side_action {
            game().process_action(side_action);
        }

        // XXX: Explicitly save the game whenever Esc is pressed.
        // This is for the WASM build where there's no natural "close
        // application" event that can be intercepted so no natural point
        // where to save the game.
        if navni::keypress().is("Esc") && !game().is_game_over() {
            game().save(&settings().id);
            msg!("Game saved.");
        }

        // Debug keys, not for regular gameplay.
        if wizard_mode() {
            // Quit without saving so you can return to save.
            if navni::keypress().is("C-z") {
                // Use panic so the TTY cleanup hook will get tripped.
                panic!("no-save emergency exit triggered");
            }

            if navni::keypress().is("!") {
                if let Some(player) = game().r.player() {
                    msg!("Powered up to level {}", player.level_up(game()));
                }
            }
        }

        match input_press().or(side_action) {
            Some(InputAction::Inventory) if !side.is_zero() => {
                match inventory_choice(&side).await {
                    Some(e) if e.can_be_used(game()) => {
                        if ask(format!("Use {}?", e.noun(game()).the_name()))
                            .await
                        {
                            if let Some(dir) = if e.use_needs_aim(game()) {
                                aim(&main).await
                            } else {
                                Some(Default::default())
                            } {
                                game().act(Action::Use(e, dir));
                            }
                        }
                    }
                    Some(e) if e.can_be_equipped(game()) => {
                        game().act(Action::Equip(e));
                    }
                    Some(_) => {}
                    None => {}
                }
            }
            Some(InputAction::Equipment) if !side.is_zero() => {
                if let Some(e) = equipment_choice(&side).await {
                    game().act(Action::Unequip(e));
                }
            }
            Some(InputAction::Drop) if !side.is_zero() => {
                if let Some(e) = inventory_choice(&side).await {
                    game().act(Action::Drop(e));
                }
            }
            Some(InputAction::Throw) if !side.is_zero() => {
                if let Some(e) = inventory_choice(&side).await {
                    if let Some(dir) = aim(&main).await {
                        game().act(Action::Throw(e, dir));
                    }
                }
            }
            Some(InputAction::Use) if !side.is_zero() => {
                if let Some(e) = usable_choice(&side).await {
                    if let Some(dir) = if e.use_needs_aim(game()) {
                        aim(&main).await
                    } else {
                        Some(Default::default())
                    } {
                        game().act(Action::Use(e, dir));
                    }
                }
            }
            Some(InputAction::Cancel) if !side.is_zero() => {
                if let Some(p) = game().current_active() {
                    if p.is_player(game()) {
                        p.clear_goal(game());
                    } else {
                        p.set_goal(game(), Goal::FollowPlayer);
                    }
                }
                game().set_selection([]);
            }
            Some(InputAction::Roam) if !side.is_zero() => {
                if let Some(p) = game().current_active() {
                    if let Some(loc) = p.loc(game()) {
                        if !game().autofight(p) {
                            game().act(Goal::StartAutoexplore(loc.sector()));
                        }
                    }
                }
            }
            Some(InputAction::QuitGame) => {
                break;
            }
            Some(InputAction::Retire) => {
                if ask("Really retire your character?").await {
                    game().retire();
                    break;
                }
            }
            Some(InputAction::TravelNorth) => game().travel(NORTH),
            Some(InputAction::TravelEast) => game().travel(EAST),
            Some(InputAction::TravelSouth) => game().travel(SOUTH),
            Some(InputAction::TravelWest) => game().travel(WEST),
            Some(InputAction::TravelUp) => game().travel(UP),
            Some(InputAction::TravelDown) => game().travel(DOWN),
            _ => {}
        }
    }
}

async fn inventory_choice(panel: &Window) -> Option<Entity> {
    let _backdrop = Backdrop::from(*panel);

    if let Some(p) = game().current_active() {
        if p.inventory(game()).next().is_none() {
            msg!("[One] [is] not carrying anything."; p.noun(game()));
            return None;
        }
    }

    while let Some(p) = game().current_active() {
        // Cancel if resized.
        game().draw().await?;

        if let Some(e) = view::item_list(panel, p, view::inventory_filter) {
            return Some(e);
        }

        if input_press() == Some(InputAction::Cancel) {
            break;
        }
    }

    None
}

async fn usable_choice(panel: &Window) -> Option<Entity> {
    let _backdrop = Backdrop::from(*panel);

    if let Some(p) = game().current_active() {
        if !p.inventory(game()).any(|e| e.can_be_used(game())) {
            msg!("[One] [has] nothing usable."; p.noun(game()));
            return None;
        }
    }

    while let Some(p) = game().current_active() {
        // Cancel if resized.
        game().draw().await?;

        if let Some(e) = view::item_list(panel, p, view::usable_filter) {
            return Some(e);
        }

        if input_press() == Some(InputAction::Cancel) {
            break;
        }
    }

    None
}

async fn equipment_choice(panel: &Window) -> Option<Entity> {
    let _backdrop = Backdrop::from(*panel);

    if let Some(p) = game().current_active() {
        if p.equipment(game()).next().is_none() {
            msg!("[One] [has] nothing equipped."; p.noun(game()));
        }
    }

    while let Some(p) = game().current_active() {
        game().draw().await?;

        if let Some(e) = view::item_list(panel, p, view::equipment_filter) {
            return Some(e);
        }

        if input_press() == Some(InputAction::Cancel) {
            break;
        }
    }

    None
}

async fn aim(main: &Window) -> Option<IVec2> {
    writeln!(Cursor::new(*main), "Direction? ");

    loop {
        use InputAction::*;

        game().draw().await?;

        match input_press() {
            Some(North) | Some(FireNorth) => return Some(ivec2(0, -1)),
            Some(East) | Some(FireEast) => return Some(ivec2(1, 0)),
            Some(South) | Some(FireSouth) => return Some(ivec2(0, 1)),
            Some(West) | Some(FireWest) => return Some(ivec2(-1, 0)),
            Some(Cancel) => break,
            _ => {}
        }
    }

    None
}
