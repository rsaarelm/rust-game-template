use std::fmt::Write;

use content::EquippedAt;
use engine::prelude::*;
use navni::Key;
use ui::prelude::*;
use util::{text, write, writeln};

pub fn item_list(
    win: &Window,
    mob: Entity,
    filter: impl Fn(&Entity) -> bool,
) -> Option<Entity> {
    win.clear();
    let g = game();

    let keys = "abcdefghijklmnopqrstuvwxyz";
    let items: Vec<Entity> = mob.contents(g).filter(filter).collect();

    let mut cur = Cursor::new(*win);

    let keypress = navni::keypress();

    for (k, e) in keys.chars().zip(items.into_iter()) {
        if cur.print_button(&format!("{k}) {}", e.name(&g.r)))
            || keypress.key() == Key::Char(k)
        {
            return Some(e);
        }
        writeln!(cur);
    }

    None
}

pub fn inventory_filter(e: &Entity) -> bool {
    !e.is_equipped(game())
}

pub fn usable_filter(e: &Entity) -> bool {
    !e.is_equipped(game()) && e.can_be_used(game())
}

pub fn equipment_filter(e: &Entity) -> bool {
    e.is_equipped(game())
}

pub struct StatusPanel(pub Entity);

impl Widget for StatusPanel {
    type Output = InputAction;

    fn render(&self, win: &Window) -> Option<Self::Output> {
        use InputAction::*;

        let g = game();
        let player = self.0;

        let mut cur = Cursor::new(*win);
        // Two of these just so that both closures below get one to borrow.
        // They all get merged into one output at the end.
        let mut actions = Vec::new();
        let mut actions2 = Vec::new();

        // Print help for a key, also have it act as a button that dispatches the
        // action when clicked.
        let mut command_key = |cur: &mut Cursor, action| {
            let s = if let Some(k) = g.input_map.key_for(action) {
                // These are supposed to always be single-char, snip to one
                // character here just in case they're something weird
                let k = k.to_string();
                match k.as_ref() {
                    k if k.len() == 1 => format!("[{k}]"),
                    "Up" => "[↑]".into(),
                    "Right" => "[→]".into(),
                    "Down" => "[↓]".into(),
                    "Left" => "[←]".into(),
                    _ => "[?]".into(),
                }
            } else {
                "[ ]".into()
            };
            if cur.print_button(&s) {
                actions.push(action);
            }
        };

        // Print a named command for key, also have the text act as a button.
        let mut command_help = |cur: &mut Cursor, action, name| {
            let s = if let Some(k) = g.input_map.key_for(action) {
                text::input_help_string(&k.to_string(), name)
            } else {
                format!("--: {name}")
            };
            if cur.print_button(&s) {
                actions2.push(action);
            }
        };

        writeln!(cur, "{}", player.name(g));
        let max_hp = player.max_wounds(g);
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

        cur.pos.y = win.height() - 1;
        cur.pos.x = 0;
        if cur.print_button("forfeit run") {
            actions2.push(ForfeitRun);
        }

        // There should be at most one input action caught up in these, return
        // that as the widget return value.
        actions.into_iter().chain(actions2).next()
    }
}
