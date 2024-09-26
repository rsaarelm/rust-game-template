use std::fmt::Write;

use navni::X256Color as X;
use util::writeln;

use crate::{ask, prelude::*};

#[derive(Clone, Debug)]
pub enum CampAction {
    LevelUp,

    // TODO Spell selection needs list of actually selected
    // spells...
    SelectSpells,

    /// Bring permakilled enemies around camp back to life for grinding.
    ReviveSpirits,

    Leave,
}

async fn render(win: &Window) -> Option<CampAction> {
    use CampAction::*;

    let win = win.box_border();

    let mut cur = Cursor::new(win);

    let key = navni::keypress();

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
    writeln!(cur);

    if cur.print_button("Esc) Leave") || key.is("Esc") {
        return Some(Leave);
    }
    writeln!(cur);

    None
}

pub async fn camp() -> CampAction {
    let mut win = Window::root();
    win.foreground_col = X::BROWN;

    loop {
        if game().draw().await.is_none() {
            return CampAction::Leave;
        }

        if let Some(ret) = render(&win).await {
            return ret;
        }
    }
}
