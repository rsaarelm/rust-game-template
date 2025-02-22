pub type Result<'a, T> = std::result::Result<(T, &'a str), &'a str>;

pub fn word(input: &str) -> Result<&str> {
    let end_pos = input
        .char_indices()
        .find_map(|(i, c)| c.is_whitespace().then_some(i))
        .unwrap_or(input.len());

    let next_pos = input[end_pos..]
        .char_indices()
        .find_map(|(i, c)| (!c.is_whitespace()).then_some(end_pos + i))
        .unwrap_or(input.len());

    debug_assert!(next_pos >= end_pos);

    if end_pos > 0 {
        Ok((&input[0..end_pos], &input[next_pos..]))
    } else {
        Err(input)
    }
}

pub fn multiples(input: &str) -> Result<i32> {
    let (word, rest) = word(input)?;
    let Some(num) = word.strip_suffix("x") else {
        return Err(input);
    };
    let Ok(num) = num.parse::<i32>() else {
        return Err(input);
    };

    if num > 0 { Ok((num, rest)) } else { Err(input) }
}

pub fn multipliable(input: &str) -> (i32, &str) {
    if let Ok((count, rest)) = multiples(input) {
        (count, rest)
    } else {
        (1, input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multiple() {
        assert_eq!(multipliable("13x squid"), (13, "squid"));
        assert_eq!(multipliable("1x squid"), (1, "squid"));
        assert_eq!(multipliable("squid"), (1, "squid"));
        // Deformed multipliers just go in the main word for now.
        // We might want these to be errors but let's keep things stupid and
        // simple for the time being.
        assert_eq!(multipliable("13yx squid"), (1, "13yx squid"));
        assert_eq!(multipliable("13 squid"), (1, "13 squid"));
        assert_eq!(multipliable("0x squid"), (1, "0x squid"));
    }
}
