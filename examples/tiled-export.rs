use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::Result;
use clap::Parser;
use glam::{IVec3, ivec2, ivec3};
use serde::{Deserialize, Serialize};

use util::{HashMap, HashSet, StrExt};
use world::{Rect, Region, SECTOR_HEIGHT, SECTOR_WIDTH, Scenario, SectorMap};

// NB. This thing is sort of weird and janky, the actual spatial positions of
// prefab maps aren't stable with the presence of procgen segments and
// branchings, so regions are compressed into dense stacks of only the prefab
// maps.
//
// The IDM scenario file is the ultimate source of truth. The existing prefab
// layers must be specified manually in the IDM, drawing outside the bounds in
// the Tiled file will not add anything. So if you need 5 over-the-ground site
// layers on one region, you must add dummy layers (that should have some
// non-void terrain or the tool will ignore them) to the IDM first and then
// draw the proper terrain in Tiled. Legend data also lives in IDM scenario
// and can't be specified in Tiled, you just paint the ASCII letters with
// Tiled and then specify per-sector meanings by editing the IDM.
//
// XXX: Above-ground must have at least one initial non-'_' tile to show up in
// Tiled export.
//
// You need to have mapedit-tiles.png available in the directory of the
// exported Tiled json file.

const TILE_W: u32 = 8;
const TILE_H: u32 = 8;

#[derive(Parser, Debug)]
enum Args {
    /// Generate a Tiled map file from the given IDM scenario file.
    Extract(Param),
    /// Rewrite the contents of an IDM scenario file based on a Tiled map file
    /// generated from it.
    Inject(Param),
}

#[derive(Parser, Debug)]
struct Param {
    path: PathBuf,
}

fn main() -> Result<()> {
    let args = Args::parse();

    match args {
        Args::Extract(param) => extract(&param.path),
        Args::Inject(param) => inject(&param.path),
    }
}

fn extract(path: &Path) -> Result<()> {
    let scenario: Scenario = idm::from_str(&fs::read_to_string(path)?)?;

    let mut cells = Vec::new();

    // In case the scenario overworld reuses a region with maps attached to it,
    // only extract the first instance of it. This makes it unambiguous which
    // part of the Tiled file must be edited to propagate the changes back.
    let mut seen_regions = HashSet::default();

    let regions = scenario.indexed_map()?;
    for (p, idx) in regions {
        if seen_regions.contains(&idx) {
            continue;
        }
        seen_regions.insert(idx);

        let p = p * ivec2(SECTOR_WIDTH, SECTOR_HEIGHT);

        let (overground, underground) = extract_maps(&scenario.legend[idx].1);
        for (z, map) in overground
            .iter()
            .enumerate()
            .map(|(i, m)| (overground.len() as i32 - 1 - i as i32, m))
        {
            let p = p.extend(z);
            for (q, c) in map.map.char_grid() {
                // XXX: Special case, '_' is used to represent holes in maps,
                // Tiled version does transparency instead so just no-op here.
                if c == '_' {
                    continue;
                }
                cells.push((p + q.extend(0), c as u32));
            }
        }
        for (z, map) in underground
            .iter()
            .enumerate()
            .map(|(i, m)| (-1 - i as i32, m))
        {
            let p = p.extend(z);
            for (q, c) in map.map.char_grid() {
                if c == '_' {
                    continue;
                }
                cells.push((p + q.extend(0), c as u32));
            }
        }
    }

    let tiled: Map = cells.into_iter().collect();
    let output = path.with_extension("json");
    fs::write(&output, serde_json::to_string(&tiled)?)?;
    eprintln!("Wrote Tiled world map to {}", output.display());
    Ok(())
}

