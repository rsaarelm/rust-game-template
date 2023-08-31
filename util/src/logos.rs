use std::{fmt, hash::Hash, str::FromStr};

use anyhow::bail;
use derive_deref::Deref;
use itertools::Itertools;
use rand::prelude::*;
use serde_with::{DeserializeFromStr, SerializeDisplay};

pub const ALPHABET: &str = "0123456789TANHRDLUCMFWYPVBGKJQXZ";

/// Strings that are normalized to be case, whitespace and punctuation
/// insensitive. Use as RNG seeds so that trivial transcription errors like an
/// added space can't mess up the seed.
///
/// Logos strings can be spoken out loud unambigously using the NATO phonetic
/// alphabet and can be used as a binary serialization format. Letters 'I',
/// 'S' and 'O' are removed to prevent ambiguity with '1', '5' and '0' and 'E'
/// is removed to bring the total alphabet down to 32 characters for better
/// data encoding. Use L33T5P3AK to get around the missing characters.
///
/// A Logos string corresponds to a little-endian binary number with each
/// letter corresponding to one sequence of five bits and letter `0`
/// corresponding to `0b00000`. Logos strings with matching prefixes and any
/// length of `0` as suffix are numerically equal. If treated as a byte
/// sequence, the string is considered to specify bytes as far as it encodes
/// bytes that either has at least one bit set or all bits covered by the
/// encoding. String `"00"` covers 10 bits so it amounts to `[0u8]` (8 bits
/// covered, but not 16). String `"0000"` covers 20 bits, so it amounts to
/// `[0u8, 0u8]` (16 bits covered, but not 24).
///
/// ```
/// # use util::{Logos, srng};
/// use rand::prelude::*;
///
/// assert_ne!(
///   srng("pass word").gen_range(0..1000),
///   srng("password").gen_range(0..1000));
///
/// assert_eq!(
///   srng(&Logos::new("pAss Word")).gen_range(0..1000),
///   srng(&Logos::new("password")).gen_range(0..1000));
///
/// // Trailing zeroes are ignored for hashing.
/// assert_eq!(
///   srng(&Logos::new("xyzzy")).gen_range(0..1000),
///   srng(&Logos::new("xyzzy000")).gen_range(0..1000));
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
        Logos(
            iter.into_iter()
                .map(|c| c.to_ascii_uppercase())
                .filter(|&c| ALPHABET.contains(c))
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

    /// Construct a logos string, substituting missing letters with numbers.
    pub fn elite_new(s: impl AsRef<str>) -> Self {
        s.as_ref()
            .chars()
            .map(|c| match c.to_ascii_uppercase() {
                'E' => '3',
                'I' => '1',
                'O' => '0',
                'S' => '5',
                a => a,
            })
            .collect()
    }

    pub fn from_bytes(data: &[u8]) -> Self {
        if data.is_empty() {
            return Default::default();
        }

        fn bit(data: &[u8], pos: usize) -> usize {
            if pos / 8 < data.len() {
                (data[pos / 8] & (1 << (pos % 8)) != 0) as usize
            } else {
                0
            }
        }

        let n_chars = (data.len() * 8 + 4) / 5;
        let last_byte = data[data.len() - 1];

        let mut ret = String::new();
        for i in 0..n_chars {
            let c: usize =
                (0..5).map(|j| bit(data, i * 5 + j) << j).sum::<usize>();

            // Don't push the last zero chars if the last byte can be inferred
            // from bits already written.
            let remaining_bits = data.len() * 8 - i * 5;
            if remaining_bits < 8
                && last_byte != 0
                && last_byte >> (8 - remaining_bits) == 0
            {
                break;
            }

            if i == n_chars - 1 && c == 0 && data[data.len() - 1] != 0 {
                break;
            }

            ret.push(ALPHABET.as_bytes()[c] as char);
        }

        Logos(ret)
    }

    /// Generate a random logos of `len` characters.
    pub fn sample<R: Rng + ?Sized>(rng: &mut R, len: usize) -> Logos {
        (0..len)
            .map(|_| *ALPHABET.as_bytes().choose(rng).unwrap() as char)
            .collect()
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut ret = Vec::new();

        for chunk in &self
            .0
            .chars()
            .map(|c| ALPHABET.find(c).expect("invalid logos") as u8)
            .flat_map(|b| (0..5).map(move |i| (b >> i) & 1))
            .chunks(8)
        {
            let vec: Vec<u8> = chunk.collect();
            // End is all zero bits and does not fill a full byte, drop out
            // early.
            if vec.len() < 8 && vec.iter().all(|&e| e == 0) {
                break;
            } else {
                ret.push(
                    vec.into_iter()
                        .enumerate()
                        .fold(0, |a, (i, b)| a + (b << i)),
                );
            }
        }

        ret
    }

    /// Return the value prefix of the logos that omits trailing zeroes.
    ///
    /// Logoi are assumed to have the same value regardless of trailing
    /// zeroes, analogous to how 00360 and 360 denote the same number.
    pub fn value(&self) -> &str {
        let suffix_len = (0..self.0.len())
            .rev()
            .take_while(|&i| self.0.as_bytes()[i] == b'0')
            .count();
        let n = self.0.len() - suffix_len;
        if n == 0 {
            // No content in string, return single zero from alphabet.
            &ALPHABET[0..1]
        } else {
            &self.0[0..n]
        }
    }
}

