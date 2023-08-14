use gfx::Buffer;
use navni::prelude::*;

const WIDTH: u32 = 160;
const HEIGHT: u32 = 45;

struct GameState {
    buf: Buffer<CharCell>,
}

impl Default for GameState {
    fn default() -> Self {
        let mut buf = Buffer::new(WIDTH, HEIGHT);
        buf.as_mut()[0] =
            CharCell::new('@', X256Color::FOREGROUND, X256Color::BACKGROUND);

        GameState { buf }
    }
}

fn hello(
    game: &mut GameState,
    b: &mut dyn Backend,
    _: u32,
) -> Option<StackOp<GameState>> {
    b.draw_chars(WIDTH, HEIGHT, game.buf.as_ref());

    None
}

fn main() {
    run(
        &Config {
            window_title: "gametemplate".to_string(),
            ..Default::default()
        },
        GameState::default(),
        hello,
    );
}
