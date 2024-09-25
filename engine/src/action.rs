//! Entities doing things

use content::{Block, EquippedAt, Power};
use rand::{seq::SliceRandom, Rng};
use serde::{Deserialize, Serialize};
use util::{s4, RngExt};

use crate::{
    ecs::{ActsNext, Momentum, Voice},
    prelude::*,
    ALERT_RADIUS, PHASES_IN_TURN, SHOUT_RADIUS,
};

impl Entity {
    /// Execute action commanded by goal AI.
    pub fn execute_indirect(
        &self,
        r: &mut impl AsMut<Runtime>,
        action: Action,
    ) {
        self.execute(r, action, false);
    }

    /// Execute action using a direct command.
    ///
    /// Can do things like pick up items automatically.
    pub fn execute_direct(&self, r: &mut impl AsMut<Runtime>, action: Action) {
        self.execute(r, action, true);
    }

    fn execute(
        &self,
        r: &mut impl AsMut<Runtime>,
        action: Action,
        is_direct: bool,
    ) {
        use Action::*;
        let r = r.as_mut();

        let is_confused = self.is_confused(r) && r.rng.one_chance_in(3);
        let confusion_dir = if is_confused {
            Some(*s4::DIR.choose(&mut r.rng).unwrap())
        } else {
            None
        };

        let modified_dir = |dir| confusion_dir.unwrap_or(dir);

        match action {
            Pass => self.pass(r, is_direct),
            Bump(dir) => {
                let dir = modified_dir(dir);
                let succeeded = self.attack_step(r, dir, is_direct);

                if !succeeded && self.is_player(r) {
                    // Player bumps into altar, request altar menu.
                    if let Some(loc) =
                        self.loc(r).map(|loc| loc + dir.extend(0))
                    {
                        if loc.voxel(r) == Some(Block::Altar) {
                            send_msg(Msg::ActivatedAltar(loc));
                        }
                    }
                }
            }
            Shoot(dir) => {
                self.shoot(r, modified_dir(dir));
            }
            Drop(item) => self.drop(r, &item),
            Use(item, dir) => {
                if is_confused {
                    msg!("[One] stare[s] at [another]."; self.noun(r), item.noun(r));
                } else {
                    self.use_item(r, &item, dir);
                }
            }
            Cast(power, dir) => self.cast(r, power, modified_dir(dir)),
            Throw(item, dir) => self.throw(r, &item, modified_dir(dir)),
            Equip(item) => self.equip(r, &item),
            Unequip(item) => self.unequip(r, &item),
        }
    }

    fn step(
        &self,
        r: &mut impl AsMut<Runtime>,
        dir: IVec2,
        is_direct: bool,
    ) -> bool {
        let r = r.as_mut();
        debug_assert!(dir.taxi_len() == 1);

        let Some(loc) = self.loc(r) else { return false };

        let Some(new_loc) = loc.walk_step(r, dir) else {
            return false;
        };

        // Assume terrain is valid, there might be a displaceable friendly
        // mob.

        let mut displace = None;

        if let Some(mob) = new_loc.mob_at(r) {
            if self.can_displace(r, dir, &mob, is_direct) {
                displace = Some(mob);
                r.placement.remove(&mob);
            }
        }

        if self.can_enter(r, new_loc) {
            self.place(r, new_loc);
            self.set(r, Momentum(dir));

            // Put the displaced mob where this one was.
            if let Some(mob) = displace {
                r.placement.insert(loc, mob);
            }

            // Pick up items when moving with a direct command.
            if is_direct {
                if let Some(item) = self.loc(r).and_then(|loc| loc.item_at(r)) {
                    self.take(r, &item);
                }
            }

            // This is walking, so we only complete a phase, not a full turn.
            self.complete_phase(r);

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
    fn attack_step(
        &self,
        r: &mut impl AsMut<Runtime>,
        dir: IVec2,
        is_direct: bool,
    ) -> bool {
        let r = r.as_mut();

        if let Some(mob) = self.target_for_attack(r, dir, EquippedAt::RunHand) {
            self.attack(r, mob);
            return true;
        }

        self.step(r, dir, is_direct)
    }

    fn shoot(&self, r: &mut impl AsMut<Runtime>, dir: IVec2) {
        let r = r.as_mut();

        if let Some(mob) = self.target_for_attack(r, dir, EquippedAt::GunHand) {
            self.attack(r, mob);
        }
    }

    fn pass(&self, r: &mut impl AsMut<Runtime>, is_direct: bool) {
        let r = r.as_mut();

        if self.is_npc(r) && is_direct {
            // If you tell a NPC to wait, exhaust all the actions.
            while self.can_be_commanded(r) {
                self.complete_phase(r);
            }
        } else {
            self.complete_phase(r);
        }
    }

    /// Mark the entity as having taken a long action.
    pub(crate) fn complete_turn(&self, r: &mut impl AsMut<Runtime>) {
        let r = r.as_mut();
        let t = self.acts_next(r).max(r.now());
        self.set(r, ActsNext(t + PHASES_IN_TURN));
    }

    /// Mark the entity as having taken a short action.
    fn complete_phase(&self, r: &mut impl AsMut<Runtime>) {
        let r = r.as_mut();
        self.set(r, ActsNext(self.next_phase_frame(r)));
    }

    fn attack(&self, r: &mut impl AsMut<Runtime>, target: Entity) {
        let r = r.as_mut();

        if let Some(d) = self.vec_towards(r, &target) {
            if d.taxi_len() > 1 {
                send_msg(Msg::Fire(*self, d.to_dir4()));
            }
        }

        if self.try_to_hit(r, &target) {
            let dmg = self.stats(r).dmg;
            target.damage(r, Some(*self), dmg);
        } else {
            send_msg(Msg::Miss(target));
        }

        self.complete_turn(r);
    }

    pub fn try_to_hit(
        &self,
        r: &mut impl AsMut<Runtime>,
        other: &Entity,
    ) -> bool {
        let r = r.as_mut();

        let odds = Odds(self.to_hit(r) - other.evasion(r));
        r.rng().sample(odds)
    }

    pub(crate) fn shout(
        &self,
        r: &mut impl AsMut<Runtime>,
        enemy: Option<&Entity>,
    ) {
        let r = r.as_mut();

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
    pub(crate) fn alert_to(
        &self,
        r: &mut impl AsMut<Runtime>,
        enemy: &Entity,
    ) -> bool {
        let r = r.as_mut();

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
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Action {
    Pass,
    // Mixed step and melee attack. Split to separate "Step" and "Attack"
    // actions later if there's need.
    Bump(IVec2),
    Shoot(IVec2),
    Drop(Entity),
    Cast(Power, IVec2),
    Use(Entity, IVec2),
    Throw(Entity, IVec2),
    Equip(Entity),
    Unequip(Entity),
}
