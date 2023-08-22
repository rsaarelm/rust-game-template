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
    pub fn is_item(&self, r: &Runtime) -> bool {
        !self.is_mob(r)
    }

    pub fn use_needs_aim(&self, _c: &Runtime) -> bool {
        // TODO 2023-02-01 Make wands etc. require aiming when applied
        false
    }

    pub fn can_be_applied(&self, r: &Runtime) -> bool {
        self.get::<ItemAbility>(r).0.is_some()
    }

    pub fn is_equipped(&self, r: &Runtime) -> bool {
        self.equipped_at(r).is_some()
    }

    pub fn can_be_equipped(&self, r: &Runtime) -> bool {
        use ItemKind::*;
        matches!(
            self.get::<ItemKind>(r),
            MeleeWeapon | RangedWeapon | Armor | Ring
        )
    }

    pub fn equip(&self, r: &mut Runtime, item: &Entity) {
        if !item.can_be_equipped(r) {
            msg!("You can't equip that.");
            return;
        }

        if item.is_equipped(r) {
            msg!("That is already equipped.");
            return;
        }

        let kind = item.get::<ItemKind>(r);

        let slots: Vec<EquippedAt> = self
            .free_slots(r)
            .into_iter()
            .filter(|&s| kind.fits(s))
            .collect();

        if slots.is_empty() {
            // TODO Try to unequip the item in the way.
            msg!("You can't equip any more of that sort of item.");
            return;
        }

        let slot = if kind == ItemKind::RangedWeapon
            && slots.contains(&EquippedAt::GunHand)
        {
            // Always start by equipping a ranged weapon in gun hand even if
            // run hand is also free.
            EquippedAt::GunHand
        } else {
            // Guaranteed to work since we already covered slots.is_empty.
            slots[0]
        };

        msg!("Equipped {}.", item.name(r));
        item.set(r, slot);
        self.complete_turn(r);
    }

    pub fn is_ranged_weapon(&self, r: &Runtime) -> bool {
        self.get::<ItemKind>(r) == ItemKind::RangedWeapon
    }

    pub fn unequip(&self, r: &mut Runtime, item: &Entity) {
        if item.is_equipped(r) {
            item.set(r, EquippedAt::None);
            msg!("Removed {}.", item.name(r));
            self.complete_turn(r);
        } else {
            msg!("That isn't equipped.");
        }
    }

    pub fn free_slots(&self, r: &Runtime) -> Vec<EquippedAt> {
        let mut ret: Vec<EquippedAt> =
            EquippedAt::iter().filter(|c| c.is_some()).collect();

        for (slot, _) in self.current_equipment(r) {
            if let Some(p) = ret.iter().position(|&a| a == slot) {
                ret.remove(p);
            }
        }

        ret
    }

    pub fn equipped_at(&self, r: &Runtime) -> EquippedAt {
        self.get(r)
    }

    pub fn current_equipment<'a>(
        &self,
        r: &'a Runtime,
    ) -> impl Iterator<Item = (EquippedAt, Entity)> + 'a {
        self.contents(r).filter_map(|e| {
            let slot = e.equipped_at(r);
            slot.is_some().then_some((slot, e))
        })
    }

    pub fn equipment_at(
        &self,
        r: &Runtime,
        slot: EquippedAt,
    ) -> Option<Entity> {
        self.contents(r).find(|e| e.equipped_at(r) == slot)
    }

    pub fn consumed_on_use(&self, r: &Runtime) -> bool {
        use ItemKind::*;
        matches!(self.get::<ItemKind>(r), Scroll | Potion)
    }

    pub(crate) fn take(&self, r: &mut Runtime, item: &Entity) {
        r.placement.insert(*self, *item);
        msg!("{} picks up {}.", self.Name(r), item.name(r));
    }

    pub(crate) fn drop(&self, r: &mut Runtime, item: &Entity) {
        if let Some(loc) = self.loc(r) {
            if item.is_equipped(r) {
                self.unequip(r, item);
            }
            item.place_on_open_spot(r, loc);
            msg!("{} drops {}.", self.Name(r), item.name(r));
        } else {
            log::warn!("Entity::drop: Dropping entity has no location");
        }
        self.complete_turn(r);
    }

    pub(crate) fn use_item(&self, r: &mut Runtime, item: &Entity, v: IVec2) {
        // TODO 2023-02-01 Item apply logic
        let effect = item.get::<ItemAbility>(r).0;
        let Some(loc) = self.loc(r) else { return };
        if let Some(effect) = effect {
            effect.invoke(r, loc, v, Some(*self));
        }
        if item.consumed_on_use(r) {
            item.destroy(r);
        }
        self.complete_turn(r);
    }

    pub(crate) fn throw(&self, r: &mut Runtime, item: &Entity, _v: IVec2) {
        // TODO 2023-02-01 Item throw logic
        msg!("Whoosh!");
        self.drop(r, item);
    }
}
