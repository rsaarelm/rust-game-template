use rand::Rng;

use engine::prelude::*;
use navni::prelude::*;
use ui::Game;

mod game_screen;

const GAME_NAME: &str = "gametemplate";

fn main() {
    navni::logger::start(GAME_NAME);

    let world: World = rand::thread_rng().gen();
    let game = Game::new(Runtime::new(&world).unwrap());

    run(
        &Config {
            window_title: GAME_NAME.to_string(),
            system_color_palette: Some(ui::LIGHT_PALETTE),
            ..Default::default()
        },
        game,
        game_screen::run,
    );
}
