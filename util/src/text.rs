use std::{iter, sync::LazyLock};

use regex::Regex;

/// Split text at whitespace so it fits within `max_width`.
///
/// Words that are longer than `max_width` will be sliced into `max_width`
/// sized segments.
fn split_fitting(max_width: usize, text: &str) -> (&str, &str) {
    // Can't consume anything if width is zero.
    debug_assert!(max_width > 0);

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

    // Cut off the white space in between split lines.
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
    }
}
