// Release builds made for Windows don't create a terminal window when run.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use std::path::PathBuf;

use clap::Parser;

use engine::prelude::*;
use navni::prelude::*;
use ui::Game;
use util::{IncrementalOutline, Logos};

mod game_screen;

pub const GAME_NAME: &str = "gametemplate";

#[derive(Parser, Debug)]
struct Args {
    #[arg(long, value_parser = |e: &str| Ok::<Logos, &str>(Logos::new(e)), help = "Game world seed")]
    seed: Option<Logos>,

    #[arg(
        long,
        value_delimiter = ',',
        help = "Comma-separarted list of mod files to apply"
    )]
    mods: Vec<PathBuf>,
}

fn main() -> anyhow::Result<()> {
    util::panic_handler();
    navni::logger::start(GAME_NAME);

    let args = Args::parse();

    let mut mods: Vec<IncrementalOutline> = Default::default();
    for path in args.mods {
        let md = util::dir_to_idm(path)?;
        mods.push(md);
    }
    engine::register_mods(mods);

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
