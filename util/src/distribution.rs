use std::{fmt, ops::RangeInclusive};

use nom::{
    branch::alt,
    bytes::complete::tag,
    character::complete::digit1,
    combinator::{all_consuming, map_res, opt, recognize},
    error::Error,
    sequence::{pair, preceded},
    Finish, IResult, Parser,
};
use rand::Rng;
use serde_with::{DeserializeFromStr, SerializeDisplay};

/// Distributions defined by a domain of [0.0, 1.0]. A lot like general random
/// distributions, but you can plot the contents on a graph.
pub trait PlottedDistribution {
    type Item;

    /// Sample the distribution using a value between 0 and 1.
    ///
    /// This maps to the whole probability space of the distribution.
    fn plot(&self, x: f32) -> Self::Item;

    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Self::Item {
        self.plot(rng.random::<f32>())
    }
}

/// A stepped integer range that can be parsed and used as a distribution.
#[derive(
    Copy, Clone, Debug, Eq, PartialEq, DeserializeFromStr, SerializeDisplay,
)]
pub struct RangeDistribution {
    min: i32,
    step: i32,
    max: i32,
}

impl RangeDistribution {
    pub fn new(min: i32, max: i32) -> Self {
        Self { min, step: 1, max }
    }

    pub fn with_step(mut self, step: i32) -> Self {
        assert!(step > 0);
        self.step = step;
        self
    }
}

impl Iterator for RangeDistribution {
    type Item = i32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.min > self.max {
            return None;
        }

        let current = self.min;
        self.min += self.step;
        Some(current)
    }
}

impl PlottedDistribution for RangeDistribution {
    type Item = i32;

    fn plot(&self, x: f32) -> i32 {
        let range = (self.max - self.min) / self.step;
        if range <= 0 {
            return self.min;
        }

        // Plot the parameter in the range of positions and floor down to
        // integer.
        let pos = (x * range as f32) as i32;
        self.min + pos * self.step
    }
}

impl From<RangeInclusive<i32>> for RangeDistribution {
    fn from(range: RangeInclusive<i32>) -> Self {
        Self::new(*range.start(), *range.end())
    }
}

impl fmt::Display for RangeDistribution {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.step == 1 {
            write!(f, "{}..{}", self.min, self.max)
        } else {
            write!(f, "{},{}..{}", self.min, self.min + self.step, self.max)
        }
    }
}

impl std::str::FromStr for RangeDistribution {
    type Err = Error<String>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        fn number(s: &str) -> IResult<&str, i32> {
            map_res(recognize(preceded(opt(tag("-")), digit1)), str::parse)
                .parse(s)
        }

        let pair = |s| {
            map_res(pair(number, preceded(tag(","), number)), |(min, next)| {
                if next <= min {
                    return Err(nom::Err::Error("invalid step"));
                }
                let step = next - min;
                let max = next;
                Ok::<RangeDistribution, nom::Err<&str>>(RangeDistribution {
                    min,
                    step,
                    max,
                })
            })
            .parse(s)
        };

        let range =
            |s| {
                map_res(
                    (
                        number,
                        opt(preceded(tag(","), number)),
                        preceded(tag(".."), number),
                    ),
                    |(min, next, max)| {
                        let next = next.unwrap_or(min + 1);
                        if next <= min {
                            return Err(nom::Err::Error("invalid step"));
                        }
                        let step = next - min;
                        Ok::<RangeDistribution, nom::Err<&str>>(
                            RangeDistribution { min, step, max },
                        )
                    },
                )
                .parse(s)
            };

        Ok(all_consuming(alt((range, pair)))
            .parse(s)
            .finish()
            .map_err(|e: Error<&str>| Error::new(e.input.to_string(), e.code))?
            .1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_distribution() {
        assert_eq!(RangeDistribution::new(2, 3).to_string(), "2..3");
        assert_eq!(
            RangeDistribution::new(2, 10).with_step(2).to_string(),
            "2,4..10"
        );
        assert_eq!(RangeDistribution::new(-5, -2).to_string(), "-5..-2");

        assert_eq!("2..5".parse(), Ok(RangeDistribution::new(2, 5)));
        assert_eq!(
            "2,5".parse(),
            Ok(RangeDistribution::new(2, 5).with_step(3))
        );
        assert_eq!("2,3..5".parse(), Ok(RangeDistribution::new(2, 5)));
        assert_eq!(
            "2,4..10".parse(),
            Ok(RangeDistribution::new(2, 10).with_step(2))
        );

        assert_eq!(
            "2..5"
                .parse::<RangeDistribution>()
                .unwrap()
                .collect::<Vec<_>>(),
            vec![2, 3, 4, 5]
        );
        assert_eq!(
            "2,4..10"
                .parse::<RangeDistribution>()
                .unwrap()
                .collect::<Vec<_>>(),
            vec![2, 4, 6, 8, 10]
        );

        assert_eq!("-9..-5".parse(), Ok(RangeDistribution::new(-9, -5)));
        assert_eq!("-5..5".parse(), Ok(RangeDistribution::new(-5, 5)));
        assert_eq!(
            "-5,-3..5".parse(),
            Ok(RangeDistribution::new(-5, 5).with_step(2))
        );

        assert!("1..5trash".parse::<RangeDistribution>().is_err());
        assert!("foo..bar".parse::<RangeDistribution>().is_err());
        assert!("1,-1..2".parse::<RangeDistribution>().is_err());
        assert!("1,1..2".parse::<RangeDistribution>().is_err());
        assert!("..".parse::<RangeDistribution>().is_err());
    }
}
