use std::{iter, sync::LazyLock};

use glam::{ivec2, IVec2};
use regex::Regex;

use crate::HashMap;

pub trait StrExt {
    /// Convert identifiers to lowercase kebab-case. Adds hyphens between
    /// connected lowercase and uppercase characters for CamelCase
    /// identifiers.
    fn to_kebab_case(&self) -> String;

    /// Split text at whitespace so it fits within `max_width`.
    ///
    /// Words that are longer than `max_width` will be sliced into `max_width`
    /// sized segments.
    fn split_fitting(&self, max_width: usize) -> (&str, &str);

    /// Iterate over lines of text that fit within `max_width`.
    fn lines_of(&self, max_width: usize) -> impl Iterator<Item = &str>;

    /// Pack repeating message lines into a single message with a multiplier
    /// count.
    ///
    /// If the function returns a value, the previous message in the message queue
    /// is intended to be replaced with the value string and the redundant new
    /// message discarded. Otherwise the new message is appended to the queue.
    ///
    /// ```
    /// # use util::StrExt;
    /// assert_eq!("Bump.".deduplicate_message("Jump."), None);
    ///
    /// assert_eq!("Bump.".deduplicate_message("Bump."),
    ///     Some("Bump. (x2)".to_string()));
    /// assert_eq!("Bump. (x2)".deduplicate_message("Bump."),
    ///     Some("Bump. (x3)".to_string()));
    ///
    /// // Refuse to parse stupidly large numbers.
    /// assert_eq!("Bump. (x131236197263917263)".deduplicate_message("Bump."),
    ///     None);
    /// ```
    fn deduplicate_message(&self, next: &str) -> Option<String>;

    /// Create formatted help strings given a keyboard shortcut and a command name
    /// that try to embed the shortcut in the name as a mnemonic.
    ///
    /// ```
    /// # use util::StrExt;
    /// assert_eq!("z".input_help_string("stats"), "z) stats");
    /// assert_eq!("i".input_help_string("inventory"), "i)nventory");
    /// assert_eq!("V".input_help_string("travel"), "tra(V)el");
    /// assert_eq!("z".input_help_string("xyzzy"), "xy(z)zy");
    /// ```
    fn input_help_string(&self, command: &str) -> String;

    fn is_capitalized(&self) -> bool;

    fn capitalize(&self) -> String;

    fn uncapitalize(&self) -> String;

    /// Get the smallest common indentation depth of nonempty lines of text.
    ///
    /// Both tabs and spaces are treated as a single unit of indentation.
    fn indentation(&self) -> usize;

    /// Return non-whitespace chars from a block of text mapped to their
    /// coordinates.
    ///
    /// The text is trimmed so that the result set will have a minimum x
    /// coordinate and a minimum y coordinate at 0.
    fn char_grid(&self) -> impl Iterator<Item = (IVec2, char)> + '_;

    /// Try to do English noun pluralization, using the given list of irregular
    /// words.
    fn pluralize(&self, irregular_words: &HashMap<String, String>) -> String;

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
    /// use util::StrExt;
    ///
    /// fn mapping(word: &str) -> Result<String, ()> {
    ///     match word {
    ///         "foo" => Ok("bar"),
    ///         _ => Err(())
    ///     }.map(|x| x.to_string())
    /// }
    ///
    /// assert_eq!(Ok("Foo bar baz".into()), "Foo [foo] baz".templatize(mapping));
    /// assert_eq!(Ok("Bar foo baz".to_string()), "[Foo] foo baz".templatize(mapping));
    /// assert_eq!(Err(()), "foo [bar] baz".templatize(mapping));
    /// assert_eq!(Ok("foo [foo] baz".to_string()), "foo [[foo]] baz".templatize(mapping));
    /// ```
    fn templatize<F, E>(&self, mapper: F) -> Result<String, E>
    where
        F: FnMut(&str) -> Result<String, E>;
}

impl StrExt for str {
    fn to_kebab_case(&self) -> String {
        let mut result = String::with_capacity(self.len());
        let mut prev = '_';
        for c in self.chars() {
            match c {
                '_' => result.push('-'),
                c if c.is_uppercase() && prev.is_lowercase() => {
                    result.push('-');
                    result.push(c.to_ascii_lowercase());
                }
                c => result.push(c.to_ascii_lowercase()),
            }
            prev = c;
        }

        result
    }

