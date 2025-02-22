use std::fmt::Write;

use navni::X256Color as X;
use util::writeln;

use crate::prelude::*;

#[derive(Clone, Debug)]
pub enum CampAction {
    LevelUp,

    // TODO Spell selection needs list of actually selected
    // spells...
    SelectSpells,

    Leave,
}

async fn render(win: &Window) -> Option<CampAction> {
    use CampAction::*;

    let win = win.box_border();

    let mut cur = Cursor::new(win);

    let key = navni::keypress();
    let player = game().r.player()?;

    // Dim out the level-up when you don't have enough cash
    if !player.can_afford_level_up(game()) {
        cur.win.foreground_col = X::GRAY;
    }

    if cur.print_button(&format!(
        "Raise e)ssence ({}$)",
        player.level_up_cost(game())
    )) || key.is("e")
    {
        // There needs to be a level-up menu here if there's stat or perk
        // selections involved.
        return Some(LevelUp);
    }
    cur.win.foreground_col = X::BROWN;
    writeln!(cur);

    if cur.print_button("Attune s)pells") || key.is("s") {
        // TODO: Submenu function to select spells.
        return Some(SelectSpells);
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
