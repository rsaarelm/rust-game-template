use rand::Rng;

use engine::prelude::*;
use navni::prelude::*;
use ui::Game;

mod game_screen;
mod wasm_getrandom;

const GAME_NAME: &str = "gametemplate";

fn main() {
    navni::logger::start(GAME_NAME);

    let world: World = rand::thread_rng().gen();
    let game = Game::new(Runtime::new(&world).unwrap());

    run(
        &Config {
            window_title: GAME_NAME.to_string(),
            system_color_palette: Some(LIGHT_PALETTE),
            ..Default::default()
        },
        game,
        game_screen::run,
    );
}

const LIGHT_PALETTE: [Rgba; 16] = [
    Rgba::new(0xaa, 0xaa, 0xaa, 0xff), // white
    Rgba::new(0x66, 0x00, 0x00, 0xff), // maroon
    Rgba::new(0x00, 0x66, 0x00, 0xff), // green
    Rgba::new(0x66, 0x33, 0x00, 0xff), // brown
    Rgba::new(0x00, 0x00, 0x88, 0xff), // navy
    Rgba::new(0x66, 0x00, 0x66, 0xff), // purple
    Rgba::new(0x00, 0x66, 0x66, 0xff), // teal
    Rgba::new(0x33, 0x33, 0x33, 0xff), // gray
    Rgba::new(0x77, 0x77, 0x77, 0xff), // silver
    Rgba::new(0xaa, 0x00, 0x00, 0xff), // red
    Rgba::new(0x00, 0xaa, 0x00, 0xff), // lime
    Rgba::new(0xaa, 0x55, 0x00, 0xff), // yellow
    Rgba::new(0x00, 0x00, 0xaa, 0xff), // blue
    Rgba::new(0xaa, 0x00, 0xaa, 0xff), // fuchsia
    Rgba::new(0x00, 0x99, 0x99, 0xff), // aqua
    Rgba::new(0x00, 0x00, 0x00, 0xff), // black
];
