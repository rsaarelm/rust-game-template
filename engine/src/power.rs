//! Special powers entities can use

use content::{Power, Rect, Zone};
use serde::{Deserialize, Serialize};
use util::{v2, Neighbors2D};

use crate::{
    ecs::{Powers, Wounds},
    prelude::*,
    FOV_RADIUS,
};

impl Runtime {
    pub fn invoke_power(
        &mut self,
        power: Power,
        perp: Option<Entity>,
        loc: &Location,
        v: IVec2,
    ) {
        use Power::*;
        match power {
            CallLightning => self.lightning(perp, loc),
            Confusion => self.confusion(perp, loc, v),
            Fireball => self.fireball(perp, loc, v),
            MagicMapping => self.magic_map(perp, loc),
            HealSelf => self.heal(perp, loc),
        }
    }

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
        from: &Location,
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
            let loc = loc.snap_above_floor(self);

            // Hit a wall, pull back one tile.
            if loc.blocks_shot(self) {
                return loc - dir.extend(0);
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

    pub fn trace_enemy(
        &self,
        perp: Option<Entity>,
        from: &Location,
        dir: IVec2,
        range: usize,
    ) -> Option<Entity> {
        if let Some(mob) =
            self.trace_target(perp, from, dir, range).mob_at(self)
        {
            if let Some(perp) = perp {
                if mob.is_enemy(self, &perp) {
                    return Some(mob);
                }
            } else {
                return Some(mob);
            }
        }
        None
    }

    fn confusion(&mut self, perp: Option<Entity>, from: &Location, dir: IVec2) {
        const CONFUSION_RANGE: usize = 12;
        if let Some(target) = self.trace_enemy(perp, from, dir, CONFUSION_RANGE)
        {
            target.confuse(self);
        }
    }

    fn fireball(&mut self, perp: Option<Entity>, from: &Location, dir: IVec2) {
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
            (target + v2(p).extend(0)).damage(self, perp, FIREBALL_DAMAGE);
        }
    }

    fn heal(&mut self, perp: Option<Entity>, _from: &Location) {
        const HEAL_AMOUNT: i32 = 8;
        if let Some(e) = perp {
            let is_hurt = e.get::<Wounds>(self).0 != 0;
            let wounds = Wounds((e.get::<Wounds>(self).0 - HEAL_AMOUNT).max(0));
            e.set(self, wounds);
            if is_hurt {
                msg!("[One] [is] healed."; e.noun(self));
            }
        }
    }

    fn lightning(&mut self, perp: Option<Entity>, from: &Location) {
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

    fn magic_map(&mut self, _perp: Option<Entity>, from: &Location) {
        const MAGIC_MAP_RANGE: usize = 100;

        let zone = from.sector().fat();

        let mut revealed: Vec<(Location, usize)> = util::dijkstra_map(
            |loc| {
                loc.hover_neighbors(self)
                    .map(|(_, loc)| loc)
                    .filter(|&loc| zone.contains(loc))
            },
            [*from],
        )
        .filter(|(loc, d)| !loc.is_explored(self) && *d < MAGIC_MAP_RANGE)
        .collect();

        // Hack to add walls to the cover.
        let rim: Vec<(Location, usize)> = revealed
            .iter()
            .flat_map(|(loc, n)| {
                loc.ns_8().map(|loc| (loc.snap_above_floor(self), *n + 1))
            })
            .filter(|(loc, _)| {
                !loc.is_explored(self)
                    && !revealed.iter().any(|(loc_2, _)| loc == loc_2)
            })
            .collect();

        // Reveal the terrain.
        for (loc, _) in &revealed {
            self.fov.insert(*loc);
        }

        for (loc, n) in rim {
            if loc.is_explored(self) {
                revealed.push((loc, n));
            }
        }

        // Finally flatten everything to the base z level so all animations
        // are visible at the same layer and revealed bumps don't show up
        // early.
        for (loc, _) in revealed.iter_mut() {
            loc.z = from.z;
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
    pub fn has_powers(&self, r: &impl AsRef<Runtime>) -> bool {
        self.with::<Powers, _>(r, |a| !a.0.is_empty())
    }

    pub fn powers(&self, r: &impl AsRef<Runtime>) -> Vec<Power> {
        self.with::<Powers, _>(r, |ab| ab.0.keys().copied().collect())
    }

    pub(crate) fn cast(
        &self,
        r: &mut impl AsMut<Runtime>,
        power: Power,
        v: IVec2,
    ) {
        let r = r.as_mut();
        let Some(loc) = self.loc(r) else { return };
        r.invoke_power(power, Some(*self), &loc, v);
        self.complete_turn(r);
    }
}
