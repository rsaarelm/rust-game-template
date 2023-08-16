use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::{
    ability::Ability,
    ecs::{ActsNext, Momentum, Voice},
    prelude::*,
    EquippedAt, ALERT_RADIUS, PHASES_IN_TURN, SHOUT_RADIUS,
};

impl Entity {
    pub fn execute(&self, c: &mut Core, action: Action) {
        use Action::*;

        match action {
            Pass => self.pass(c),
            Bump(dir) => {
                self.attack_step(c, dir);
            }
            Shoot(dir) => {
                self.shoot(c, dir);
            }
            Drop(item) => self.drop(c, &item),
            Use(item, dir) => self.use_item(c, &item, dir),
            Cast(ability, dir) => self.cast(c, ability, dir),
            Throw(item, dir) => self.throw(c, &item, dir),
            Equip(item) => self.equip(c, &item),
            Unequip(item) => self.unequip(c, &item),
        }
    }

    fn step(&self, c: &mut Core, dir: IVec2) -> bool {
        debug_assert!(dir.taxi_len() == 1);

        let Some(loc) = self.loc(c) else { return false };
        let new_loc = (loc + dir).fold(c);

        // Early exit here if the target terrain is unwalkable.
        if !new_loc.is_walkable(c) {
            return false;
        }

        // Assume terrain is valid, there might be a displaceable friendly
        // mob.

        let mut displace = None;

        if let Some(mob) = new_loc.mob_at(c) {
            if self.can_displace(c, dir, &mob) {
                displace = Some(mob);
                c.placement.remove(&mob);
            }
        }

        if self.can_enter(c, new_loc) {
            self.place(c, new_loc);
            self.set(c, Momentum(dir));
            // This is walking, so we only complete a phase, not a full turn.
            self.complete_phase(c);

            // Put the displaced mob where this one was.
            if let Some(mob) = displace {
                c.placement.insert(loc, mob);
            }
            true
        } else {
            // Put the displaced mob back where it was.
            if let Some(mob) = displace {
                c.placement.insert(new_loc, mob);
            }
            false
        }
    }

    /// Attack if running into enemy.
    fn attack_step(&self, c: &mut Core, dir: IVec2) -> bool {
        if let Some(mob) = self.attack_target(c, dir, EquippedAt::RunHand) {
            self.attack(c, mob);
            return true;
        }

        self.step(c, dir)
    }

    fn shoot(&self, c: &mut Core, dir: IVec2) {
        if let Some(mob) = self.attack_target(c, dir, EquippedAt::GunHand) {
            self.attack(c, mob);
        }
    }

    fn pass(&self, c: &mut Core) {
        self.complete_phase(c);
    }

    /// Mark the entity as having taken a long action.
    pub(crate) fn complete_turn(&self, c: &mut Core) {
        let t = self.acts_next(c).max(c.now());
        self.set(c, ActsNext(t + PHASES_IN_TURN));
    }

    /// Mark the entity as having taken a short action.
    fn complete_phase(&self, c: &mut Core) {
        self.set(c, ActsNext(self.next_phase_frame(c)));
    }

    fn attack(&self, c: &mut Core, target: Entity) {
        if let Some(d) = self.vec_towards(c, &target) {
            if d.taxi_len() > 1 {
                send_msg(Msg::Fire(*self, d.to_dir4()));
            }
        }

        if self.try_to_hit(c, &target) {
            let dmg = self.stats(c).dmg;
            target.damage(c, dmg);
        } else {
            send_msg(Msg::Miss(target));
        }

        self.complete_turn(c);
    }

    pub fn try_to_hit(&self, c: &mut Core, other: &Entity) -> bool {
        let stats = self.stats(c);
        let other_stats = other.stats(c);

        let odds = Odds(stats.hit - other_stats.ev);
        c.rng().sample(odds)
    }

    pub(crate) fn shout(&self, c: &mut Core, enemy: Option<&Entity>) {
        match self.get::<Voice>(c) {
            Voice::Silent => {
                return;
            }
            Voice::Shout => {
                msg!("{} shouts angrily.", self.Name(c));
            }
            Voice::Hiss => {
                msg!("{} hisses.", self.Name(c));
            }
            Voice::Gibber => {
                msg!("{} gibbers.", self.Name(c));
            }
            Voice::Roar => {
                msg!("{} roars.", self.Name(c));
            }
        }

        // The shout alerts nearby other mobs.
        let mobs = self.fov_mobs(c, SHOUT_RADIUS);
        for m in mobs {
            if m != *self && m.is_ally(c, self) {
                if let Some(enemy) = enemy {
                    m.alert_to(c, enemy);
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
    pub(crate) fn alert_to(&self, c: &mut Core, enemy: &Entity) -> bool {
        match self.vec_towards(c, enemy) {
            None => return false,
            Some(v) if v.taxi_len() > ALERT_RADIUS => return false,
            _ => {}
        }
        if self.is_looking_for_fight(c) {
            self.set_goal(c, Goal::Attack(*enemy));
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
