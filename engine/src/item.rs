//! Entity logic for usable items.
use serde::{Deserialize, Serialize};
use strum::{EnumIter, IntoEnumIterator};

use crate::{ecs::ItemAbility, prelude::*};

#[derive(
    Copy, Clone, Default, Debug, Eq, PartialEq, Serialize, Deserialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum ItemKind {
    // Have a baked-in None value so this can be used directly as a component
    #[default]
    None,
    MeleeWeapon,
    RangedWeapon,
    Armor,
    Ring,
    Scroll,
    Potion,
    Treasure,
}

impl ItemKind {
    pub fn fits(&self, slot: EquippedAt) -> bool {
        use EquippedAt::*;
        use ItemKind::*;
        match self {
            MeleeWeapon | RangedWeapon => slot == RunHand || slot == GunHand,
            Armor => slot == Body,
            Ring => slot == Ring1 || slot == Ring2,
            _ => false,
        }
    }
}

#[derive(
    Copy, Clone, Default, Debug, Eq, PartialEq, Serialize, Deserialize, EnumIter,
)]
#[serde(rename_all = "kebab-case")]
pub enum EquippedAt {
    #[default]
    None,
    RunHand,
    GunHand,
    Body,
    Ring1,
    Ring2,
}

impl EquippedAt {
    pub fn is_some(&self) -> bool {
        !matches!(self, EquippedAt::None)
    }
}

impl Entity {
    pub fn is_item(&self, c: &Core) -> bool {
        !self.is_mob(c)
    }

    pub fn use_needs_aim(&self, _c: &Core) -> bool {
        // TODO 2023-02-01 Make wands etc. require aiming when applied
        false
    }

    pub fn can_be_applied(&self, c: &Core) -> bool {
        self.get::<ItemAbility>(c).0.is_some()
    }

    pub fn is_equipped(&self, c: &Core) -> bool {
        self.equipped_at(c).is_some()
    }

    pub fn can_be_equipped(&self, c: &Core) -> bool {
        use ItemKind::*;
        matches!(
            self.get::<ItemKind>(c),
            MeleeWeapon | RangedWeapon | Armor | Ring
        )
    }

    pub fn equip(&self, c: &mut Core, item: &Entity) {
        if !item.can_be_equipped(c) {
            msg!("You can't equip that.");
            return;
        }

        if item.is_equipped(c) {
            msg!("That is already equipped.");
            return;
        }

        let kind = item.get::<ItemKind>(c);

        let slots: Vec<EquippedAt> = self
            .free_slots(c)
            .into_iter()
            .filter(|&s| kind.fits(s))
            .collect();

        if slots.is_empty() {
            // TODO Try to unequip the item in the way.
            msg!("You can't equip any more of that sort of item.");
            return;
        }

        let slot;
        if kind == ItemKind::RangedWeapon
            && slots.contains(&EquippedAt::GunHand)
        {
            // Always start by equipping a ranged weapon in gun hand even if
            // run hand is also free.
            slot = EquippedAt::GunHand;
        } else {
            // Guaranteed to work since we already covered slots.is_empty.
            slot = slots[0];
        }

        msg!("Equipped {}.", item.name(c));
        item.set(c, slot);
        self.complete_turn(c);
    }

    pub fn is_ranged_weapon(&self, c: &Core) -> bool {
        self.get::<ItemKind>(c) == ItemKind::RangedWeapon
    }

    pub fn unequip(&self, c: &mut Core, item: &Entity) {
        if item.is_equipped(c) {
            item.set(c, EquippedAt::None);
            msg!("Removed {}.", item.name(c));
            self.complete_turn(c);
        } else {
            msg!("That isn't equipped.");
        }
    }

    pub fn free_slots(&self, c: &Core) -> Vec<EquippedAt> {
        let mut ret: Vec<EquippedAt> =
            EquippedAt::iter().filter(|c| c.is_some()).collect();

        for (slot, _) in self.current_equipment(c) {
            if let Some(p) = ret.iter().position(|&a| a == slot) {
                ret.remove(p);
            }
        }

        ret
    }

    pub fn equipped_at(&self, c: &Core) -> EquippedAt {
        self.get(c)
    }

    pub fn current_equipment<'a>(
        &self,
        c: &'a Core,
    ) -> impl Iterator<Item = (EquippedAt, Entity)> + 'a {
        self.contents(c).filter_map(|e| {
            let slot = e.equipped_at(c);
            slot.is_some().then_some((slot, e))
        })
    }

    pub fn equipment_at(&self, c: &Core, slot: EquippedAt) -> Option<Entity> {
        self.contents(c).find(|e| e.equipped_at(c) == slot)
    }

    pub fn consumed_on_use(&self, c: &Core) -> bool {
        use ItemKind::*;
        matches!(self.get::<ItemKind>(c), Scroll | Potion)
    }

    pub(crate) fn take(&self, c: &mut Core, item: &Entity) {
        c.placement.insert_in(self, *item);
        msg!("{} picks up {}.", self.Name(c), item.name(c));
    }

    pub(crate) fn drop(&self, c: &mut Core, item: &Entity) {
        if let Some(loc) = self.loc(c) {
            if item.is_equipped(c) {
                self.unequip(c, item);
            }
            item.place_on_open_spot(c, loc);
            msg!("{} drops {}.", self.Name(c), item.name(c));
        } else {
            log::warn!("Entity::drop: Dropping entity has no location");
        }
        self.complete_turn(c);
    }

    pub(crate) fn use_item(&self, c: &mut Core, item: &Entity, v: IVec2) {
        // TODO 2023-02-01 Item apply logic
        let effect = item.get::<ItemAbility>(c).0;
        let Some(loc) = self.loc(c) else { return };
        if let Some(effect) = effect {
            effect.invoke(c, loc, v, Some(*self));
        }
        if item.consumed_on_use(c) {
            item.destroy(c);
        }
        self.complete_turn(c);
    }

    pub(crate) fn throw(&self, c: &mut Core, item: &Entity, _v: IVec2) {
        // TODO 2023-02-01 Item throw logic
        msg!("Whoosh!");
        self.drop(c, item);
    }
}
