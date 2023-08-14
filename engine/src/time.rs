use serde::{Deserialize, Serialize};

use crate::{Core, PHASES_IN_TURN};

/// An opaque representation of a time instant.
///
/// The unit of time is a tick that's 1/60th of a second.
#[derive(
    Copy,
    Clone,
    Default,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
    Debug,
    Serialize,
    Deserialize,
)]
pub struct Instant(pub(crate) i64);

impl Instant {
    /// Return number of ticks elapsed since this instant.
    pub fn elapsed(&self, c: &Core) -> i64 {
        c.now() - *self
    }

    /// Return whether mobs with the given speed get to act on this time
    /// point.
    pub const fn is_action_frame(self, speed: u8) -> bool {
        let speed = speed as i64;
        if speed == 0 {
            false
        } else {
            let phase = self.0.rem_euclid(PHASES_IN_TURN);
            phase * speed / PHASES_IN_TURN
                != (phase + 1) * speed / PHASES_IN_TURN
        }
    }
}

impl std::ops::Add<i64> for Instant {
    type Output = Self;

    fn add(self, rhs: i64) -> Self::Output {
        Instant(self.0 + rhs)
    }
}

impl std::ops::AddAssign<i64> for Instant {
    fn add_assign(&mut self, rhs: i64) {
        self.0 += rhs;
    }
}

impl std::ops::Sub<Instant> for Instant {
    type Output = i64;

    fn sub(self, rhs: Instant) -> Self::Output {
        self.0 - rhs.0
    }
}

impl std::ops::Sub<i64> for Instant {
    type Output = Self;

    fn sub(self, rhs: i64) -> Self::Output {
        Instant(self.0 - rhs)
    }
}

impl std::ops::SubAssign<i64> for Instant {
    fn sub_assign(&mut self, rhs: i64) {
        self.0 -= rhs;
    }
}
