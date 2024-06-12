// Release builds made for Windows don't create a terminal window when run.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use std::path::PathBuf;

use clap::Parser;
use content::settings;
use engine::prelude::*;
use ui::game;
use util::{IncrementalOutline, Outline, Silo};
use version::VERSION;

mod map_view;
mod run;
mod version;
mod view;

#[derive(Parser, Debug)]
struct Args {
    /// Start a new game, optionally with specific seed
    #[arg(
        long,
        value_name = "SEED",
        value_parser = |e: &str| Ok::<Silo, &str>(Silo::new(e)),
    )]
    new_game: Option<Option<Silo>>,

    /// Load game data from a given path instead of using default data.
    #[arg(long, value_name = "PATH")]
    gamedata: Option<PathBuf>,

    /// Comma-separarted list of mod files to apply
    #[arg(
        long,
        value_delimiter = ',',
        help = "Comma-separarted list of mod files to apply"
    )]
    mods: Vec<PathBuf>,

    /// Display game version and exit
    #[arg(short = 'v', long)]
    version: bool,
}

fn main() -> anyhow::Result<()> {
    util::panic_handler();

    let args = Args::parse();

    if args.version {
        println!("{} version {VERSION}", settings().title);
        return Ok(());
    }

    let mut mods: Vec<IncrementalOutline> = Default::default();
    for path in args.mods {
        let md = util::dir_to_idm(path)?;
        mods.push(md);
    }

    let mut data: Outline = if let Some(gamedata) = args.gamedata.as_ref() {
        let data = util::dir_to_idm(gamedata)?.to_string();
        eprint!("{data}");
        idm::from_str(&data)?
    } else {
        let data = snap::raw::Decoder::new()
            .decompress_vec(include_bytes!("../target/data.idm.sz"))?;
        let data = std::str::from_utf8(&data)?;
        idm::from_str(data)?
    };

    for md in &mods {
        data += md;
    }

    content::register_data(idm::transmute(&data)?);

    navni::logger::start(&settings().id);

    navni::run(&settings().id, async move {
        ui::init_game();

        if args.new_game.is_some() {
            log::info!("New game requested, deleting any existing saves");
            game().delete_save(&settings().id);
        }

        // Restore game or init a new one.
        loop {
            match game().load(&settings().id) {
                Ok(None) => {
                    // No save file found, initialize a new game.
                    let seed = if let Some(Some(seed)) = args.new_game {
                        // A fixed seed was given, use that.
                        seed
                    } else {
                        // Otherwise sample from the system clock.
                        Silo::sample(
                            &mut util::srng(&navni::now().to_le_bytes()),
                            10,
                        )
                    };

                    log::info!("seed: {seed}");

                    game().r = Runtime::new(seed).unwrap();

                    msg!(
                        "Welcome to {}, {}!",
                        settings().title,
                        util::user_name()
                    );
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
                        game().delete_save(&settings().id);
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
            game().save(&settings().id);
        } else {
            game().delete_save(&settings().id);
        }
    });

    Ok(())
}
