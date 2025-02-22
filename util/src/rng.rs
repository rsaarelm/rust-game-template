use std::{
    fmt,
    hash::{Hash, Hasher},
};

use rand::{distr::StandardUniform, prelude::*};
use serde::{Deserialize, Serialize};

use crate::GameRng;

/// Construct a throwaway random number generator seeded by a noise value.
///
/// Good for short-term use in immutable contexts given a varying source of
/// noise like map position coordinates.
pub fn srng(seed: &(impl Hash + ?Sized)) -> GameRng {
    // NB. This hash function used here must work the same on all platforms.
    // Do not use fxhash hasher.
    let mut h = twox_hash::XxHash64::default();
    seed.hash(&mut h);
    GameRng::seed_from_u64(h.finish())
}

/// Deciban log-odds type.
///
/// Expresses a probability, but in a form that is easier to reason about and
/// calculate with in some conditions.
///
/// Inner value is integer because video games, YAGNI more precision.
///
/// See <https://en.wikipedia.org/wiki/Hartley_(unit)>
#[derive(Copy, Clone, PartialEq, PartialOrd, Debug, Serialize, Deserialize)]
pub struct Odds(pub i32);

impl Odds {
    pub fn from_prob(p: f32) -> Odds {
        debug_assert!((0.0..=1.0).contains(&p));

        Odds((10.0f32 * (p / (1.0 - p)).log(10.0)).round() as i32)
    }

    pub fn prob(self) -> f32 {
        1.0 - 1.0 / (1.0 + 10.0f32.powf(self.0 as f32 / 10.0))
    }
}

impl Distribution<Odds> for StandardUniform {
    fn sample<R: rand::Rng + ?Sized>(&self, rng: &mut R) -> Odds {
        Odds::from_prob(rng.random())
    }
}

impl Distribution<bool> for Odds {
    fn sample<R: rand::Rng + ?Sized>(&self, rng: &mut R) -> bool {
        rng.random_range(0.0..1.0) < self.prob()
    }
}

impl fmt::Display for Odds {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::ops::Add for Odds {
    type Output = Odds;

    fn add(self, rhs: Self) -> Self::Output {
        Odds(self.0 + rhs.0)
    }
}

impl std::ops::Sub for Odds {
    type Output = Odds;

    fn sub(self, rhs: Self) -> Self::Output {
        Odds(self.0 - rhs.0)
    }
}

pub trait RngExt {
    fn one_chance_in(&mut self, n: usize) -> bool;
}

impl<T: Rng + ?Sized> RngExt for T {
    fn one_chance_in(&mut self, n: usize) -> bool {
        if n == 0 {
            return false;
        }
        self.random_range(0..n) == 0
    }
}
