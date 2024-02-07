#![feature(int_roundings)]
use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::Result;
use clap::Parser;
use glam::{ivec2, IVec3};
use serde::{Deserialize, Serialize};

use content::{Rect, Region, Scenario, SectorMap, SECTOR_HEIGHT, SECTOR_WIDTH};
use util::{text, HashMap};

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

    // In case the scenario overworld reuses a letter with maps attached to it,
    // only extract the first instance of it. This makes it unambiguous which
    // part of the Tiled file must be edited to propagate the changes back.
    let mut seen_regions = HashSet::default();

    for (p, c) in text::char_grid(&scenario.map) {
        if seen_regions.contains(&c) {
            continue;
        }
        seen_regions.insert(c);

        let p = p * ivec2(SECTOR_WIDTH, SECTOR_HEIGHT);
        for (z, map) in map_stack(scenario.legend[&c].as_ref()) {
            let p = p.extend(z);
            for (q, c) in text::char_grid(&map.map) {
                // XXX: Special case, '_' is used to represent holes in maps,
                // Tiled version does transparency instead so just no-op here.
                if c == '_' {
                    continue;
                }
                let p = p + q.extend(0);
                cells.push((p, c as u32));
            }
        }
    }

    let tiled: Map = cells.into_iter().collect();
    fs::write(path.with_extension("json"), serde_json::to_string(&tiled)?)?;

    Ok(())
}

fn inject(path: &Path) -> Result<()> {
    todo!()
}

/// Return a compacted stack of maps.
///
/// The z-values in the iterator do not correspond to in-game z-values. They
/// respect above/below surface, but otherwise stack explicit maps densely and
/// ignore all generated terrain regions.
fn map_stack(
    regions: &[Region],
) -> impl Iterator<Item = (i32, &SectorMap)> + '_ {
    let num_maps = regions.iter().filter_map(|r| r.as_map()).count() as i32;
    let z = regions.iter().filter(|r| r.is_site()).count() as i32;
    let offset = z - num_maps;
    eprintln!("Top z: {z}, num-maps: {num_maps}, offset: {offset}");
    regions
        .iter()
        .rev()
        .filter_map(|r| r.as_map())
        .enumerate()
        .map(move |(i, m)| (i as i32 + offset, m))
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
