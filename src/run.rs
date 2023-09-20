use std::{borrow::Cow, fmt::Write};

use engine::prelude::*;
use navni::X256Color as X;
use ui::{prelude::*, ConfirmationDialog};
use util::writeln;

use crate::{
    map_view::{view_map, MapAction::*},
    view,
};

pub async fn explore() {
    loop {
        game().tick();
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
                    game().planned_path.update(game(), orig, loc, mouse_pos);
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

        if let Some(side_action) = side_action {
            game().process_action(side_action);
        }

        if navni::keypress() == "C-c".parse().unwrap() {
            break;
        }

        match input_press().or(side_action) {
            Some(InputAction::Inventory) if !side.is_zero() => {
                match inventory_choice(&side).await {
                    Some(e) if e.can_be_used(game()) => {
                        if ask(format!(
                            "Activate {}?",
                            e.noun(game()).the_name()
                        ))
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
                    if !game().autofight(p) {
                        game().act(Goal::StartAutoexplore);
                    }
                }
            }
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

async fn ask(msg: impl Into<Cow<'_, str>>) -> bool {
    let dialog = ConfirmationDialog::new(msg);
    let mut win = Window::root().center(dialog.preferred_size().unwrap());
    win.foreground_col = X::BROWN;

    let _backdrop = Backdrop::from(win);
    loop {
        if game().draw().await.is_none() {
            return false;
        }

        if let Some(ret) = dialog.render(&win) {
            return ret;
        }
    }
}
