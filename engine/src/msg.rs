use std::sync::{
    mpsc::{self, Sender},
    LazyLock, Mutex,
};

use derive_deref::Deref;

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
        // TODO 2023-02-04 Support grammar templating
        msg!($fmt)
    };

    ($fmt:expr, $($arg:expr),*; $($grammar_arg:expr),*) => {
        msg!($fmt, $($arg),*)
    };
}
