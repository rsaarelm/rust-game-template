//! Special powers entities can use

use serde::{Deserialize, Serialize};
use util::{s8, v2};

use crate::{ecs::Powers, prelude::*, Rect, FOV_RADIUS};

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
            CallLightning => r.lightning(perp, loc),
            Confusion => msg!("TODO!"),
            Fireball => r.fireball(perp, loc, v),
            MagicMapping => r.magic_map(perp, loc),
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
                return loc - dir;
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

        if let Some(perp) = perp {
            send_msg(Msg::Fire(perp, dir));
        }
        send_msg(Msg::Explosion(target));

        // No need to worry about it going through walls since it only extends
        // one cell in any direction from the valid starting cell.
        for p in Rect::new([-1, -1], [2, 2]) {
            (target + v2(p)).damage(self, perp, FIREBALL_DAMAGE);
        }
    }

    fn lightning(&mut self, perp: Option<Entity>, from: Location) {
        const LIGHTNING_DAMAGE: i32 = 14;

        let targets: Vec<_> = self
            .fov_from(from, FOV_RADIUS)
            .filter_map(|(_, loc)| loc.mob_at(self))
            .collect();

        // Target enemies of caster, or any mob if there is no caster.
        let Some(target) = targets
            .into_iter()
            .find(|e| perp.map_or(true, |perp| e.is_enemy(self, &perp)))
        else {
            msg!("You hear distant thunder.");
            return;
        };

        msg!("There is a peal of thunder.");
        if let Some(loc) = target.loc(self) {
            send_msg(Msg::LightningBolt(loc));
        }
        target.damage(self, perp, LIGHTNING_DAMAGE);
    }

    fn magic_map(&mut self, _perp: Option<Entity>, from: Location) {
        const MAGIC_MAP_RANGE: usize = 100;

        let neighbors = |loc: &Location| {
            // Location that stops FOV, do not proceed.
            let mut ret = Vec::new();

            if !loc.is_walkable(self) && loc.tile(self).blocks_sight() {
                return ret;
            }
            // Stop at sector edge.
            if loc.sector() != from.sector() {
                return ret;
            }

            for d in s8::DIR {
                let loc = *loc + d;
                // Only add corners if they block further FOV, this is so that
                // corners of rectangular rooms get added.
                if d.taxi_len() == 2 && !loc.tile(self).blocks_sight() {
                    continue;
                }

                ret.push(loc);
            }
            ret
        };

        let revealed: Vec<(Location, usize)> =
            util::dijkstra_map(neighbors, [from])
                .filter(|(loc, d)| {
                    !loc.is_explored(self) && *d < MAGIC_MAP_RANGE
                })
                .collect();

        // Reveal the terrain.
        for (loc, _) in &revealed {
            self.fov.insert(*loc);
        }

        send_msg(Msg::MagicMap(revealed));
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
