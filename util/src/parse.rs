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

pub fn multiple(input: &str) -> Result<i32> {
    let (word, rest) = word(input)?;
    let Some(num) = word.strip_suffix("x") else {
        return Err(input);
    };
    let Ok(num) = num.parse::<i32>() else {
        return Err(input);
    };

    if num > 0 {
        Ok((num, rest))
    } else {
        Err(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multiple() {
        assert_eq!(multiple("13x squid"), Ok((13, "squid")));
        assert_eq!(multiple("13yx squid"), Err("13yx squid"));
        assert_eq!(multiple("13 squid"), Err("13 squid"));
        assert_eq!(multiple("0x squid"), Err("0x squid"));
    }
}