    fn split_fitting(&self, max_width: usize) -> (&str, &str) {
        // Can't consume anything if width is zero.
        assert!(max_width > 0);

        // Text fits in a single line and has no newlines, return as is.
        if self.chars().count() <= max_width && !self.chars().any(|c| c == '\n')
        {
            return (self, "");
        }

        // Position of end of text that fits in split-off line.
        let mut line_end = None;

        // Set to true in case line starts with whitespace
        let mut traversing_whitespace = true;
        for (i, (pos, c)) in self.char_indices().enumerate() {
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
            None => self.len(),
            Some(n) => n,
        };

        // Cut off the whitespace in between split lines.
        // Start with the assumption that the whole remaining string is
        // whitespace, truncate in the loop.
        let mut whitespace_span = self[line_end..].len();
        for (i, c) in self[line_end..].char_indices() {
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

        (&self[..line_end], &self[(line_end + whitespace_span)..])
    }

    fn lines_of(&self, max_width: usize) -> impl Iterator<Item = &str> {
        let mut text = self;
        iter::from_fn(move || {
            if text.is_empty() {
                None
            } else {
                let (line, rest) = text.split_fitting(max_width);
                text = rest;
                Some(line)
            }
        })
    }

    fn deduplicate_message(&self, next: &str) -> Option<String> {
        static RE: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r"^(.*) \(x(\d{1,8})\)$").unwrap());

        let mut base = self;
        let mut count: usize = 1;

        if let Some(caps) = RE.captures(self) {
            base = caps.get(1).expect("Invalid regex").as_str();
            count = caps
                .get(2)
                .expect("Invalid regex")
                .as_str()
                .parse()
                .expect("Invalid regex");
        }

        if base == next {
            Some(format!("{next} (x{})", count + 1))
        } else {
            None
        }
    }

    fn input_help_string(&self, command: &str) -> String {
        // XXX: Assumes strings are ASCII-7.
        if self.len() <= 1 {
            if let Some(p) = command.to_lowercase().find(&self.to_lowercase()) {
                if p == 0 {
                    return format!("{}){}", self, &command[1..]);
                } else {
                    return format!(
                        "{}({}){}",
                        &command[..p],
                        self,
                        &command[(p + 1)..]
                    );
                }
            }
        }
        format!("{}) {}", self, command)
    }

    fn is_capitalized(&self) -> bool {
        self.chars()
            .next()
            .map(|c| c.is_uppercase())
            .unwrap_or(false)
    }

    fn capitalize(&self) -> String {
        let mut chars = self.chars();
        match chars.next() {
            None => String::new(),
            Some(c) => c.to_uppercase().chain(chars).collect(),
        }
    }

    fn uncapitalize(&self) -> String {
        let mut chars = self.chars();
        match chars.next() {
            None => String::new(),
            Some(c) => c.to_lowercase().chain(chars).collect(),
        }
    }

    fn indentation(&self) -> usize {
        self.lines()
            .filter(|a| !a.trim().is_empty())
            .map(|a| a.chars().take_while(|c| c.is_whitespace()).count())
            .min()
            .unwrap_or(0)
    }

    fn char_grid(&self) -> impl Iterator<Item = (IVec2, char)> + '_ {
        let x_skip = self.indentation();

        self.lines()
            .skip_while(|a| a.trim().is_empty())
            .enumerate()
            .flat_map(move |(y, line)| {
                line.chars()
                    .skip(x_skip)
                    .enumerate()
                    .filter(|(_, c)| !c.is_whitespace())
                    .map(move |(x, c)| (ivec2(x as i32, y as i32), c))
            })
    }

    fn pluralize(&self, irregular_words: &HashMap<String, String>) -> String {
        if self.trim().is_empty() {
            return self.to_string();
        }

        // Pluralize before the " of whatever" part, if there is one.
        let (input, suffix) = if let Some(idx) = self.find(" of ") {
            (&self[..idx], &self[idx..self.len()])
        } else {
            (self, "")
        };
        let word = input.split(&[' ', '-'][..]).last().unwrap_or("");
        let prefix = &input[0..(input.len() - word.len())];

        if let Some(plural) = irregular_words.get(word) {
            let mut parts = plural.rsplitn(2, ' ');
            let plural = parts.next().unwrap_or("");

            if let Some(head) = parts.next() {
                return format!("{head} {prefix}{plural}{suffix}");
            } else {
                return format!("{prefix}{plural}{suffix}");
            }
        }

        if word.ends_with("ch")
            || word.ends_with('s')
            || word.ends_with("sh")
            || word.ends_with('x')
            || word.ends_with('z')
        {
            return format!("{prefix}{word}es{suffix}");
        }

        format!("{prefix}{word}s{suffix}")
    }

    fn templatize<F, E>(&self, mut mapper: F) -> Result<String, E>
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

        let mut text = self;

        let mut ret = String::new();
        let mut templating = false;
        while !text.is_empty() {
            let (mut chunk, remain) = next_chunk(text);
            text = remain;

            if templating {
                if chunk.is_capitalized() {
                    chunk = mapper(&chunk.uncapitalize())?;
                    chunk = chunk.capitalize();
                } else {
                    chunk = mapper(&chunk)?;
                }
            }

            ret += &chunk;
            templating = !templating;
        }
        Ok(ret)
    }
}