// Hashing uses the true value and discounts the trailing zeroes.

impl Hash for Logos {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.value().hash(state);
    }
}

// XXX: Is there a more compact way to do numeric conversions?

impl From<u64> for Logos {
    fn from(value: u64) -> Self {
        let bytes = value.to_le_bytes();
        let mut buf = &bytes[..];
        while !buf.is_empty() && buf[buf.len() - 1] == 0 {
            buf = &buf[0..buf.len() - 1];
        }
        Logos::from_bytes(buf)
    }
}

impl From<&Logos> for u64 {
    fn from(value: &Logos) -> Self {
        let mut bytes = [0; 8];
        for (i, b) in value.to_bytes().into_iter().enumerate().take(8) {
            bytes[i] = b;
        }
        u64::from_le_bytes(bytes)
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

#[cfg(test)]
mod test {
    use super::*;
    use quickcheck::{Arbitrary, Gen};
    use quickcheck_macros::quickcheck;

    fn m(log: &str, bytes: &[u8]) {
        let logos = Logos::new(log);
        assert_eq!(logos.0, log);
        assert_eq!(logos, Logos::from_bytes(bytes));
        assert_eq!(bytes, logos.to_bytes());
    }

    #[test]
    fn matches() {
        m("", &[]);
        m("00", &[0]);
        m("0000", &[0, 0]);
        m("1000", &[1, 0]);
        m("08", &[0, 1]);
        m("00000", &[0, 0, 0]);
        m("0000000", &[0, 0, 0, 0]);
        m("0000L", &[0, 0, 0, 1]);
        m("00000000", &[0, 0, 0, 0, 0]);
        m("ZZZZZZZZ", &[0xff, 0xff, 0xff, 0xff, 0xff]);
    }

    #[test]
    fn value() {
        assert_eq!(Logos::new("").value(), "0");
        assert_eq!(Logos::new("0").value(), "0");
        assert_eq!(Logos::new("1").value(), "1");
        assert_eq!(Logos::new("0000").value(), "0");
        assert_eq!(Logos::new("123000").value(), "123");
        assert_eq!(Logos::new("000001").value(), "000001");
    }

    impl Arbitrary for Logos {
        fn arbitrary(g: &mut Gen) -> Logos {
            let size = { usize::arbitrary(g) % 40 };
            Logos(
                (0..size)
                    .map(|_| *g.choose(ALPHABET.as_bytes()).unwrap() as char)
                    .collect(),
            )
        }

        fn shrink(&self) -> Box<dyn Iterator<Item = Self>> {
            let mut elt = self.clone();
            Box::new(std::iter::from_fn(move || {
                if elt.0.pop().is_some() {
                    Some(elt.clone())
                } else {
                    None
                }
            }))
        }
    }

    #[quickcheck]
    fn bytes_to_logos(bytes: Vec<u8>) -> bool {
        let logos = Logos::from_bytes(&bytes);
        logos.to_bytes() == bytes
    }

    #[quickcheck]
    fn logos_to_bytes(logos: Logos) -> bool {
        let bytes = logos.to_bytes();
        let roundtrip = Logos::from_bytes(&bytes);

        // All logoi don't survive the roundtrip intact wrt. their 0-suffixes.
        // Their values must match though.
        logos.value() == roundtrip.value()
    }
}
