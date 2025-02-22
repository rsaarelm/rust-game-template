use std::path::PathBuf;

use clap::Parser;

use util::{GameRng, IncrementalOutline, Outline, Silo};
use world::{Block, Lot, Patch, SectorMap, Zone, mapgen};

#[derive(Parser, Debug)]
#[command(about = "Test map generators")]
struct Args {
    /// Load game data from a given path instead of using default data.
    #[arg(long, value_name = "PATH")]
    data_dir: Option<PathBuf>,

    /// Comma-separarted list of mod files to apply
    #[arg(
        long,
        value_delimiter = ',',
        help = "Comma-separarted list of mod files to apply"
    )]
    mods: Vec<PathBuf>,

    #[command(subcommand)]
    command: Cmds,
}

#[derive(Parser, Debug)]
#[command(about = "Test map generators")]
enum Cmds {
    /// Generate a rooms and corridors map.
    Corridors(CorridorsArgs),
}

#[derive(Parser, Debug)]
struct CorridorsArgs {
    #[arg(long, value_name = "SEED", value_parser = |e: &str| Ok::<Silo, &str>(Silo::new(e)))]
    /// Fixed RNG seed.
    seed: Option<Silo>,

    #[arg(long)]
    /// Is the map connected horizontally to neighbors.
    connected: bool,

    #[arg(long, default_value = "0.1")]
    /// How much of the map is rooms.
    roominess: f32,
    #[arg(long, default_value = "0.1")]
    /// How many looping paths there are.
    loopiness: f32,
    // 0.8
    #[arg(long, default_value = "0.8")]
    /// How much of the map is tunnels.
    maziness: f32,
    // 0.0
    #[arg(long, default_value = "0.0")]
    /// How much of the map is carved into cave.
    caviness: f32,
}

impl CorridorsArgs {
    fn rng(&self) -> GameRng {
        if let Some(seed) = self.seed.as_ref() {
            util::srng(seed)
        } else {
            let seed = Silo::sample(
                &mut util::srng(&std::time::SystemTime::now()),
                10,
            );
            eprintln!("Generated seed: {seed}");
            util::srng(&seed)
        }
    }

    fn build(&self) -> Patch {
        let mut lot = Lot::default();
        lot.sides = if self.connected { 0b1111 } else { 0 };

        mapgen::rooms_and_corridors(
            &mut self.rng(),
            &lot,
            self.roominess,
            self.loopiness,
            self.maziness,
            self.caviness,
        )
        .expect("mapgen failed")
    }
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let mut mods: Vec<IncrementalOutline> = Default::default();
    for path in args.mods {
        let md = util::dir_to_idm(path)?;
        mods.push(md);
    }

    let mut data: Outline = if let Some(data_dir) = args.data_dir.as_ref() {
        let data = util::dir_to_idm(data_dir)?.to_string();
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

    world::register_data(idm::transmute(&data)?);

    let now = std::time::Instant::now();
    let mut map = match args.command {
        Cmds::Corridors(args) => args.build(),
    };
    let elapsed = now.elapsed();

    let volume = &Lot::default().volume;

    // Fill unmapped area with earth.
    for p in volume.fat() {
        if !map.terrain.contains_key(&p) {
            map.terrain.insert(p, Some(Block::Stone));
        }
    }

    let map = SectorMap::from_area(&map.terrain, &volume, &map.spawns);
    println!(
        "{}",
        idm::to_string(&map).expect("IDM serialization failed")
    );

    eprintln!("Map generated in {elapsed:.2?}");

    Ok(())
}
