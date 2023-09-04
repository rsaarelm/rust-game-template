use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

use crate::text;

#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub enum Noun {
    You,
    He(String),
    She(String),
    It(String),
    Plural(String),
}

use Noun::*;

impl Noun {
    pub fn third_person_singular(&self) -> bool {
        matches!(self, He(_) | She(_) | It(_))
    }

    pub fn is_you(&self) -> bool {
        matches!(self, You)
    }

    pub fn name(&self) -> &str {
        match self {
            You => "you",
            He(n) | She(n) | It(n) | Plural(n) => n,
        }
    }

    pub fn is_proper_noun(&self) -> bool {
        text::is_capitalized(self.name())
    }

    pub fn the_name(&self) -> String {
        if matches!(self, You) {
            "you".into()
        } else if self.is_proper_noun() {
            self.name().into()
        } else {
            format!("the {}", self.name())
        }
    }

    pub fn a_name(&self) -> String {
        if matches!(self, You) {
            "you".into()
        } else if self.is_proper_noun() || matches!(self, Plural(_)) {
            self.name().into()
        } else {
            // TODO: Add look-up table of irregular words ('honor', 'unit') as they show up in game
            // text.
            let article =
                if self.name().chars().next().map_or(false, text::is_vowel) {
                    "an"
                } else {
                    "a"
                };
            format!("{article} {}", self.name())
        }
    }

    pub fn they(&self) -> &str {
        match self {
            You => "you",
            He(_) => "he",
            She(_) => "she",
            It(_) => "it",
            Plural(_) => "they",
        }
    }

    pub fn them(&self) -> &str {
        match self {
            You => "you",
            He(_) => "him",
            She(_) => "her",
            It(_) => "it",
            Plural(_) => "them",
        }
    }

    pub fn their(&self) -> &str {
        match self {
            You => "your",
            He(_) => "his",
            She(_) => "her",
            It(_) => "its",
            Plural(_) => "their",
        }
    }

    pub fn possessive(&self) -> String {
        match self {
            You => "your".into(),
            n => {
                let mut s = n.the_name();
                s += "'s";
                s
            }
        }
    }

    pub fn themselves(&self) -> &str {
        match self {
            You => "yourself",
            He(_) => "himself",
            She(_) => "herself",
            It(_) => "itself",
            Plural(_) => "themselves",
        }
    }

    pub fn convert(&self, token: &str) -> Result<String> {
        let ret = match token {
            "some" => self.a_name(),
            "one" => self.the_name(),
            "one's" => self.their().into(),
            "oneself" => self.themselves().into(),
            "they" => self.they().into(),

            // Second / third person verb endings and irregular verbs.
            // All of these are assummed to apply to subject.
            // hit/hits
            "s" => {
                if self.third_person_singular() {
                    "s".into()
                } else {
                    "".into()
                }
            }
            // slash/slashes
            "es" => {
                if self.third_person_singular() {
                    "es".into()
                } else {
                    "".into()
                }
            }
            // parry/parries
            "ies" => {
                if self.third_person_singular() {
                    "ies".into()
                } else {
                    "y".into()
                }
            }
            "is" | "are" => {
                if self.third_person_singular() {
                    "is".into()
                } else {
                    "are".into()
                }
            }
            "has" | "have" => {
                if self.third_person_singular() {
                    "has".into()
                } else {
                    "have".into()
                }
            }

            _ => {
                bail!("bad token")
            }
        };
        Ok(ret)
    }
}

pub struct Sentence<'a> {
    subject: &'a Noun,
    object: &'a Noun,
}

impl<'a> Sentence<'a> {
    pub fn new(subject: &'a Noun, object: &'a Noun) -> Sentence<'a> {
        Sentence { subject, object }
    }

    pub fn convert(&self, token: &str) -> Result<String> {
        let ret = match token {
            "another" => self.object.the_name(),
            "a thing" => self.object.a_name(),
            "another's" => self.object.possessive(),
            "their" => self.object.their().into(),
            "them" => self.object.them().into(),

            _ => {
                return self.subject.convert(token);
            }
        };
        Ok(ret)
    }
}

#[cfg(test)]
mod test {
    use super::{
        Noun::{self, *},
        Sentence,
    };
    use crate::text::templatize;

    fn make_noun(name: &str) -> Noun {
        match name {
            "PLAYER" => You,
            "Alexander" => He("Alexander".into()),
            "Athena" => She("Athena".into()),
            "2 rocks" => Plural("2 rocks".into()),
            thing => It(thing.into()),
        }
    }

