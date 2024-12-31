//! Emitting messages about instant events to the UI layer

use std::sync::{
    mpsc::{self, Sender},
    LazyLock, Mutex,
};

use anyhow::bail;
use derive_more::Deref;
use util::{Noun, Sentence, StrExt};

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

    /// A fireball explodes.
    Explosion(Location),

    /// A lightning bolt hits an entity.
    LightningBolt(Location),

    /// Magic mapping effect
    ///
    /// Lists the exposed locations and how far they are from the start of the
    /// exposure event.
    MagicMap(Vec<(Location, usize)>),

    /// Altar was activated, run client-side altar menu.
    ActivatedAltar(Location),
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
        s.templatize(|_| bail!("no nouns")).unwrap()
    }
}

impl Grammatize for (Noun,) {
    fn format(&self, s: &str) -> String {
        s.templatize(|e| self.0.convert(e)).unwrap()
    }
}

impl Grammatize for (Noun, Noun) {
    fn format(&self, s: &str) -> String {
        s.templatize(|e| Sentence::new(&self.0, &self.1).convert(e))
            .unwrap()
    }
}

#[macro_export]
macro_rules! msg {
    // NB. Even the simple cases needs to be wrapped in `format!` in case the
    // fmt string is doing named variable capture.
    ($fmt:expr) => {
        $crate::send_msg($crate::Msg::Message(format!($fmt)))
    };

    ($fmt:expr, $($arg:expr),*) => {
        let __txt = format!($fmt, $($arg),*);
        $crate::send_msg($crate::Msg::Message(__txt))
    };

    ($fmt:expr; $($grammar_arg:expr),*) => {
        let __txt = format!($fmt);
        let __txt = $crate::Grammatize::format(&($($grammar_arg,)*), &__txt);
        $crate::send_msg($crate::Msg::Message(__txt))
    };

    ($fmt:expr, $($arg:expr),*; $($grammar_arg:expr),*) => {
        let __txt = format!($fmt, $($arg),*);
        let __txt = $crate::Grammatize::format(&($($grammar_arg,)*), &__txt);
        $crate::send_msg($crate::Msg::Message(__txt))
    };
}
