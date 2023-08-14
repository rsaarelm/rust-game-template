use crate::Core;

/// An opaque representation of a time instant.
///
/// The unit of time is a tick that's 1/60th of a second.
#[derive(Copy, Clone, Default, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct Instant(pub(crate) i64);

impl Instant {
    /// Return number of ticks elapsed since this instant.
    pub fn elapsed(&self, c: &Core) -> i64 {
        c.now() - *self
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
