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
    let p0 = origin.unfold_wide();
    for p in Rect::sized([SECTOR_WIDTH * 2, SECTOR_HEIGHT]) {
        let p_loc = p0 + v2(p);

        win.put(&mut g.s, p, ui::map_display::terrain_cell(&g.r, p_loc));
        if p_loc.x % 2 == 0 {
            let loc = Location::fold_wide(p_loc);

            if let Some(e) = loc.mob_at(&g.r) {
                let mut icon = e.icon(&g.r);
                if g.r.player() == Some(e) {
                    icon = '@';
                }
                win.put(&mut g.s, p, CharCell::c(icon));
            }
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
