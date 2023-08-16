use rand::Rng;

use engine::prelude::*;
use gfx::prelude::*;
use navni::prelude::*;
use ui::Game;

use engine::Rect;

mod wasm_getrandom;

fn hello(g: &mut Game, b: &mut dyn Backend, _: u32) -> Option<StackOp<Game>> {
    g.s.as_mut()[0] =
        CharCell::new('@', X256Color::FOREGROUND, X256Color::BACKGROUND);

    let win = Window::from(&g.s);

    // TODO camera.
    let origin = Location::default();
    for p in Rect::sized([SECTOR_WIDTH, SECTOR_HEIGHT]) {
        let p = IVec2::from(p);
        let w = p * ivec2(2, 1);
        let loc = origin + p;
        if loc.tile(&g.r) == Tile::Wall {
            win.write(&mut g.s, w, "#");
        }
    }

    win.write(&mut g.s, [0, 0], "@");
    win.write(&mut g.s, [10, 10], "Hello, world!");

    g.draw(b);

    None
}

fn main() {
    let world: World = rand::thread_rng().gen();
    let game = Game::new(Runtime::new(&world).unwrap());

    run(
        &Config {
            window_title: "gametemplate".to_string(),
            ..Default::default()
        },
        game,
        hello,
    );
}
