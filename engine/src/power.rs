//! Special powers entities can use

use serde::{Deserialize, Serialize};
use util::v2;

use crate::{ecs::Powers, prelude::*, Rect};

#[derive(
    Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum Power {
    BerserkRage,
    CallLightning,
    Confusion,
    Fireball,
    MagicMapping,
}

use Power::*;

impl Power {
    pub fn needs_aim(self) -> bool {
        matches!(self, Fireball)
    }

    pub fn invoke(
        self,
        r: &mut Runtime,
        perp: Option<Entity>,
        loc: Location,
        v: IVec2,
    ) {
        match self {
            BerserkRage => msg!("TODO!"),
            CallLightning => msg!("TODO!"),
            Confusion => msg!("TODO!"),
            Fireball => r.fireball(perp, loc, v),
            MagicMapping => msg!("TODO!"),
        }
    }
}

impl Runtime {
    /// Raycast for a hittable target given a starting position, a direction
    /// and a maximum range.
    ///
    /// If the perpetrator is given, allies will be passed through.
    ///
    /// NB. It is not guaranteed that there won't be friendlies at the
    /// resulting location. The trace will always end at the end of the given
    /// range.
    pub fn trace_target(
        &self,
        perp: Option<Entity>,
        from: Location,
        dir: IVec2,
        range: usize,
    ) -> Location {
        let friend = |loc: Location| {
            if let (Some(perp), Some(target)) = (perp, loc.mob_at(self)) {
                target.is_ally(self, &perp)
            } else {
                false
            }
        };

        for (i, loc) in from.trace(dir).enumerate() {
            // Hit a wall, pull back one tile.
            if loc.tile(self).blocks_shot() {
                // Unless there's a friendly in which case you hit the wall.
                if friend(loc - dir) {
                    return loc;
                } else {
                    return loc - dir;
                }
            }

            // Stop at range limit.
            if i + 1 >= range {
                return loc;
            }

            // Non-friend found, stop here.
            if loc.mob_at(self).is_some() && !friend(loc) {
                return loc;
            }
        }
        unreachable!()
    }

    fn fireball(&mut self, perp: Option<Entity>, from: Location, dir: IVec2) {
        const FIREBALL_RANGE: usize = 12;
        const FIREBALL_DAMAGE: i32 = 10;
        let target = self.trace_target(perp, from, dir, FIREBALL_RANGE);

        // No need to worry about it going through walls since it only extends
        // one cell in any direction from the valid starting cell.
        for p in Rect::new([-1, -1], [2, 2]) {
            (target + v2(p)).damage(self, perp, FIREBALL_DAMAGE);
        }

        if let Some(perp) = perp {
            send_msg(Msg::Fire(perp, dir));
        }
        send_msg(Msg::Explosion(target));
    }
}

#[derive(
    Copy, Clone, Default, Debug, Eq, PartialEq, Serialize, Deserialize,
)]
#[serde(default, rename_all = "kebab-case")]
pub struct PowerState {
    cooldown_until: Instant,
}

impl Entity {
    pub fn has_powers(&self, r: &Runtime) -> bool {
        self.with::<Powers, _>(r, |a| !a.0.is_empty())
    }

    pub fn powers(&self, r: &Runtime) -> Vec<Power> {
        self.with::<Powers, _>(r, |ab| ab.0.keys().copied().collect())
    }

    pub(crate) fn cast(&self, r: &mut Runtime, power: Power, v: IVec2) {
        let Some(loc) = self.loc(r) else { return };
        power.invoke(r, Some(*self), loc, v);
        self.complete_turn(r);
    }
}
