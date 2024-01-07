use clap::Parser;

use content::{mapgen, Block, Lot, Patch, SectorMap, Zone};
use util::{GameRng, Logos};

#[derive(Parser, Debug)]
#[command(about = "Test map generators")]
enum Args {
    Corridors(CorridorsArgs),
}

#[derive(Parser, Debug)]
struct CorridorsArgs {
    #[arg(long, value_name = "SEED", value_parser = |e: &str| Ok::<Logos, &str>(Logos::elite_new(e)), help = "Use a fixed generator seed")]
    seed: Option<Logos>,
}

impl CorridorsArgs {
    fn rng(&self) -> GameRng {
        if let Some(seed) = self.seed.as_ref() {
            util::srng(seed)
        } else {
            let seed = Logos::sample(
                &mut util::srng(&std::time::SystemTime::now()),
                10,
            );
            eprintln!("Generated seed: {seed}");
            util::srng(&seed)
        }
    }

    fn gen(&self) -> Patch {
        mapgen::rooms_and_corridors(
            &mut self.rng(),
            &Default::default(),
            0.2,
            0.03,
            0.5,
            0.0,
        )
        .expect("mapgen failed")
    }
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let mut map = match args {
        Args::Corridors(args) => args.gen(),
    };

    let volume = &Lot::default().volume;

    // Fill unmapped area with earth.
    for p in volume.fat() {
        if !map.terrain.contains_key(&p) {
            map.terrain.insert(p, Some(Block::Rock));
        }
    }

    let map = SectorMap::from_area(&map.terrain, &volume, &map.spawns);
    println!(
        "{}",
        idm::to_string(&map).expect("IDM serialization failed")
    );

    Ok(())
}
