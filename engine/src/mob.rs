//! Entity logic for active creatures.
use serde::{Deserialize, Serialize};

use crate::{
    ecs::{
        ActsNext, Buffs, IsEphemeral, IsMob, Momentum, NumDeaths, Speed, Stats,
        Wounds,
    },
    prelude::*,
    PHASES_IN_TURN,
};

impl Entity {
    pub fn is_player(&self, r: &impl AsRef<Runtime>) -> bool {
        let r = r.as_ref();
        r.player == Some(*self)
    }

    pub fn become_player(&self, r: &mut impl AsMut<Runtime>) {
        let r = r.as_mut();
        let prev_player = r.player();
        if Some(*self) == prev_player {
            return;
        }

        msg!("You are now [one]."; self.noun(r));

        r.player = Some(*self);
        // Clear goal, existing ones probably won't make sense for a
        // player mob.
        self.clear_goal(r);

        if let Some(prev_player) = prev_player {
            // Give the previous player mob a follower AI so it won't just
            // stand around.
            if prev_player != *self {
                prev_player.set_goal(r, Goal::FollowPlayer);
            }
        }
    }

    pub fn is_mob(&self, r: &impl AsRef<Runtime>) -> bool {
        self.get::<IsMob>(r).0
    }

    pub fn is_npc(&self, r: &impl AsRef<Runtime>) -> bool {
        self.is_player_aligned(r) && !self.is_player(r)
    }

    /// Return the vector the mob is moving along "right now".
    ///
    /// Used for displace deadlocks, a mob with existing momentum shouldn't be
    /// displaced against the momentum vector.
    pub fn live_momentum(&self, r: &impl AsRef<Runtime>) -> IVec2 {
        let r = r.as_ref();
        if self.acts_next(r) > r.now() {
            // Only display momentum if the mob has moved during the current
            // phase.
            self.get::<Momentum>(r).0
        } else {
            Default::default()
        }
    }

    pub fn can_step(&self, r: &impl AsRef<Runtime>, dir: IVec2) -> bool {
        let r = r.as_ref();

        let Some(n) = self.loc(r).and_then(|loc| loc.walk_step(r, dir)) else {
            return false;
        };

        if let Some(mob) = n.mob_at(r) {
            if !self.can_displace(r, dir, &mob, false) {
                return false;
            }
        }

        true
    }

    pub fn can_displace(
        &self,
        r: &impl AsRef<Runtime>,
        _dir: IVec2,
        other: &Entity,
        is_direct_move: bool,
    ) -> bool {
        // Can't displace enemies.
        if !self.is_ally(r, other) {
            return false;
        }

        // The player, and other mobs if they're commanded directly, can
        // displace regardless of momentum.
        if self.is_player(r) || is_direct_move {
            return true;
        }

        // Don't displace the player when you're not executing a direct
        // command.
        if other.is_player(r) {
            return false;
        }

        // Don't displace other mobs if they're already in motion.
        //
        // (There used to be a clever thing here where you could still
        // displace the mob if you helped it move further along it's momentum,
        // but it still lead to pathing deadlocks. Going to just keep things
        // simple.)
        let m = other.live_momentum(r);
        if m != IVec2::ZERO {
            return false;
        }

        true
    }

    pub fn acts_next(&self, r: &impl AsRef<Runtime>) -> Instant {
        self.get::<ActsNext>(r).0
    }

    pub fn acts_this_frame(&self, r: &impl AsRef<Runtime>) -> bool {
        let r = r.as_ref();
        self.get::<Speed>(r).0 > 0 && self.acts_next(r) <= r.now()
    }

    pub fn acts_before_next_player_frame(
        &self,
        r: &impl AsRef<Runtime>,
    ) -> bool {
        let r = r.as_ref();
        if let Some(player) = r.player() {
            self.acts_next(r) <= player.next_phase_frame(r)
        } else {
            self.acts_this_frame(r)
        }
    }

    pub fn can_be_commanded(&self, r: &impl AsRef<Runtime>) -> bool {
        let r = r.as_ref();
        // NPCs can be commanded up to one full turn into the future.
        self.is_alive(r) && self.acts_next(r) - r.now() < PHASES_IN_TURN
    }

    pub fn is_waiting_commands(&self, r: &impl AsRef<Runtime>) -> bool {
        self.can_be_commanded(r)
            && matches!(self.goal(r), Goal::None | Goal::FollowPlayer)
    }

