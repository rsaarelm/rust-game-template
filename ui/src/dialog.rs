use std::borrow::Cow;

use crate::{prelude::*, ConfirmationDialog};
use navni::X256Color as X;

pub async fn ask(msg: impl Into<Cow<'_, str>>) -> bool {
    let dialog = ConfirmationDialog::new(msg);
    let mut win = Window::root().center(dialog.preferred_size().unwrap());
    win.foreground_col = X::BROWN;

    let _backdrop = Backdrop::from(win);
    loop {
        if game().draw().await.is_none() {
            return false;
        }

        if let Some(ret) = dialog.render(&win) {
            return ret;
        }
    }
}