fn inject(path: &Path) -> Result<()> {
    let scenario_path = path.with_extension("idm");
    let scenario_text = fs::read_to_string(&scenario_path)?;
    let mut scenario: Scenario = idm::from_str(&scenario_text)?;
    let tiled: Map = serde_json::from_str(&fs::read_to_string(path)?)?;
    let cells: HashMap<IVec3, u32> =
        tiled.iter().map(|(p, c)| (p.into(), c)).collect();

    let mut seen_regions = HashSet::default();

    let regions = scenario.indexed_map()?;
    for (p, idx) in regions {
        if seen_regions.contains(&idx) {
            continue;
        }
        seen_regions.insert(idx);
        let p = p * ivec2(SECTOR_WIDTH, SECTOR_HEIGHT);

        let (overground, underground) = extract_maps(&scenario.legend[idx].1);

        let mut new_overground = Vec::new();
        for (z, old_map) in overground
            .iter()
            .enumerate()
            .map(|(i, m)| (overground.len() as i32 - 1 - i as i32, m))
        {
            // Build a new text map.
            let mut map = String::new();
            for y in p[1]..(p[1] + SECTOR_HEIGHT) {
                for x in p[0]..(p[0] + SECTOR_WIDTH) {
                    let p = ivec3(x, y, z);
                    if let Some(c) = cells.get(&p) {
                        map.push(char::from_u32(*c).unwrap());
                    } else {
                        // Default to empty void overground.
                        map.push('_');
                    }
                }
                map.push('\n');
            }

            let mut new_map = old_map.clone();
            new_map.map = map;
            new_overground.push(new_map);
        }

        let mut new_underground = Vec::new();
        for (z, old_map) in underground
            .iter()
            .enumerate()
            .map(|(i, m)| (-1 - i as i32, m))
        {
            // Build a new text map.
            let mut map = String::new();
            for y in p[1]..(p[1] + SECTOR_HEIGHT) {
                for x in p[0]..(p[0] + SECTOR_WIDTH) {
                    let p = ivec3(x, y, z);
                    if let Some(c) = cells.get(&p) {
                        map.push(char::from_u32(*c).unwrap());
                    } else {
                        // Default to solid overground.
                        map.push('#');
                    }
                }
                map.push('\n');
            }

            let mut new_map = old_map.clone();
            new_map.map = map;
            new_underground.push(new_map);
        }

        new_overground.reverse();
        new_underground.reverse();

        inject_maps(
            scenario.legend[idx].1.as_mut(),
            new_overground,
            new_underground,
        );
    }

    fs::write(
        &scenario_path,
        idm::to_string_styled_like(&scenario_text, &scenario)?,
    )?;
    eprintln!(
        "Rewrote scenario file {} with Tiled map",
        scenario_path.display()
    );
    Ok(())
}

/// Extract maps into compacted overground and underground stacks.
fn extract_maps(regions: &[Region]) -> (Vec<SectorMap>, Vec<SectorMap>) {
    let mut overground = Vec::new();
    let mut underground = Vec::new();

    // Fill overground.
    for r in regions {
        if let Region::Site(map) = r {
            overground.push(map.clone());
        } else {
            break;
        }
    }

    fn push_underground(underground: &mut Vec<SectorMap>, rs: &[Region]) {
        for r in rs {
            // This was already handled.
            if r.is_site() {
                continue;
            }

            match r {
                Region::Hall(map) => underground.push(map.clone()),
                Region::Repeat(_, b) => {
                    if let Region::Hall(map) = &**b {
                        underground.push(map.clone());
                    }
                }
                Region::Branch(rs) => {
                    push_underground(underground, rs);
                }
                _ => {}
            }
        }
    }

    push_underground(&mut underground, regions);

    (overground, underground)
}

/// Inject map stacks back into region list.
///
/// Will panic unless region's overground and underground map counts match the
/// input lengths.
fn inject_maps(
    regions: &mut [Region],
    mut overground: Vec<SectorMap>,
    mut underground: Vec<SectorMap>,
) {
    for r in regions.iter_mut() {
        if matches!(r, Region::Site(_)) {
            *r = Region::Site(overground.pop().unwrap());
        }
    }

    fn write_underground(
        regions: &mut [Region],
        underground: &mut Vec<SectorMap>,
    ) {
        if regions.is_empty() {
            assert!(underground.is_empty());
            return;
        }

        for r in regions.iter_mut() {
            match r {
                Region::Hall(_) => {
                    *r = Region::Hall(underground.pop().unwrap());
                }
                Region::Repeat(n, b) => {
                    if let Region::Hall(_) = &**b {
                        *r = Region::Repeat(
                            *n,
                            Box::new(Region::Hall(underground.pop().unwrap())),
                        );
                    }
                }
                Region::Branch(rs) => {
                    write_underground(rs, underground);
                }
                _ => {}
            }
        }
    }

    write_underground(regions, &mut underground);
}

