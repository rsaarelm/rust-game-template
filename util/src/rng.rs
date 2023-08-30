use std::{fmt, hash::Hash};
use std::{hash::Hasher, str::FromStr};

use anyhow::bail;
use derive_deref::Deref;
use rand::{distributions::Standard, prelude::*};
use rand_xorshift::XorShiftRng;
use serde::{Deserialize, Serialize};
use serde_with::{DeserializeFromStr, SerializeDisplay};

/// Construct a throwaway random number generator seeded by a noise value.
///
/// Good for short-term use in immutable contexts given a varying source of
/// noise like map position coordinates.
pub fn srng(seed: &(impl Hash + ?Sized)) -> XorShiftRng {
    let mut h = crate::FastHasher::default();
    seed.hash(&mut h);
    XorShiftRng::seed_from_u64(h.finish())
}

/// Strings that are normalized to be case, whitespace and punctuation
/// insensitive. Use as RNG seeds so that trivial transcription errors like an
/// added space can't mess up the seed.
///
/// ```
/// # use util::{Logos, srng};
/// use rand::prelude::*;
///
/// assert_ne!(
///   srng("pAss Word").gen_range(0..1000),
///   srng("password").gen_range(0..1000));
///
/// assert_eq!(
///   srng(&Logos::new("pAss Word")).gen_range(0..1000),
///   srng(&Logos::new("password")).gen_range(0..1000));
///
/// assert_ne!(
///   srng(&Logos::new("pAss Word 123")).gen_range(0..1000),
///   srng(&Logos::new("password")).gen_range(0..1000));
///
/// assert_eq!(
///   srng(&Logos::new("!@#'")).gen_range(0..1000),
///   srng(&Logos::new(" ")).gen_range(0..1000));
/// ```
#[derive(
    Clone,
    Debug,
    Default,
    Eq,
    PartialEq,
    Hash,
    Ord,
    PartialOrd,
    Deref,
    SerializeDisplay,
    DeserializeFromStr,
)]
pub struct Logos(String);

impl fmt::Display for Logos {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromIterator<char> for Logos {
    fn from_iter<T: IntoIterator<Item = char>>(iter: T) -> Self {
        const MAX_LEN: usize = 64;

        Logos(
            iter.into_iter()
                .map(|c| c.to_ascii_uppercase())
                .filter(char::is_ascii_alphanumeric)
                .take(MAX_LEN)
                .collect(),
        )
    }
}

impl Logos {
    /// Construct a new logos, stripping out punctuation, whitespace,
    /// character case and non-ASCII characters from the input.
    pub fn new(s: impl AsRef<str>) -> Self {
        s.as_ref().chars().collect()
    }

    /// Generate a random logos of `len` characters.
    pub fn sample<R: Rng + ?Sized>(rng: &mut R, len: usize) -> Logos {
        (0..len)
            .map(|_| {
                *b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ".choose(rng).unwrap()
                    as char
            })
            .collect()
    }
}

impl FromStr for Logos {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if !s
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
        {
            bail!("not a valid logos")
        } else {
            Ok(Logos(s.into()))
        }
    }
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

impl Distribution<Odds> for Standard {
    fn sample<R: rand::Rng + ?Sized>(&self, rng: &mut R) -> Odds {
        Odds::from_prob(rng.gen())
    }
}

impl Distribution<bool> for Odds {
    fn sample<R: rand::Rng + ?Sized>(&self, rng: &mut R) -> bool {
        rng.gen_range(0.0..1.0) < self.prob()
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
        self.gen_range(0..n) == 0
    }
}
