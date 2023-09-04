use anyhow::{bail, Result};
use engine::{prelude::*, EquippedAt};

#[derive(Copy, Clone, Debug, Default)]
pub enum CommandState {
    #[default]
    None,
    Partial(Part),
    Complete(Command),
}

use CommandState::{Complete, Partial};

#[derive(Copy, Clone, Debug)]
pub enum Command {
    Direct(Action),
    Indirect(Goal),
}

impl From<Action> for Command {
    fn from(value: Action) -> Self {
        Command::Direct(value)
    }
}

impl From<Goal> for Command {
    fn from(value: Goal) -> Self {
        Command::Indirect(value)
    }
}

impl From<Part> for CommandState {
    fn from(value: Part) -> Self {
        Partial(value)
    }
}

impl<T: Into<Command>> From<T> for CommandState {
    fn from(value: T) -> Self {
        Complete(value.into())
    }
}

impl CommandState {
    pub fn is_some(self) -> bool {
        !matches!(self, CommandState::None)
    }

    /// Extract a completed command and clear the command state.
    pub fn pop(&mut self) -> Option<Command> {
        if let CommandState::Complete(cmd) = self {
            let ret = *cmd;
            *self = CommandState::None;
            Some(ret)
        } else {
            None
        }
    }

    /// A partial command needs an inventory item.
    pub fn needs_item(&self) -> bool {
        matches!(self, Partial(p) if p.needs_item())
    }

    /// A partial command needs an equipped item.
    pub fn needs_equipment(&self) -> bool {
        matches!(self, Partial(p) if p.needs_equipment())
    }

    /// The partial command accepts given item as input.
    pub fn matches_item(&self, r: &Runtime, item: Entity) -> bool {
        matches!(self, Partial(p) if p.matches_item(r, item))
    }

    /// The partial command needs to be aimed.
    pub fn needs_direction(&self) -> bool {
        matches!(self, Partial(p) if p.needs_direction())
    }

    pub fn add_item(&mut self, r: &Runtime, item: Entity) {
        if let Partial(p) = self {
            if let Ok(next) = p.add_item(r, item) {
                *self = next;
            }
        }
    }

    pub fn add_direction(&mut self, dir: IVec2) {
        if let Partial(p) = self {
            if let Ok(next) = p.add_direction(dir) {
                *self = next;
            }
        }
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// Incomplete command that needs further parameters before being deployable.
#[derive(Copy, Clone, Debug)]
pub enum Part {
    Drop,
    Throw,
    Use,
    /// Show inventory screen, allows equipping items if selected.
    ViewInventory,
    /// Show equipment screen, allows equip/unequip.
    ViewEquipment,
    /// Expects equipment from inventory.
    EquipForSlot(EquippedAt),
    /// Selected an item to throw, prompting for direction.
    AimThrow(Entity),
    /// Selected an item to use, prompting for direction.
    AimUse(Entity),
    /// Direction for inherent power.
    AimCast(Power),
}

use Part::*;

impl Part {
    fn needs_item(&self) -> bool {
        matches!(self, Drop | Throw | Use | EquipForSlot(_) | ViewInventory)
    }

    fn needs_equipment(&self) -> bool {
        matches!(self, ViewEquipment)
    }

    fn needs_direction(&self) -> bool {
        matches!(self, AimThrow(_) | AimUse(_) | AimCast(_))
    }

    pub fn matches_any_contents(self, r: &Runtime, container: Entity) -> bool {
        container.contents(r).any(|item| self.matches_item(r, item))
    }

    pub fn matches_item(self, r: &Runtime, item: Entity) -> bool {
        match self {
            Use => item.can_be_used(r),
            EquipForSlot(slot) => item.fits(r, slot) && !item.is_equipped(r),
            // If the action is item type agnostic or if matching items
            // doesn't apply to begin with, just return true.
            _ => true,
        }
    }

    fn add_item(self, r: &Runtime, item: Entity) -> Result<CommandState> {
        match self {
            Drop => Ok(Action::Drop(item).into()),
            Throw => Ok(AimThrow(item).into()),
            Use => {
                if item.use_needs_aim(r) {
                    Ok(AimUse(item).into())
                } else {
                    Ok(Action::Use(item, Default::default()).into())
                }
            }
            // TODO: Actually use the slot here, if equipping for gun hand,
            // don't put it in empty run hand. Needs a change in
            // Action::Equip.
            EquipForSlot(_) => Ok(Action::Equip(item).into()),
            ViewInventory => {
                if item.can_be_equipped(r) {
                    Ok(Action::Equip(item).into())
                } else {
                    Ok(CommandState::None)
                }
            }
            AimThrow(_) | AimUse(_) | AimCast(_) | ViewEquipment => {
                bail!("does not accept item")
            }
        }
    }

    fn add_direction(self, dir: IVec2) -> Result<CommandState> {
        match self {
            Drop | Throw | Use | ViewEquipment | ViewInventory
            | EquipForSlot(_) => {
                bail!("does not accept direction")
            }
            AimThrow(e) => Ok(Action::Throw(e, dir).into()),
            AimUse(e) => Ok(Action::Use(e, dir).into()),
            AimCast(a) => Ok(Action::Cast(a, dir).into()),
        }
    }
}
