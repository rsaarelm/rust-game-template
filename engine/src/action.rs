//! Entities doing things

use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::{
    ability::Ability,
    ecs::{ActsNext, Momentum, Voice},
    prelude::*,
    EquippedAt, ALERT_RADIUS, PHASES_IN_TURN, SHOUT_RADIUS,
};

impl Entity {
    fn execute(&self, r: &mut Runtime, action: Action, is_direct: bool) {
        use Action::*;

        match action {
            Pass => self.pass(r),
            Bump(dir) => {
                self.attack_step(r, dir);
                // Pick up items when moving with a direct command.
                if is_direct {
                    if let Some(item) =
                        self.loc(r).and_then(|loc| loc.item_at(r))
                    {
                        self.take(r, &item);
                    }
                }
            }
            Shoot(dir) => {
                self.shoot(r, dir);
            }
            Drop(item) => self.drop(r, &item),
            Use(item, dir) => self.use_item(r, &item, dir),
            Cast(ability, dir) => self.cast(r, ability, dir),
            Throw(item, dir) => self.throw(r, &item, dir),
            Equip(item) => self.equip(r, &item),
            Unequip(item) => self.unequip(r, &item),
        }
    }

    /// Execute action
    pub fn execute_indirect(&self, r: &mut Runtime, action: Action) {
        self.execute(r, action, false);
    }

    /// Execute action using a direct command.
    ///
    /// Can do things like pick up items automatically.
    pub fn execute_direct(&self, r: &mut Runtime, action: Action) {
        self.execute(r, action, true);
    }

    fn step(&self, r: &mut Runtime, dir: IVec2) -> bool {
        debug_assert!(dir.taxi_len() == 1);

        let Some(loc) = self.loc(r) else { return false };
        let new_loc = (loc + dir).follow(r);

        // Early exit here if the target terrain is unwalkable.
        if !new_loc.is_walkable(r) {
            return false;
        }

        // Assume terrain is valid, there might be a displaceable friendly
        // mob.

        let mut displace = None;

        if let Some(mob) = new_loc.mob_at(r) {
            if self.can_displace(r, dir, &mob) {
                displace = Some(mob);
                r.placement.remove(&mob);
            }
        }

        if self.can_enter(r, new_loc) {
            self.place(r, new_loc);
            self.set(r, Momentum(dir));
            // This is walking, so we only complete a phase, not a full turn.
            self.complete_phase(r);

            // Put the displaced mob where this one was.
            if let Some(mob) = displace {
                r.placement.insert(loc, mob);
            }
            true
        } else {
            // Put the displaced mob back where it was.
            if let Some(mob) = displace {
                r.placement.insert(new_loc, mob);
            }
            false
        }
    }

    /// Attack if running into enemy.
    fn attack_step(&self, r: &mut Runtime, dir: IVec2) -> bool {
        if let Some(mob) = self.target_for_attack(r, dir, EquippedAt::RunHand) {
            self.attack(r, mob);
            return true;
        }

        self.step(r, dir)
    }

    fn shoot(&self, r: &mut Runtime, dir: IVec2) {
        if let Some(mob) = self.target_for_attack(r, dir, EquippedAt::GunHand) {
            self.attack(r, mob);
        }
    }

    fn pass(&self, r: &mut Runtime) {
        self.complete_phase(r);
    }

    /// Mark the entity as having taken a long action.
    pub(crate) fn complete_turn(&self, r: &mut Runtime) {
        let t = self.acts_next(r).max(r.now());
        self.set(r, ActsNext(t + PHASES_IN_TURN));
    }

    /// Mark the entity as having taken a short action.
    fn complete_phase(&self, r: &mut Runtime) {
        self.set(r, ActsNext(self.next_phase_frame(r)));
    }

    fn attack(&self, r: &mut Runtime, target: Entity) {
        if let Some(d) = self.vec_towards(r, &target) {
            if d.taxi_len() > 1 {
                send_msg(Msg::Fire(*self, d.to_dir4()));
            }
        }

        if self.try_to_hit(r, &target) {
            let dmg = self.stats(r).dmg;
            target.damage(r, dmg, Some(*self));
        } else {
            send_msg(Msg::Miss(target));
        }

        self.complete_turn(r);
    }

    pub fn try_to_hit(&self, r: &mut Runtime, other: &Entity) -> bool {
        let stats = self.stats(r);
        let other_stats = other.stats(r);

        let odds = Odds(stats.hit - other_stats.ev);
        r.rng().sample(odds)
    }

    pub(crate) fn shout(&self, r: &mut Runtime, enemy: Option<&Entity>) {
        match self.get::<Voice>(r) {
            Voice::Silent => {
                return;
            }
            Voice::Shout => {
                msg!("[One] shout[s] angrily."; self.noun(r));
            }
            Voice::Hiss => {
                msg!("[One] hiss[es]."; self.noun(r));
            }
            Voice::Gibber => {
                msg!("[One] gibber[s]."; self.noun(r));
            }
            Voice::Roar => {
                msg!("[One] roar[s]."; self.noun(r));
            }
        }

        // The shout alerts nearby other mobs.
        let mobs = self.fov_mobs(r, SHOUT_RADIUS);
        for m in mobs {
            if m != *self && m.is_ally(r, self) {
                if let Some(enemy) = enemy {
                    m.alert_to(r, enemy);
                }
                // Not doing anything for the case where enemy isn't know for
                // now. Might have the mobs wake up and start roaming
                // randomly.
            }
        }
    }

    /// Alert a mob to the presence of another entity.
    ///
    /// Return whether mob was actually alerted
    pub(crate) fn alert_to(&self, r: &mut Runtime, enemy: &Entity) -> bool {
        match self.vec_towards(r, enemy) {
            None => return false,
            Some(v) if v.taxi_len() > ALERT_RADIUS => return false,
            _ => {}
        }
        if self.is_looking_for_fight(r) {
            self.set_goal(r, Goal::Attack(*enemy));
            return true;
        }
        false
    }
}

/// Atomic single-step actions.
#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub enum Action {
    Pass,
    // Mixed step and melee attack. Split to separate "Step" and "Attack"
    // actions later if there's need.
    Bump(IVec2),
    Shoot(IVec2),
    Drop(Entity),
    Cast(Ability, IVec2),
    Use(Entity, IVec2),
    Throw(Entity, IVec2),
    Equip(Entity),
    Unequip(Entity),
}