    /// Special method to immediately run goals on a NPC.
    ///
    /// Returns false if there is no goal or the NPC can't be commanded
    /// anymore on this turn.
    ///
    /// NB. Since this is meant for running explicit orders on NPCs, it does
    /// nothing and returns false if the goal is the default `FollowPlayer`.
    pub fn exhaust_actions(&self, r: &mut impl AsMut<Runtime>) {
        let r = r.as_mut();
        if !self.is_npc(r) {
            return;
        }

        // XXX: Goals might spin wheels forever, so add a release valve of
        // only spinning for a limited number of rounds.
        for _failsafe in 0..32 {
            let goal = self.goal(r);
            if !self.can_be_commanded(r) {
                break;
            }

            if matches!(goal, Goal::None | Goal::FollowPlayer) {
                break;
            }

            if let Some(act) = self.decide(r, goal) {
                self.execute_indirect(r, act);
            } else {
                self.next_goal(r);
            }
        }
    }

    pub(crate) fn next_phase_frame(&self, r: &impl AsRef<Runtime>) -> Instant {
        let r = r.as_ref();
        let mut t = self.acts_next(r).max(r.now()) + 1;
        let speed = self.get::<Speed>(r).0;
        assert!(speed > 0);

        while !t.is_action_frame(speed) {
            t += 1;
        }
        t
    }

    /// Return current stats for an entity, factoring in its equipment.
    ///
    /// This method should always be used when querying the stats of a mob
    /// during gameplay, the raw `Stats` component has the base stats that
    /// don't include bonuses from equipment.
    pub fn stats(&self, r: &impl AsRef<Runtime>) -> Stats {
        let mut stats = self.get::<Stats>(r);
        for (_, e) in self.equipment(r) {
            stats += e.stats(r);
        }
        stats
    }

    pub fn to_hit(&self, r: &impl AsRef<Runtime>) -> i32 {
        let stats = self.stats(r);
        stats.might + stats.hit
    }

    pub fn evasion(&self, r: &impl AsRef<Runtime>) -> i32 {
        self.stats(r).ev
    }

    pub fn confuse(&self, r: &mut impl AsMut<Runtime>) {
        let r = r.as_mut();
        msg!("[One] [is] confused."; self.noun(r));
        self.buff(r, Buff::Confusion, 40);
    }

    pub fn buff(&self, r: &mut impl AsMut<Runtime>, buff: Buff, duration: i64) {
        let r = r.as_mut();
        let now = r.now();
        self.with_mut::<Buffs, _>(r, |b| b.insert(buff, now + duration));
    }

    pub fn has_buff(&self, r: &impl AsRef<Runtime>, buff: Buff) -> bool {
        let r = r.as_ref();
        self.with::<Buffs, _>(r, |b| {
            b.get(&buff).map_or(false, |&e| e >= r.now())
        })
    }

    pub fn expired_buffs(&self, r: &mut impl AsMut<Runtime>) -> Vec<Buff> {
        let r = r.as_mut();
        let mut ret = Vec::new();

        let now = r.now();
        self.with_mut::<Buffs, _>(r, |b| {
            for (b, t) in b.iter() {
                if *t < now {
                    ret.push(*b);
                }
            }
        });

        ret
    }

    pub fn is_confused(&self, r: &impl AsRef<Runtime>) -> bool {
        self.has_buff(r, Buff::Confusion)
    }

    pub fn fully_heal(&self, r: &mut impl AsMut<Runtime>) {
        self.set(r, Wounds(0));
    }

    pub fn respawn(&self, r: &mut impl AsMut<Runtime>) {
        let r = r.as_mut();

        // Clear out all ephemeral items at respawn. This is the failed corpse
        // run mechanic, if you can't get your stuff back without dying, it's
        // gone forever.
        let ephemerals: Vec<Entity> = r
            .ecs
            .query::<&IsEphemeral>()
            .iter()
            .map(|(e, _)| Entity(e))
            .collect();

        for e in ephemerals {
            if e == *self {
                log::warn!("Respawning an ephemeral entity.");
                continue;
            }
            e.destroy(r);
        }

        let num_deaths = self.get::<NumDeaths>(r).0;
        if num_deaths == 0 {
            msg!("[One] [is] no longer mortal."; self.noun(r));
        }
        self.set(r, NumDeaths(num_deaths + 1));

        self.fully_heal(r);

        msg!("[One] awaken[s] in a familiar place."; self.noun(r));
        self.place(r, r.world.player_entrance());
    }
}

/// Status effects.
#[derive(
    Copy,
    Clone,
    Debug,
    Eq,
    PartialEq,
    Hash,
    Ord,
    PartialOrd,
    Serialize,
    Deserialize,
)]
pub enum Buff {
    Confusion,
}

impl Buff {
    pub fn expire_msg(&self, r: &impl AsRef<Runtime>, e: Entity) {
        let noun = e.noun(r);
        match self {
            Buff::Confusion => {
                msg!("[One] [is] no longer confused."; noun);
            }
        }
    }
}
