//! Emitting messages about instant events to the UI layer

use std::sync::{
    mpsc::{self, Sender},
    LazyLock, Mutex,
};

use anyhow::bail;
use derive_deref::Deref;
use util::{text, Noun, Sentence};

use crate::prelude::*;

/// Interface for receiving game event messages for displaying.
pub enum Msg {
    /// Text message.
    Message(String),

    /// Entity e shot a projectile towards direction.
    Fire(Entity, IVec2),

    /// Entity is hurt.
    Hurt(Entity),

    /// An attack missed an entity.
    Miss(Entity),

    /// Entity dies.
    Death(Location),
}

static RCV: LazyLock<Mutex<Option<Sender<Msg>>>> =
    LazyLock::new(Default::default);

#[derive(Deref)]
pub struct Receiver(mpsc::Receiver<Msg>);

impl Default for Receiver {
    fn default() -> Self {
        let (send, recv) = mpsc::channel();
        *RCV.lock().unwrap() = Some(send);
        Receiver(recv)
    }
}

pub fn send_msg(msg: Msg) {
    if let Some(ref mut sender) = *RCV.lock().unwrap() {
        sender.send(msg).expect("Msg channel failure");
    }
}

pub trait Grammatize {
    fn format(&self, s: &str) -> String;
}

impl Grammatize for () {
    fn format(&self, s: &str) -> String {
        text::templatize(|_| bail!("no nouns"), s).unwrap()
    }
}

impl Grammatize for (Noun,) {
    fn format(&self, s: &str) -> String {
        text::templatize(|e| self.0.convert(e), s).unwrap()
    }
}

impl Grammatize for (Noun, Noun) {
    fn format(&self, s: &str) -> String {
        text::templatize(|e| Sentence::new(&self.0, &self.1).convert(e), s)
            .unwrap()
    }
}

#[macro_export]
macro_rules! msg {
    ($fmt:expr) => {
        $crate::send_msg($crate::Msg::Message($fmt.into()));
    };

    ($fmt:expr, $($arg:expr),*) => {
        let __txt = format!($fmt, $($arg),*);
        $crate::send_msg($crate::Msg::Message(__txt));
    };

    ($fmt:expr; $($grammar_arg:expr),*) => {
        let __txt = $crate::Grammatize::format(&($($grammar_arg,)*), $fmt);
        $crate::send_msg($crate::Msg::Message(__txt));
    };

    ($fmt:expr, $($arg:expr),*; $($grammar_arg:expr),*) => {
        let __txt = format!($fmt, $($arg),*);
        let __txt = $crate::Grammatize::format(&($($grammar_arg,)*), &__txt);
        $crate::send_msg($crate::Msg::Message(__txt));
    };
}
