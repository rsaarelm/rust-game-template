use std::{collections::BTreeMap, path::PathBuf};

use clap::Parser;
use util::{IndexMap, Silo};
use world::{AtlasKey, Location, Scenario, SectorMap, World};

#[derive(Parser, Debug)]
struct Args {
    scenario: PathBuf,

    #[arg(
        long,
        value_name = "SEED",
        value_parser = |e: &str| Ok::<Silo, &str>(Silo::new(e)),
        help = "Specify a seed"
    )]
    seed: Option<Silo>,

    #[arg(long, help = "Dump raw voxels instead of maps")]
    raw: bool,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let scenario: Scenario =
        idm::from_str(&std::fs::read_to_string(args.scenario)?)?;

    let seed = if let Some(seed) = args.seed {
        // A fixed seed was given, use that.
        seed
    } else {
        // Otherwise sample from the system clock.
        Silo::sample(&mut rand::thread_rng(), 10)
    };

    eprintln!("seed: {seed}");

    let mut world = World::new(seed, scenario)?;

    let pts: Vec<Location> =
        world.levels().map(|a| Location::from(a.min())).collect();
    let mut spawns = IndexMap::default();
    for p in pts {
        for (a, b) in world.populate_around(p) {
            spawns.insert(a, b);
        }
    }

    if args.raw {
        println!("{}", idm::to_string(world.terrain_cache())?);
    } else {
        let mut levels = BTreeMap::default();
        for a in world.levels() {
            levels.insert(
                AtlasKey(Location::from(a.min())),
                SectorMap::from_area(&world, a, &spawns),
            );
        }
        println!("{}", idm::to_string(&levels)?);
    }

    Ok(())
}
