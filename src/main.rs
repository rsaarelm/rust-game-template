use gfx::prelude::*;
use navni::prelude::*;
use ui::Game;

mod wasm_getrandom;

fn hello(g: &mut Game, b: &mut dyn Backend, _: u32) -> Option<StackOp<Game>> {
    g.s.as_mut()[0] =
        CharCell::new('@', X256Color::FOREGROUND, X256Color::BACKGROUND);

    let win = Window::from(&g.s);
    win.write(&mut g.s, [0, 0], "@");
    win.write(&mut g.s, [10, 10], "Hello, world!");

    g.draw(b);

    None
}

fn main() {
    run(
        &Config {
            window_title: "gametemplate".to_string(),
            ..Default::default()
        },
        Game::default(),
        hello,
    );
}
