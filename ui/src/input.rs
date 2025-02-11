use derive_more::Deref;
use engine::Action;
use glam::ivec2;
use navni::{Key, KeyTyped};
use serde::{Deserialize, Serialize};
use util::{IndexMap, Layout};

use crate::game;

/// Convenience method, get input action from current navni keypress if you
/// can make one.
pub fn input_press() -> Option<InputAction> {
    game()
        .input_map
        .get(&navni::keypress().ignore_repeat_flag())
        .copied()
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub enum InputAction {
    North,
    South,
    West,
    East,
    FireNorth,
    FireSouth,
    FireWest,
    FireEast,
    SouthEast,
    SouthWest,
    NorthWest,
    NorthEast,
    TravelNorth,
    TravelSouth,
    TravelWest,
    TravelEast,
    TravelUp,
    TravelDown,
    Cycle,
    Pass,
    Inventory,
    Powers,
    Drop,
    Throw,
    Use,
    QuitGame,
    Retire,
    Cancel,
    Roam,
    BecomePlayer,
    ScrollNorth,
    ScrollSouth,
    ScrollWest,
    ScrollEast,
}

#[derive(Clone, Deref, Eq, PartialEq, Serialize, Deserialize)]
pub struct InputMap(IndexMap<KeyTyped, InputAction>);

impl Default for InputMap {
    fn default() -> Self {
        use InputAction::*;

        let mut ret: IndexMap<KeyTyped, InputAction> = Default::default();

        // NB. Order matters, first binding for command is the main binding
        // that's reported by key_for.
        for (k, cmd) in &[
            ("w", North),
            ("a", West),
            ("s", South),
            ("d", East),
            ("i", FireNorth),
            ("j", FireWest),
            ("k", FireSouth),
            ("l", FireEast),
            ("Up", North),
            ("Left", West),
            ("Down", South),
            ("Right", East),
            ("PgDn", SouthEast),
            ("End", SouthWest),
            ("Home", NorthWest),
            ("PgUp", NorthEast),
            ("W", TravelNorth),
            ("A", TravelWest),
            ("S", TravelSouth),
            ("D", TravelEast),
            ("<", TravelUp),
            (">", TravelDown),
            ("Tab", Cycle),
            ("Sp", Pass),
            ("h", Inventory),
            ("z", Powers),
            ("x", Drop),
            ("t", Throw),
            ("c", Use),
            ("C-c", QuitGame),
            ("C-q", Retire),
            ("Esc", Cancel),
            ("r", Roam),
            ("Ret", BecomePlayer),
            ("S-Up", ScrollNorth),
            ("S-Left", ScrollWest),
            ("S-Down", ScrollSouth),
            ("S-Right", ScrollEast),
        ] {
            ret.insert(
                k.parse::<KeyTyped>()
                    .expect("Error in InputMap::default map"),
                *cmd,
            );
        }

        InputMap(ret)
    }
}

impl InputMap {
    pub fn for_layout(layout: Layout) -> Self {
        let mut ret = IndexMap::default();
        for (k, a) in InputMap::default().0 {
            let key = k.key();
            if let Key::Char(c) = key {
                ret.insert(
                    KeyTyped::new(
                        Key::Char(layout.remap_from_qwerty(c)),
                        k.mods(),
                        false,
                    ),
                    a,
                );
            } else {
                ret.insert(k, a);
            }
        }

        InputMap(ret)
    }

    /// Find the key for the given action.
    pub fn key_for(&self, action: InputAction) -> Option<KeyTyped> {
        self.0
            .iter()
            .find_map(|(k, v)| (*v == action).then_some(*k))
    }
}

impl TryFrom<InputAction> for Action {
    type Error = ();

    fn try_from(value: InputAction) -> Result<Self, Self::Error> {
        use InputAction::*;
        match value {
            North => Ok(Action::Bump(ivec2(0, -1))),
            South => Ok(Action::Bump(ivec2(0, 1))),
            West => Ok(Action::Bump(ivec2(-1, 0))),
            East => Ok(Action::Bump(ivec2(1, 0))),
            FireNorth => Ok(Action::Shoot(ivec2(0, -1))),
            FireSouth => Ok(Action::Shoot(ivec2(0, 1))),
            FireWest => Ok(Action::Shoot(ivec2(-1, 0))),
            FireEast => Ok(Action::Shoot(ivec2(1, 0))),
            Pass => Ok(Action::Pass),
            _ => Err(()),
        }
    }
}