// Tiled JSON schema

/// Toplevel Tiled map data.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Map {
    #[serde(rename = "type")]
    pub type_: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backgroundcolor: Option<String>,
    pub width: u32,
    pub height: u32,
    pub layers: Vec<Layer>,
    pub infinite: bool,
    pub nextlayerid: u32,
    pub nextobjectid: u32,
    pub orientation: Orientation,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<Vec<MapProperty>>,
    pub renderorder: String,
    pub tiledversion: String,
    pub tileheight: u32,
    pub tilewidth: u32,
    pub version: String,
    pub tilesets: Vec<Tileset>,
}

impl Default for Map {
    fn default() -> Self {
        Map {
            type_: "map".into(),
            backgroundcolor: None,
            width: 0,
            height: 0,
            layers: Vec::new(),
            infinite: false,
            nextlayerid: 1,
            nextobjectid: 1,
            orientation: Orientation::Orthogonal,
            properties: None,
            renderorder: "right-down".into(),
            tiledversion: "1.10.0".into(),
            tileheight: TILE_H,
            tilewidth: TILE_W,
            version: "1.10".into(),
            tilesets: vec![Tileset::new("mapedit-tiles.png")],
        }
    }
}

impl<P: Into<[i32; 3]>> FromIterator<(P, u32)> for Map {
    fn from_iter<T: IntoIterator<Item = (P, u32)>>(iter: T) -> Self {
        let mut ret = Map::default();
        ret.infinite = true;

        // Create slices.

        // Make sure layer zero (ground) is included in the layer stack even
        // if it has no content.
        let mut min_z = 0;
        let mut max_z = 0;
        let mut layers: HashMap<i32, HashMap<[i32; 2], u32>> =
            HashMap::default();
        for (p, a) in iter.into_iter() {
            let [x, y, z] = p.into();
            min_z = min_z.min(z);
            max_z = max_z.max(z);
            layers.entry(z).or_default().insert([x, y], a);
        }

        for z in min_z..=max_z {
            let name = format!(":z {z}");

            // Interleaved encoding into positive integers.
            let id = if z < 0 { -z * 2 - 1 } else { z * 2 } as u32;

            let layer = Layer::new(
                name,
                id,
                layers.entry(z).or_default().iter().map(|(&a, &b)| (a, b)),
            );

            ret.layers.push(layer);
        }

        ret
    }
}

impl Map {
    pub fn iter(&self) -> impl Iterator<Item = ([i32; 3], u32)> + '_ {
        // If no layer is clearly marked ground, assume we're looking at a
        // flat overland plus dungeons map and the ground layer is the topmost
        // one.
        let dz = self
            .layers
            .iter()
            .position(|x| x.is_ground())
            .unwrap_or(self.layers.len() - 1) as i32;

