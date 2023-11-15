use std::{iter, sync::LazyLock};

use glam::{ivec2, IVec2};
use regex::Regex;

/// Split text at whitespace so it fits within `max_width`.
///
/// Words that are longer than `max_width` will be sliced into `max_width`
/// sized segments.
fn split_fitting(max_width: usize, text: &str) -> (&str, &str) {
    // Can't consume anything if width is zero.
    assert!(max_width > 0);

    // Text fits in a single line and has no newlines, return as is.
    if text.chars().count() <= max_width && !text.chars().any(|c| c == '\n') {
        return (text, "");
    }

    // Position of end of text that fits in split-off line.
    let mut line_end = None;

    // Set to true in case line starts with whitespace
    let mut traversing_whitespace = true;
    for (i, (pos, c)) in text.char_indices().enumerate() {
        // Always break when you see newline.
        if c == '\n' {
            line_end = Some(pos);
            break;
        }

        if i >= max_width && !c.is_whitespace() {
            if line_end.is_none() {
                // We hit max width but have no candidate prefix.
                // No choice but to cut the string mid-word.
                line_end = Some(pos);
            }
            break;
        }

        if i > 0 && c.is_whitespace() && !traversing_whitespace {
            // Mark the point where we first enter whitespace. (Use
            // traversing_whitespace flag to not update line_end at subsequent
            // whitespace chars.)
            line_end = Some(pos);
            traversing_whitespace = true;
        }
        if !c.is_whitespace() {
            traversing_whitespace = false;
        }
    }

    let line_end = match line_end {
        None => text.len(),
        Some(n) => n,
    };

    // Cut off the whitespace in between split lines.
    // Start with the assumption that the whole remaining string is
    // whitespace, truncate in the loop.
    let mut whitespace_span = text[line_end..].len();
    for (i, c) in text[line_end..].char_indices() {
        // Stop cutting right past first newline you see.
        if c == '\n' {
            whitespace_span = i + 1;
            break;
        }
        // Otherwise cut when you see non-whitespace again
        if !c.is_whitespace() {
            whitespace_span = i;
            break;
        }
    }

    (&text[..line_end], &text[(line_end + whitespace_span)..])
}

pub fn split(max_width: usize, mut text: &str) -> impl Iterator<Item = &str> {
    iter::from_fn(move || {
        if text.is_empty() {
            None
        } else {
            let (line, rest) = split_fitting(max_width, text);
            text = rest;
            Some(line)
        }
    })
}

/// Pack repeating message lines into a single message with a multiplier
/// count.
///
/// If the function returns a value, the previous message in the message queue
/// is intended to be replaced with the value string and the redundant new
/// message discarded. Otherwise the new message is appended to the queue.
///
/// ```
/// # use util::text::deduplicate_message;
/// assert_eq!(deduplicate_message("Bump.", "Jump."), None);
///
/// assert_eq!(deduplicate_message("Bump.", "Bump."),
///     Some("Bump. (x2)".to_string()));
/// assert_eq!(deduplicate_message("Bump. (x2)", "Bump."),
///     Some("Bump. (x3)".to_string()));
///
/// // Refuse to parse stupidly large numbers.
/// assert_eq!(deduplicate_message("Bump. (x131236197263917263)", "Bump."),
///     None);
/// ```
pub fn deduplicate_message(prev: &str, current: &str) -> Option<String> {
    static RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^(.*) \(x(\d{1,8})\)$").unwrap());

    let mut base = prev;
    let mut count: usize = 1;

    if let Some(caps) = RE.captures(prev) {
        base = caps.get(1).expect("Invalid regex").as_str();
        count = caps
            .get(2)
            .expect("Invalid regex")
            .as_str()
            .parse()
            .expect("Invalid regex");
    }

    if base == current {
        Some(format!("{current} (x{})", count + 1))
    } else {
        None
    }
}

/// Create formatted help strings given a keyboard shortcut and a command name
/// that try to embed the shortcut in the name as a mnemonic.
///
/// ```
/// # use util::text::input_help_string;
/// assert_eq!(input_help_string("z", "stats"), "z) stats");
/// assert_eq!(input_help_string("i", "inventory"), "i)nventory");
/// assert_eq!(input_help_string("V", "travel"), "tra(V)el");
/// assert_eq!(input_help_string("z", "xyzzy"), "xy(z)zy");
/// ```
pub fn input_help_string(key: &str, command: &str) -> String {
    // XXX: Assumes strings are ASCII-7.
    if key.len() <= 1 {
        if let Some(p) = command.to_lowercase().find(&key.to_lowercase()) {
            if p == 0 {
                return format!("{}){}", key, &command[1..]);
            } else {
                return format!(
                    "{}({}){}",
                    &command[..p],
                    key,
                    &command[(p + 1)..]
                );
            }
        }
    }
    format!("{}) {}", key, command)
}

pub fn is_vowel(c: char) -> bool {
    // If accented chars are used, they need to be added here...
    matches!(c.to_ascii_lowercase(), 'a' | 'e' | 'i' | 'o' | 'u')
}

