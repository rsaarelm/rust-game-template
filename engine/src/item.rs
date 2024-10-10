//! Entity logic for usable items.

use content::{EquippedAt, ItemKind};
use rand::seq::SliceRandom;
use strum::IntoEnumIterator;
use util::{s4, RngExt};

use crate::{
    ecs::{Cash, Count, IsEphemeral, ItemPower},
    prelude::*,
    THROW_RANGE,
};

impl Entity {
    pub fn is_item(&self, r: &impl AsRef<Runtime>) -> bool {
        !self.is_mob(r)
    }

    pub fn can_stack_with(
        &self,
        r: &impl AsRef<Runtime>,
        other: &Entity,
    ) -> bool {
        // Both items must be designated stackable.
        if self.get::<Count>(r).0 == 0 || other.get::<Count>(r).0 == 0 {
            return false;
        }

        // Both items must be non-unique.
        if self.is_unique(r) || other.is_unique(r) {
            return false;
        }

        // NB. I'm not even trying to make this more robust than this. It's on
        // the data designer to only designate things as stackable with the
        // 'count' component if items with that name cannot vary otherwise.

        self.is_item(r)
            && other.is_item(r)
            && self.base_desc(r) == other.base_desc(r)
    }

    pub fn count(&self, r: &impl AsRef<Runtime>) -> i32 {
        match self.get::<Count>(r).0 {
            // The component value is 0 for un-stackable entities, but they
            // still logically have a count of 1.
            x if x <= 1 => 1,
            x => x,
        }
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

    /// Return slots this piece of equipment can go in.
    /// Always returns the preferred slot first.
    pub fn valid_slots(
        &self,
        r: &impl AsRef<Runtime>,
    ) -> impl IntoIterator<Item = &'static EquippedAt> {
        use EquippedAt::*;
        use ItemKind::*;
        match self.get::<ItemKind>(r) {
            MeleeWeapon => &[RunHand][..],
            RangedWeapon => &[GunHand, RunHand][..],
            Armor => &[Body][..],
            Ring => &[Ring1, Ring2][..],
            _ => &[],
        }
    }

    /// Find a free slot for the item in a mob.
    ///
    /// If not found, return the blocking item and slot.
    pub fn find_slot_in(
        &self,
        r: &impl AsRef<Runtime>,
        mob: &Entity,
    ) -> std::result::Result<EquippedAt, (EquippedAt, Entity)> {
        let mut ret = Ok(EquippedAt::None);

        for &slot in self.valid_slots(r) {
            if let Some(e) = mob.equipment_at(r, slot) {
                // If colliding at the first (preferred) slot, mark return
                // value with that slot and entity to be unequipped.
                if ret.is_ok() {
                    ret = Err((slot, e));
                }
            } else {
                return Ok(slot);
            }
        }

        ret
    }

    pub fn fits(&self, r: &impl AsRef<Runtime>, slot: EquippedAt) -> bool {
        self.get::<ItemKind>(r).fits(slot)
    }

    /// Internal equip method, does not give message feedback or pass turn.
    ///
    /// Return true when item wasn't equipped before and is equipped now.
    pub fn make_equipped(
        &self,
        r: &mut impl AsMut<Runtime>,
        item: &Entity,
    ) -> bool {
        let r = r.as_mut();

        if item.is_equipped(r) {
            return false;
        }

        let slot = match item.find_slot_in(r, self) {
            Ok(slot) => slot,
            Err((slot, previous)) => {
                // Unequip previous thing if it's in the way.
                self.unequip(r, &previous);
                slot
            }
        };

        if slot.is_none() {
            return false;
        }

        item.set(r, slot);
        true
    }

    pub fn equip(&self, r: &mut impl AsMut<Runtime>, item: &Entity) {
        let r = r.as_mut();

        if item.is_equipped(r) {
            msg!("That is already equipped.");
            return;
        }

        if self.make_equipped(r, item) {
            msg!("[One] equip[s] [another]."; self.noun(r), item.noun(r));
            self.complete_turn(r);
        } else {
            msg!("[One] can't equip that."; self.noun(r));
        }
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
        self.contents(r)
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

        if self.is_player(r) && item.is_cash(r) {
            // Cash items get deleted and added to cash component.
            let n = item.count(r);
            self.with_mut::<Cash, _>(r, |Cash(c)| *c += n);
            item.destroy(r);
        } else {
            // Regular pick-up.
            item.place(r, *self);
        }

        msg!("[One] pick[s] up [another]."; self.noun(r), item.noun(r));
    }

    pub(crate) fn drop(&self, r: &mut impl AsMut<Runtime>, item: &Entity) {
        let r = r.as_mut();

        if let Some(loc) = self.loc(r) {
            item.place_near(r, loc);
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
            r.invoke_power(effect, Some(*self), loc, v);
        }
        if item.consumed_on_use(r) {
            item.consume(r);
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

        // If it's a stack of items, just throw one.
        let item = item.split_off_one(r);

        if target == loc {
            // No room to throw, just drop it.
            self.drop(r, &item);
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

    pub fn carried_cash(&self, r: &impl AsRef<Runtime>) -> i32 {
        self.get::<Cash>(r).0
    }

    pub(crate) fn subtract_cash(
        &self,
        r: &mut impl AsMut<Runtime>,
        amount: i32,
    ) -> bool {
        let r = r.as_mut();

        let balance = self.carried_cash(r);
        if balance >= amount {
            self.set(r, Cash(balance - amount));
            true
        } else {
            false
        }
    }

    pub(crate) fn is_cash(&self, r: &impl AsRef<Runtime>) -> bool {
        self.base_desc(r) == "silver coin"
    }

    /// If the mob is carrying cash, drop all of it into a pile and return the
    /// pile entity.
    pub(crate) fn drop_wallet(
        &self,
        r: &mut impl AsMut<Runtime>,
    ) -> Option<Entity> {
        let r = r.as_mut();

        let Cash(cash) = self.get(r);
        if cash <= 0 {
            return None;
        }

        self.set(r, Cash(0));

        let loc = self.loc(r)?;
        let pile = r.spawn_cash_at(cash, loc);
        pile.set(r, IsEphemeral(true));

        Some(pile)
    }
}
