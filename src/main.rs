use rand::Rng;

use engine::prelude::*;
use gfx::prelude::*;
use navni::prelude::*;
use ui::Game;

use engine::Rect;

mod wasm_getrandom;

fn hello(g: &mut Game, b: &mut dyn Backend, _: u32) -> Option<StackOp<Game>> {
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

        if let Some(e) = loc.mob_at(&g.r) {
            let mut icon = e.icon(&g.r);
            if g.r.player() == Some(e) {
                icon = '@';
            }
            win.write(&mut g.s, w, &format!("{}", icon));
        }
    }

    win.write(&mut g.s, [2, 35], "Hello, world!");

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