pub fn is_capitalized(s: &str) -> bool {
    s.chars().next().map(|c| c.is_uppercase()).unwrap_or(false)
}

pub fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().chain(chars).collect(),
    }
}

pub fn uncapitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_lowercase().chain(chars).collect(),
    }
}

/// Translate segments in square brackets in string with the given function.
///
/// Square brackets can be escaped by doubling them, `[[` becomes a literal
/// `[` and `]]` becomes a literal `]`.
///
/// If your opening and closing brackets don't match, the formatting behavior
/// is unspecified.
///
/// If the template parameter starts with a capital letter, the result from
/// the converter is capitalized. The converter always gets lowercase values.
///
/// # Examples
///
/// ```
/// use util::text::templatize;
///
/// fn translate(word: &str) -> Result<String, ()> {
///     match word {
///         "foo" => Ok("bar"),
///         _ => Err(())
///     }.map(|x| x.to_string())
/// }
///
/// assert_eq!(Ok("Foo bar baz".into()), templatize(translate, "Foo [foo] baz"));
/// assert_eq!(Ok("Bar foo baz".to_string()), templatize(translate, "[Foo] foo baz"));
/// assert_eq!(Err(()), templatize(translate, "foo [bar] baz"));
/// assert_eq!(Ok("foo [foo] baz".to_string()), templatize(translate, "foo [[foo]] baz"));
/// ```
pub fn templatize<F, E>(mut mapper: F, mut text: &str) -> Result<String, E>
where
    F: FnMut(&str) -> Result<String, E>,
{
    // I'm going to do some fun corner-cutting here.
    //
    // Instead of being all proper-like with the opening and closing bracket, I'll just treat them
    // both as a generic separator char, so the string will start in verbatim mode and a lone
    // bracket in either direction will toggle modes between verbatim and templatize.

    fn next_chunk(text: &str) -> (String, &str) {
        let mut acc = String::new();
        let mut prev = '\0';
        for (i, c) in text.char_indices() {
            // Escaped bracket, emit one.
            if (c == '[' || c == ']') && prev == c {
                acc.push(c);
                prev = '\0';
                continue;
            }
            // Actual bracket, end chunk here and return remain.
            if prev == '[' || prev == ']' {
                return (acc, &text[i..]);
            }
            if c != '[' && c != ']' {
                acc.push(c);
            }
            prev = c;
        }
        (acc, &text[text.len()..])
    }

    let mut ret = String::new();
    let mut templating = false;
    while !text.is_empty() {
        let (mut chunk, remain) = next_chunk(text);
        text = remain;

        if templating {
            if is_capitalized(&chunk) {
                chunk = mapper(&uncapitalize(&chunk))?;
                chunk = capitalize(&chunk);
            } else {
                chunk = mapper(&chunk)?;
            }
        }

        ret += &chunk;
        templating = !templating;
    }
    Ok(ret)
}

/// Get the smallest common indentation depth of nonempty lines of text.
///
/// Both tabs and spaces are treated as a single unit of indentation.
pub fn indentation(text: &str) -> usize {
    text.lines()
        .filter(|a| !a.trim().is_empty())
        .map(|a| a.chars().take_while(|c| c.is_whitespace()).count())
        .min()
        .unwrap_or(0)
}

/// Return non-whitespace chars from a block of text mapped to their
/// coordinates.
///
/// The text is trimmed so that the result set will have a minimum x
/// coordinate and a minimum y coordinate at 0.
pub fn char_grid(text: &str) -> impl Iterator<Item = (IVec2, char)> + '_ {
    let x_skip = indentation(text);

    text.lines()
        .skip_while(|a| a.trim().is_empty())
        .enumerate()
        .flat_map(move |(y, line)| {
            line.chars()
                .skip(x_skip)
                .enumerate()
                .map(move |(x, c)| (ivec2(x as i32, y as i32), c))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_text() {
        assert_eq!(
            split(
                24,
                "’Twas brillig, and the slithy toves
      Did gyre and gimble in the wabe:
All mimsy were the borogoves,
      And the mome raths outgrabe."
            )
            .collect::<Vec<_>>(),
            vec![
                "’Twas brillig, and the",
                "slithy toves",
                "      Did gyre and",
                "gimble in the wabe:",
                "All mimsy were the",
                "borogoves,",
                "      And the mome raths",
                "outgrabe."
            ]
        );

        assert_eq!(
            split(28, "Really cancel the quitting of the stopping?",)
                .collect::<Vec<_>>(),
            vec!["Really cancel the quitting", "of the stopping?"]
        );
    }

    #[test]
    fn capitalizers() {
        use super::{capitalize, is_capitalized};
        for &(text, cap) in &[
            ("", ""),
            ("a", "A"),
            ("A", "A"),
            ("Abc", "Abc"),
            ("abc", "Abc"),
            ("aBC", "ABC"),
            ("ABC", "ABC"),
            ("æ", "Æ"),
            ("æìë", "Æìë"),
        ] {
            assert_eq!(&capitalize(text), cap);
            assert_eq!(
                is_capitalized(text),
                !text.is_empty() && capitalize(text) == text
            );
        }
    }
}
