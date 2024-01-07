use clap::Parser;

use content::{mapgen, Lot, Patch, SectorMap};
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
        mapgen::bigroom(&mut self.rng(), &Default::default())
            .expect("mapgen failed")
    }
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let map = match args {
        Args::Corridors(args) => args.gen(),
    };
    let map =
        SectorMap::from_area(&map.terrain, &Lot::default().volume, &map.spawns);
    println!(
        "{}",
        idm::to_string(&map).expect("IDM serialization failed")
    );

    Ok(())
}
