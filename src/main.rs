// Release builds made for Windows don't create a terminal window when run.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use std::path::PathBuf;

use clap::Parser;
use engine::prelude::*;
use ui::game;
use util::{IncrementalOutline, Logos};
use version::VERSION;

mod map_view;
mod run;
mod version;
mod view;

/// Human-readable game title.
pub const GAME_NAME: &str = "Template Game";

/// System identifier for game.
pub const GAME_ID: &str = "gametemplate";

#[derive(Parser, Debug)]
struct Args {
    #[arg(
        long,
        value_name = "SEED",
        value_parser = |e: &str| Ok::<Logos, &str>(Logos::elite_new(e)),
    )]
    /// Start a new game, optionally with specific seed
    new_game: Option<Option<Logos>>,

    #[arg(
        long,
        value_delimiter = ',',
        help = "Comma-separarted list of mod files to apply"
    )]
    /// Comma-separarted list of mod files to apply
    mods: Vec<PathBuf>,

    #[arg(short = 'v', long)]
    /// Display game version
    version: bool,
}

fn main() -> anyhow::Result<()> {
    util::panic_handler();
    navni::logger::start(GAME_ID);

    let args = Args::parse();

    if args.version {
        println!("{GAME_NAME} version {VERSION}");
        return Ok(());
    }

    let mut mods: Vec<IncrementalOutline> = Default::default();
    for path in args.mods {
        let md = util::dir_to_idm(path)?;
        mods.push(md);
    }
    content::register_mods(mods);

    navni::run(GAME_ID, async move {
        ui::init_game();

        if args.new_game.is_some() {
            log::info!("New game requested, deleting any existing saves");
            game().delete_save(GAME_ID);
        }

        // Restore game or init a new one.
        loop {
            match game().load(GAME_ID) {
                Ok(None) => {
                    // No save file found, initialize a new game.
                    let seed = if let Some(Some(logos)) = args.new_game {
                        // A fixed seed was given, use that.
                        logos
                    } else {
                        // Otherwise sample from the system clock.
                        Logos::sample(
                            &mut util::srng(&navni::now().to_le_bytes()),
                            10,
                        )
                    };

                    log::info!("seed: {seed}");

                    game().r = Runtime::new(seed).unwrap();

                    msg!("Welcome to {}, {}!", GAME_NAME, util::user_name());
                }
                Ok(Some(save)) => {
                    // Load the save.
                    game().replace_runtime(save);
                    msg!("Welcome back, {}!", util::user_name());
                }
                Err(_) => {
                    game().draw().await;
                    if crate::run::ask("Corrupt save file detected. Delete it?")
                        .await
                    {
                        game().delete_save(GAME_ID);
                        continue;
                    } else {
                        // Can't load the save file and can't clobber it, exiting
                        // game.
                        return;
                    }
                }
            }
            break;
        }
        msg!("Build version {}", VERSION);

        game().viewpoint = game()
            .r
            .player()
            .and_then(|p| p.loc(game()))
            .unwrap_or_default();
        game().camera = game().viewpoint;

        navni::set_palette(&ui::LIGHT_PALETTE);

        run::main_gameplay().await;

        // Save the game if we exited with the game still running.
        if !game().is_game_over() {
            game().save(GAME_ID);
        } else {
            game().delete_save(GAME_ID);
        }
    });

    Ok(())
}
