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
        let mut lot = Lot::default();
        lot.sides = 0b1111;

        mapgen::rooms_and_corridors(&mut self.rng(), &lot, 0.1, 0.1, 0.8, 0.0)
            .expect("mapgen failed")
    }
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let now = std::time::Instant::now();
    let mut map = match args {
        Args::Corridors(args) => args.gen(),
    };
    let elapsed = now.elapsed();

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

    eprintln!("Map generated in {elapsed:.2?}");

    Ok(())
}