pub trait CharExt {
    fn is_vowel(&self) -> bool;
}

impl CharExt for char {
    fn is_vowel(&self) -> bool {
        // If accented chars are used, they need to be added here...
        matches!(self.to_ascii_lowercase(), 'a' | 'e' | 'i' | 'o' | 'u')
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_text() {
        assert_eq!(
            "’Twas brillig, and the slithy toves
      Did gyre and gimble in the wabe:
All mimsy were the borogoves,
      And the mome raths outgrabe."
                .lines_of(24,)
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
            "Really cancel the quitting of the stopping?"
                .lines_of(28)
                .collect::<Vec<_>>(),
            vec!["Really cancel the quitting", "of the stopping?"]
        );
    }

    #[test]
    fn capitalizers() {
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
            assert_eq!(&text.capitalize(), cap);
            assert_eq!(
                text.is_capitalized(),
                !text.is_empty() && text.capitalize() == text
            );
        }
    }

    #[test]
    fn plurals() {
        let irregulars: HashMap<String, String> =
            [("vortex", "vortices"), ("gauntlets", "pairs of gauntlets")]
                .iter()
                .map(|(a, b)| (a.to_string(), b.to_string()))
                .collect();

        for (a, b) in [
            ("", ""),
            ("cat", "cats"),
            ("box", "boxes"),
            ("bus", "buses"),
            ("splotch", "splotches"),
            ("wash", "washes"),
            ("wand of fireballs", "wands of fireballs"),
            ("vortex", "vortices"),
            ("vortex of doom", "vortices of doom"),
            ("crimson vortex of doom", "crimson vortices of doom"),
            ("hell-vortex of doom", "hell-vortices of doom"),
            (
                "uranium gauntlets of smiting",
                "pairs of uranium gauntlets of smiting",
            ),
        ] {
            assert_eq!(&a.pluralize(&irregulars), b);
        }
    }

    #[test]
    fn grids() {
        fn g(text: &str) -> Vec<(IVec2, char)> {
            text.char_grid().collect()
        }

        assert_eq!(g(""), vec![]);
        assert_eq!(g("A"), vec![(ivec2(0, 0), 'A')]);
        assert_eq!(
            g("AB\nC"),
            vec![(ivec2(0, 0), 'A'), (ivec2(1, 0), 'B'), (ivec2(0, 1), 'C')]
        );

        assert_eq!(g("  A"), vec![(ivec2(0, 0), 'A')]);
        assert_eq!(g("\n\n  A"), vec![(ivec2(0, 0), 'A')]);
        assert_eq!(g("A  B"), vec![(ivec2(0, 0), 'A'), (ivec2(3, 0), 'B')]);
        assert_eq!(g("\nA"), vec![(ivec2(0, 0), 'A')]);
        assert_eq!(g("A\n\nB"), vec![(ivec2(0, 0), 'A'), (ivec2(0, 2), 'B')]);
    }
}
