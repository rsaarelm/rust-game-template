const MUNG_ALPHABET: &str =
    "-ETAOINSHRDLUCMFWYPVBGKJQXZetaoinshrdlucmfwypvbgkjqxz@0123456789";

/// Turn data into a nonsense identifier string.
///
/// ```
/// # use util::{mung, unmung};
///
/// assert_eq!(mung(0), "-");
/// assert_eq!(unmung("-"), 0);
/// assert_eq!(mung(1), "E");
/// assert_eq!(mung(4477907144996), "EEDQOROd");
/// assert_eq!(unmung("EEDQOROd"), 4477907144996);
/// assert_eq!(mung(u64::MAX), "F9999999999");
/// assert_eq!(unmung("F9999999999"), u64::MAX);
/// assert_eq!(unmung("99999999999999999"), u64::MAX);
/// ```
pub fn mung(mut data: u64) -> String {
    if data == 0 {
        return "-".to_string();
    }

    let mut ret = String::new();
    while data != 0 {
        let c = MUNG_ALPHABET.as_bytes()[data as usize % 0x40] as char;
        data >>= 6;
        ret.push(c);
    }

    ret.chars().rev().collect()
}

/// Turn a munged string back into data.
pub fn unmung(mung: impl AsRef<str>) -> u64 {
    const REVERSE_MUNG: [u64; 128] = {
        let mut ret = [0; 128];
        let mut i = 0;
        while i < 64 {
            ret[MUNG_ALPHABET.as_bytes()[i] as usize] = i as u64;
            i += 1;
        }
        ret
    };

    let mut ret = 0;
    for (i, c) in mung.as_ref().chars().rev().enumerate() {
        let c = c as usize;
        if c >= REVERSE_MUNG.len() {
            continue;
        }

        if i == 11 {
            break;
        }
        // XXX: No error handling, just emit zero on weird chars.
        let mut bits = REVERSE_MUNG[c];

        // Only 4 bits fits in u64 at 11th sextuplet.
        if i == 10 {
            bits &= 0xf;
        }

        ret |= bits << (i * 6);
    }
    ret
}
