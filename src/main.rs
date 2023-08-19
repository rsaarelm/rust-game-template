use rand::Rng;

use engine::prelude::*;
use navni::prelude::*;
use ui::Game;

mod game_screen;
mod wasm_getrandom;

fn main() {
    let world: World = rand::thread_rng().gen();
    let game = Game::new(Runtime::new(&world).unwrap());

    run(
        &Config {
            window_title: "gametemplate".to_string(),
            ..Default::default()
        },
        game,
        game_screen::run,
    );
}
