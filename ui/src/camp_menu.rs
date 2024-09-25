use std::fmt::Write;

use navni::X256Color as X;
use util::writeln;

use crate::{ask, prelude::*};

#[derive(Clone, Debug)]
pub enum CampAction {
    Rest,
    LevelUp,

    // TODO Spell selection needs list of actually selected
    // spells...
    SelectSpells,

    /// Bring permakilled enemies around camp back to life for grinding.
    ReviveSpirits,
}

async fn render(win: &Window) -> Option<CampAction> {
    use CampAction::*;

    let win = win.box_border();

    let mut cur = Cursor::new(win);

    let key = navni::keypress();

    if cur.print_button("R)est and heal") || key.is("r") {
        return Some(Rest);
    }
    writeln!(cur);

    // TODO: Dim out out the level up option if you don't have enough cash for
    // next level.
    if cur.print_button("Raise e)ssence") || key.is("e") {
        // There needs to be a level-up menu here if there's stat or perk
        // selections involved.
        return Some(LevelUp);
    }
    writeln!(cur);

    if cur.print_button("Attune s)pells") || key.is("s") {
        // TODO: Submenu function to select spells.
        return Some(SelectSpells);
    }
    writeln!(cur);

    if cur.print_button("Re(v)ive spirits") || key.is("v") {
        if ask("Disturb the restless dead?").await {
            return Some(ReviveSpirits);
        }
    }
    writeln!(cur);

    None
}

pub async fn camp() -> Option<CampAction> {
    let mut win = Window::root();
    win.foreground_col = X::FOREGROUND;

    loop {
        if game().draw().await.is_none() {
            return None;
        }

        if let Some(ret) = render(&win).await {
            return Some(ret);
        }

        if navni::keypress().is("Esc") {
            break;
        }
    }
    None
}
