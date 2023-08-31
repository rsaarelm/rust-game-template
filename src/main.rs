use clap::Parser;

use engine::prelude::*;
use navni::prelude::*;
use ui::Game;
use util::Logos;

mod game_screen;

pub const GAME_NAME: &str = "gametemplate";

#[derive(Parser, Debug)]
struct Args {
    #[arg(long, value_parser = |e: &str| Ok::<Logos, &str>(Logos::new(e)), help = "Game world seed")]
    seed: Option<Logos>,
}

fn main() {
    navni::logger::start(GAME_NAME);

    let args = Args::parse();

    let seed = args
        .seed
        .unwrap_or_else(|| Logos::sample(&mut rand::thread_rng(), 10));
    log::info!("seed: {seed}");

    let game = Game::new(Runtime::new(WorldSpec::new(seed)).unwrap());

    run(
        &Config {
            application_name: GAME_NAME.to_string(),
            system_color_palette: Some(ui::LIGHT_PALETTE),
            ..Default::default()
        },
        (game, game_screen::run),
    );
}
