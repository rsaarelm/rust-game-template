//! Entity logic for usable items.

use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};
use strum::{EnumIter, IntoEnumIterator};
use util::{s4, RngExt};

use crate::{ecs::ItemPower, prelude::*, THROW_RANGE};

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

    pub fn icon(&self) -> char {
        use ItemKind::*;
        match self {
            None => 'X',
            MeleeWeapon => ')',
            RangedWeapon => ')',
            Armor => '[',
            Ring => '°',
            Scroll => '?',
            Potion => '!',
            Treasure => '$',
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
    pub fn is_item(&self, r: &impl AsRef<Runtime>) -> bool {
        !self.is_mob(r)
    }

    pub fn use_needs_aim(&self, r: &impl AsRef<Runtime>) -> bool {
        self.get::<ItemPower>(r).0.map_or(false, |p| p.needs_aim())
    }

    pub fn can_be_used(&self, r: &impl AsRef<Runtime>) -> bool {
        self.get::<ItemPower>(r).0.is_some()
    }

    pub fn is_equipped(&self, r: &impl AsRef<Runtime>) -> bool {
        self.equipped_at(r).is_some()
    }

    pub fn can_be_equipped(&self, r: &impl AsRef<Runtime>) -> bool {
        use ItemKind::*;
        matches!(
            self.get::<ItemKind>(r),
            MeleeWeapon | RangedWeapon | Armor | Ring
        )
    }

    pub fn fits(&self, r: &impl AsRef<Runtime>, slot: EquippedAt) -> bool {
        self.get::<ItemKind>(r).fits(slot)
    }

    pub fn equip(&self, r: &mut impl AsMut<Runtime>, item: &Entity) {
        let r = r.as_mut();

        if !item.can_be_equipped(r) {
            msg!("[One] can't equip that."; self.noun(r));
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
            msg!("[One] can't equip any more of that sort of item."; self.noun(r));
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

        msg!("[One] equip[s] [another]."; self.noun(r), item.noun(r));
        item.set(r, slot);
        self.complete_turn(r);
    }

    pub fn is_ranged_weapon(&self, r: &impl AsRef<Runtime>) -> bool {
        self.get::<ItemKind>(r) == ItemKind::RangedWeapon
    }

    /// Detach an equipped item.
    ///
    /// Return whether anything was done.
    pub fn detach(&self, r: &mut impl AsMut<Runtime>) -> bool {
        let r = r.as_mut();

        if self.is_equipped(r) {
            self.set(r, EquippedAt::None);
            true
        } else {
            false
        }
    }

    pub fn unequip(&self, r: &mut impl AsMut<Runtime>, item: &Entity) {
        let r = r.as_mut();

        if item.detach(r) {
            msg!("[One] remove[s] [another]."; self.noun(r), item.noun(r));
            self.complete_turn(r);
        } else {
            msg!("That isn't equipped.");
        }
    }

    pub fn free_slots(&self, r: &impl AsRef<Runtime>) -> Vec<EquippedAt> {
        let mut ret: Vec<EquippedAt> =
            EquippedAt::iter().filter(|c| c.is_some()).collect();

        for (slot, _) in self.equipment(r) {
            if let Some(p) = ret.iter().position(|&a| a == slot) {
                ret.remove(p);
            }
        }

        ret
    }

    pub fn equipped_at(&self, r: &impl AsRef<Runtime>) -> EquippedAt {
        self.get(r)
    }

    pub fn equipment<'a>(
        &self,
        r: &'a impl AsRef<Runtime>,
    ) -> impl Iterator<Item = (EquippedAt, Entity)> + 'a {
        self.contents(r).filter_map(|e| {
            let slot = e.equipped_at(r);
            slot.is_some().then_some((slot, e))
        })
    }

    pub fn inventory<'a>(
        &self,
        r: &'a impl AsRef<Runtime>,
    ) -> impl Iterator<Item = Entity> + 'a {
        self.contents(r).filter(|e| !e.is_equipped(r))
    }

    pub fn equipment_at(
        &self,
        r: &impl AsRef<Runtime>,
        slot: EquippedAt,
    ) -> Option<Entity> {
        self.contents(r).find(|e| e.equipped_at(r) == slot)
    }

    pub fn consumed_on_use(&self, r: &impl AsRef<Runtime>) -> bool {
        use ItemKind::*;
        matches!(self.get::<ItemKind>(r), Scroll | Potion)
    }

    pub(crate) fn take(&self, r: &mut impl AsMut<Runtime>, item: &Entity) {
        let r = r.as_mut();

        item.place(r, *self);
        msg!("[One] pick[s] up [another]."; self.noun(r), item.noun(r));
    }

    pub(crate) fn drop(&self, r: &mut impl AsMut<Runtime>, item: &Entity) {
        let r = r.as_mut();

        if let Some(loc) = self.loc(r) {
            item.place_on_open_spot(r, loc);
            msg!("[One] drop[s] [another]."; self.noun(r), item.noun(r));
        } else {
            log::warn!("Entity::drop: Dropping entity has no location");
        }
        self.complete_turn(r);
    }

    pub(crate) fn use_item(
        &self,
        r: &mut impl AsMut<Runtime>,
        item: &Entity,
        v: IVec2,
    ) {
        let r = r.as_mut();

        let effect = item.get::<ItemPower>(r).0;
        let Some(loc) = self.loc(r) else { return };
        if let Some(effect) = effect {
            effect.invoke(r, Some(*self), loc, v);
        }
        if item.consumed_on_use(r) {
            item.destroy(r);
        }
        self.complete_turn(r);
    }

    pub(crate) fn throw(
        &self,
        r: &mut impl AsMut<Runtime>,
        item: &Entity,
        mut v: IVec2,
    ) {
        let r = r.as_mut();
        let Some(loc) = self.loc(r) else { return };

        // Bad aim when confused.
        let is_confused = self.is_confused(r) && r.rng.one_chance_in(3);
        let mut perp = Some(*self);
        if is_confused {
            v = *s4::DIR.choose(&mut r.rng).unwrap();

            // Perp controls friendly fire in trace, when confused you hit
            // allies.
            perp = None;
        }

        let target = r.trace_target(perp, loc, v, THROW_RANGE as usize);

        if target == loc {
            // No room to throw, just drop it.
            self.drop(r, item);
        } else {
            // Throw time.
            send_msg(Msg::Fire(*self, v));

            if let Some(mob) = target.mob_at(r) {
                if self.try_to_hit(r, &mob) {
                    // TODO Figure out throw damage based on item (and thrower strength?)
                    // TODO Throw to-hit determination should be different than melee, wielded weapon doesn't matter for one thing
                    // TODO Mulch items when they are used as weapons
                    mob.damage(r, Some(*self), 4);
                    msg!("[One] hit[s] [another]."; item.noun(r), mob.noun(r));
                } else {
                    // TODO The projectile should keep flying past the mobs it misses
                    msg!("[One] miss[es] [another]."; item.noun(r), mob.noun(r));
                }
            } else {
                msg!("[One] throw[s] [another]."; self.noun(r), item.noun(r));
            }
            item.place(r, target);
        }
    }
}