    fn parse_subj(test_script: &str) -> Vec<(&str, &str, &str)> {
        test_script
            .lines()
            .filter_map(|s| {
                let s = s.trim();
                if s.is_empty() {
                    None
                } else {
                    Some(s)
                }
            })
            .collect::<Vec<&str>>()
            .chunks(3)
            .map(|a| (a[0], a[1], a[2]))
            .collect()
    }

    fn parse_obj(test_script: &str) -> Vec<(&str, &str, &str, &str)> {
        test_script
            .lines()
            .filter_map(|s| {
                let s = s.trim();
                if s.is_empty() {
                    None
                } else {
                    Some(s)
                }
            })
            .collect::<Vec<&str>>()
            .chunks(4)
            .map(|a| (a[0], a[1], a[2], a[3]))
            .collect()
    }

    #[test]
    fn test_templating_1() {
        for (subject, template, message) in parse_subj(
            "PLAYER
            [One] drink[s] the potion.
            You drink the potion.

            goblin
            [One] drink[s] the potion.
            The goblin drinks the potion.

            PLAYER
            [One] rush[es] through the door.
            You rush through the door.

            goblin
            [One] rush[es] through the door.
            The goblin rushes through the door.

            PLAYER
            The spear runs [one] through.
            The spear runs you through.

            goblin
            The spear runs [one] through.
            The spear runs the goblin through.

            Alexander
            The spear runs [one] through.
            The spear runs Alexander through.

            PLAYER
            [One] [is] the chosen one. [They] [have] a rock.
            You are the chosen one. You have a rock.

            Athena
            [One] [is] the chosen one. [They] [have] a rock.
            Athena is the chosen one. She has a rock.

            PLAYER
            [One] nimbly parr[ies] the blow.
            You nimbly parry the blow.

            goblin
            [One] nimbly parr[ies] the blow.
            The goblin nimbly parries the blow.",
        )
        .into_iter()
        {
            let t = make_noun(subject);
            assert_eq!(
                templatize(|e| t.convert(e), template).unwrap(),
                message
            );
        }
    }

    #[test]
    fn test_templating_2() {
        for (subject, object, template, message) in parse_obj(
            "PLAYER
            goblin
            [One] hit[s] [another].
            You hit the goblin.

            goblin
            PLAYER
            [One] hit[s] [another].
            The goblin hits you.

            PLAYER
            goblin
            [One] chase[s] after [them].
            You chase after it.

            PLAYER
            wand of death
            [One] zap[s] [oneself] with [another].
            You zap yourself with the wand of death.

            Alexander
            wand of speed
            [One] zap[s] [oneself] with [another].
            Alexander zaps himself with the wand of speed.

            PLAYER
            Alexander
            [One] chase[s] after [them].
            You chase after him.

            goblin
            PLAYER
            [One] throw[s] [one's] javelin at [another].
            The goblin throws its javelin at you.

            PLAYER
            goblin
            [One] throw[s] [one's] javelin at [another].
            You throw your javelin at the goblin.

            goblin
            PLAYER
            [One] deftly slice[s] through [another's] neck with [one's] scimitar.
            The goblin deftly slices through your neck with its scimitar.

            PLAYER
            goblin
            [One] deftly slice[s] through [another's] neck with [one's] scimitar.
            You deftly slice through the goblin's neck with your scimitar.

            Alexander
            PLAYER
            [One] hit[s] [another] and disrupt[s] [their] spell.
            Alexander hits you and disrupts your spell.

            PLAYER
            Alexander
            [One] hit[s] [another] and disrupt[s] [their] spell.
            You hit Alexander and disrupt his spell.
            ",
        )
        .into_iter()
        {
            let a = make_noun(subject);
            let b = make_noun(object);
            assert_eq!(
                templatize(|e| Sentence::new(&a, &b).convert(e), template).unwrap(),
                message
            );
        }
    }

    #[test]
    fn test_plural() {
        for (subject, object, template, message) in parse_obj(
            "PLAYER
            rock
            [One] take[s] [a thing].
            You take a rock.

            PLAYER
            2 rocks
            [One] take[s] [a thing].
            You take 2 rocks.
            ",
        )
        .into_iter()
        {
            let a = make_noun(subject);
            let b = make_noun(object);
            assert_eq!(
                templatize(|e| Sentence::new(&a, &b).convert(e), template)
                    .unwrap(),
                message
            );
        }
    }
}
