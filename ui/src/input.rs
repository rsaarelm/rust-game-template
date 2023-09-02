use derive_deref::Deref;
use navni::{Key, KeyTyped};
use serde::{Deserialize, Serialize};
use util::{IndexMap, Layout};

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
    ClimbUp,
    ClimbDown,
    LongMove,
    Cycle,
    Pass,
    Inventory,
    Abilities,
    Equipment,
    Drop,
    Throw,
    Use,
    QuitGame,
    Cancel,
    Autoexplore,
    Quicksave,
    Quickload,
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
            ("<", ClimbUp),
            (">", ClimbDown),
            ("g", LongMove),
            ("Tab", Cycle),
            ("Sp", Pass),
            ("h", Inventory),
            ("z", Abilities),
            ("y", Equipment),
            ("r", Drop),
            ("t", Throw),
            ("c", Use),
            ("C-c", QuitGame),
            ("Esc", Cancel),
            ("x", Autoexplore),
            ("F5", Quicksave),
            ("F9", Quickload),
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