        self.layers.iter().enumerate().flat_map(move |(i, a)| {
            let z = i as i32 - dz;
            a.iter().map(move |([x, y], a)| ([x, y, z], a))
        })
    }

    pub fn save(
        &self,
        filename: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let json = serde_json::to_string_pretty(self)?;
        fs::write(filename, &json)?;
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Layer {
    name: String,
    id: u32,
    visible: bool,
    opacity: f32,

    x: i32,
    y: i32,

    #[serde(flatten)]
    variant: LayerVariant,
}

impl Layer {
    pub fn new<P: Into<[i32; 2]>>(
        name: impl AsRef<str>,
        id: u32,
        content: impl IntoIterator<Item = (P, u32)>,
    ) -> Self {
        Layer {
            name: name.as_ref().to_owned(),
            id,
            visible: true,
            opacity: 1.0,
            // Hopefully these won't be needed if we do chunks.
            x: 0,
            y: 0,
            variant: content.into_iter().collect(),
        }
    }

    /// Is this layer the ground level (z = 0)?
    ///
    /// The ground layer is either recognized by a case-insensitive occurrence
    /// of the word "ground" somewhere in the layer name or the layer being
    /// named "Tile Layer 1", Tiled's default name for the first layer.
    pub fn is_ground(&self) -> bool {
        self.name == ":z 0"
    }

    pub fn iter(&self) -> impl Iterator<Item = ([i32; 2], u32)> + '_ {
        // Non-chunky layers not currently supported.

        let LayerVariant::TileLayer {
            chunks: Some(ChunkMap(ref chunks)),
            ..
        } = self.variant
        else {
            panic!("Unsupported Layer type");
        };

        chunks.iter().flat_map(|c| {
            let cell = Rect::new(
                [c.x, c.y],
                [c.x + c.width as i32, c.y + c.height as i32],
            );
            c.data.iter().enumerate().filter_map(move |(i, &a)| {
                (a != 0).then(|| (cell.get(i), a - 1))
            })
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum LayerVariant {
    #[serde(rename = "tilelayer")]
    TileLayer {
        width: u32,
        height: u32,

        #[serde(skip_serializing_if = "Option::is_none")]
        chunks: Option<ChunkMap>,
        #[serde(skip_serializing_if = "Option::is_none")]
        data: Option<Vec<u32>>,
        // Not supported: compression, encoding
    },
    #[serde(rename = "objectgroup")]
    ObjectGroup {
        draworder: String,
        objects: Vec<Object>,
    },
    // Not supported: imagelayer, group
}

impl<P: Into<[i32; 2]>> FromIterator<(P, u32)> for LayerVariant {
    fn from_iter<T: IntoIterator<Item = (P, u32)>>(iter: T) -> Self {
        let mut chunks: HashMap<Rect, Vec<u32>> = HashMap::default();
        const W: i32 = 16;
        const H: i32 = 16;

        let mut min = [i32::MAX, i32::MAX];
        let mut max = [i32::MIN, i32::MIN];

        for (p, a) in iter.into_iter() {
            let p = p.into();
            min = [min[0].min(p[0]), min[1].min(p[1])];
            max = [max[0].max(p[0]), max[1].max(p[1])];
            let cell = Rect::cell_containing([W, H], p);
            let data = chunks
                .entry(cell)
                .or_insert_with(|| vec![0; (W * H) as usize]);

            // Zero is used for missing value, so real values are all
            // incremented.
            data[cell.idx(p)] = a + 1;
        }

        let chunks = ChunkMap(
            chunks
                .into_iter()
                .map(|(b, data): (Rect, Vec<u32>)| Chunk {
                    data,
                    width: b.width() as u32,
                    height: b.height() as u32,
                    x: b.min()[0],
                    y: b.min()[1],
                })
                .collect(),
        );

        if chunks.0.is_empty() {
            min = [0, 0];
            max = [0, 0];
        }

        LayerVariant::TileLayer {
            width: (max[0] - min[0]) as u32,
            height: (max[1] - min[1]) as u32,
            chunks: Some(chunks),
            data: None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Orientation {
    Orthogonal,
    Isometric,
    Staggered,
    Hexagonal,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum MapProperty {
    #[serde(rename = "boolean")]
    Boolean { name: String, value: bool },
    #[serde(rename = "color")]
    Color {
        name: String,
        value: String, // TODO: Deserialize to colory type from "#rrggbbaa"
    },
    #[serde(rename = "file")]
    File { name: String, value: PathBuf },
    #[serde(rename = "float")]
    Float { name: String, value: f64 },
    #[serde(rename = "int")]
    Int { name: String, value: i32 },
    #[serde(rename = "string")]
    String { name: String, value: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChunkMap(pub Vec<Chunk>);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Chunk {
    pub data: Vec<u32>,
    pub width: u32,
    pub height: u32,
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Object {
    #[serde(rename = "type")]
    pub type_: String,
    pub gid: u32,
    pub id: u32,
    pub name: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub rotation: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Tileset {
    pub columns: u32,
    pub tilecount: u32,
    pub tilewidth: u32,
    pub tileheight: u32,
    pub spacing: u32,
    pub firstgid: u32,
    pub image: PathBuf,
    pub imagewidth: u32,
    pub imageheight: u32,
    pub margin: u32,
    pub name: String,
}

impl Tileset {
    pub fn new(file: impl AsRef<str>) -> Tileset {
        Tileset {
            columns: 16,
            tilecount: 256,
            tilewidth: TILE_W,
            tileheight: TILE_H,
            spacing: 0,
            firstgid: 1,
            image: file.as_ref().into(),
            imagewidth: 16 * TILE_W,
            imageheight: 16 * TILE_H,
            margin: 0,
            name: "mapedit-tiles".into(),
        }
    }
}
