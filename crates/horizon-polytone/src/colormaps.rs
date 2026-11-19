use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::Value as Json;

use crate::colors::parse_color;
use crate::expr::{Field, Host, Program, Root, Value};

pub const ALL_TINTS: i32 = -1;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Texture {
    width: u32,
    height: u32,
    pixels: Vec<u32>,
}

impl Texture {
    pub fn new(width: u32, height: u32, pixels: Vec<u32>) -> Option<Self> {
        let count = (width as usize).checked_mul(height as usize)?;
        (count != 0 && count == pixels.len()).then_some(Self {
            width,
            height,
            pixels,
        })
    }

    pub fn decode_png(bytes: &[u8]) -> Result<Self, String> {
        let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
        decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
        let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
        let size = reader.output_buffer_size().ok_or("colormap too large")?;
        let mut buffer = vec![0; size];
        let info = reader.next_frame(&mut buffer).map_err(|e| e.to_string())?;
        let channels = info.color_type.samples();
        let pixels = buffer[..info.buffer_size()]
            .chunks_exact(channels)
            .map(|p| {
                let (r, g, b) = if channels >= 3 {
                    (p[0], p[1], p[2])
                } else {
                    (p[0], p[0], p[0])
                };
                u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b)
            })
            .collect();
        Self::new(info.width, info.height, pixels).ok_or_else(|| "empty colormap".to_owned())
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn at(&self, x: u32, y: u32) -> u32 {
        let x = x.min(self.width - 1) as usize;
        let y = y.min(self.height - 1) as usize;
        self.pixels[y * self.width as usize + x]
    }

    pub fn center(&self) -> u32 {
        self.at(self.width / 2, self.height / 2)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SampleContext<'a> {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub temperature: f64,
    pub downfall: f64,
    pub biome: &'a str,
    pub biome_index: u32,
    pub item_damage: f64,
}

struct Sampler<'a> {
    at: &'a SampleContext<'a>,
    mapper: Option<&'a BiomeMapper>,
}

#[derive(Debug, Default)]
struct BiomeMapper {
    values: BTreeMap<String, f64>,
    size: f64,
}

impl BiomeMapper {
    fn parse(json: Option<&Json>) -> Result<Option<Self>, String> {
        let Some(json) = json.filter(|v| !v.is_null()) else {
            return Ok(None);
        };
        let object = json
            .as_object()
            .ok_or("a biome_id_mapper is an object of biome ids to numbers")?;
        let mut mapper = Self {
            values: BTreeMap::new(),
            size: 1.0,
        };
        for (key, value) in object {
            let number = value
                .as_f64()
                .ok_or_else(|| format!("biome_id_mapper {key} is a number"))?;
            if key == "texture_size" {
                if number <= 0.0 {
                    return Err("texture_size is above zero".to_owned());
                }
                mapper.size = number;
            } else {
                mapper.values.insert(normalize(key), number);
            }
        }
        Ok(Some(mapper))
    }

    fn value(&self, biome: &str) -> f64 {
        self.values.get(&normalize(biome)).copied().unwrap_or(0.0) / self.size
    }
}

fn spell_samplers(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut word = String::new();
    let flush = |word: &mut String, out: &mut String| {
        match word.as_str() {
            "temperature" | "downfall" | "biome_id" | "item_damage" | "legacy_temperature" => {
                out.push_str(&word.to_ascii_uppercase());
            }
            _ => out.push_str(word),
        }
        word.clear();
    };
    for c in source.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            word.push(c);
        } else {
            flush(&mut word, &mut out);
            out.push(c);
        }
    }
    flush(&mut word, &mut out);
    out
}

impl Sampler<'_> {
    fn read(&self, name: &str) -> Option<f64> {
        let at = self.at;
        Some(match name {
            "x" => at.x,
            "y" => at.y,
            "z" => at.z,
            "temperature" => at.temperature,
            "downfall" => at.downfall,
            "biomeId" => match self.mapper {
                Some(mapper) => mapper.value(at.biome),
                None => f64::from(at.biome_index) / 255.0,
            },
            "itemDamage" => at.item_damage,
            "legacyTemperature" => {
                if at.y > 64.0 {
                    at.temperature - (at.y - 64.0) * 0.05 / 30.0
                } else {
                    at.temperature
                }
            }
            _ => return None,
        })
    }
}

impl Host for Sampler<'_> {
    fn root(&mut self, root: Root) -> Value {
        if root == Root::Object {
            Value::Object(0)
        } else {
            Value::Null
        }
    }

    fn method(&mut self, target: &Value, name: &str, args: &[Value]) -> Result<Value, String> {
        match (target, args.is_empty(), self.read(name)) {
            (Value::Object(0), true, Some(value)) => Ok(Value::Num(value)),
            _ => Err(format!("{name} is not available in a colormap")),
        }
    }

    fn function(&mut self, name: &str, args: &[Value]) -> Result<Value, String> {
        match (name, args) {
            ("config", [Value::Str(id)]) => crate::pack::active_config(id),
            _ => Err(format!("{name} is not available in a colormap")),
        }
    }

    fn field(&mut self, root: Root, field: Field) -> Option<Value> {
        if root != Root::Object {
            return None;
        }
        let value = match field {
            Field::X => self.at.x,
            Field::Y => self.at.y,
            Field::Z => self.at.z,
            Field::Temperature => self.at.temperature,
            Field::Downfall => self.at.downfall,
            _ => return None,
        };
        Some(Value::Num(value))
    }
}

fn axis(json: Option<&Json>, default: &str) -> Result<Program, String> {
    let source = match json {
        None => default.to_owned(),
        Some(Json::String(s)) => match s.trim() {
            other => spell_samplers(other),
        },
        Some(Json::Number(n)) => n.to_string(),
        Some(_) => return Err("an axis is an expression".to_owned()),
    };
    Program::compile(&source)
}

fn flag(json: &Json, key: &str, default: bool) -> bool {
    json.get(key).and_then(Json::as_bool).unwrap_or(default)
}

fn pick(unit: f64, size: u32, rounds: bool) -> u32 {
    let scaled = (1.0 - unit) * f64::from(size - 1);
    let index = if rounds {
        scaled.round()
    } else {
        scaled.floor()
    };
    (index as u32).min(size - 1)
}

fn unit(value: f64) -> f64 {
    if value.is_nan() {
        0.0
    } else {
        value.clamp(0.0, 1.0)
    }
}

struct Channels([f64; 3]);

impl Host for Channels {
    fn root(&mut self, root: Root) -> Value {
        if root == Root::Object {
            Value::Object(0)
        } else {
            Value::Null
        }
    }

    fn method(&mut self, target: &Value, name: &str, args: &[Value]) -> Result<Value, String> {
        match (target, args.is_empty(), name) {
            (Value::Object(0), true, "red") => Ok(Value::Num(self.0[0])),
            (Value::Object(0), true, "green") => Ok(Value::Num(self.0[1])),
            (Value::Object(0), true, "blue") => Ok(Value::Num(self.0[2])),
            _ => Err(format!("{name} is not available in a colour modifier")),
        }
    }

    fn function(&mut self, name: &str, args: &[Value]) -> Result<Value, String> {
        match (name, args) {
            ("config", [Value::Str(id)]) => crate::pack::active_config(id),
            _ => Err(format!("{name} is not available in a colour modifier")),
        }
    }
}

fn modifier(json: Option<&Json>) -> Result<Option<[Option<Program>; 3]>, String> {
    let Some(json) = json.filter(|v| !v.is_null()) else {
        return Ok(None);
    };
    let object = json
        .as_object()
        .ok_or("a color_modifier is an object of red, green and blue expressions")?;
    let channel = |key: &str| -> Result<Option<Program>, String> {
        match object.get(key) {
            None => Ok(None),
            Some(Json::String(s)) => Program::compile(s).map(Some),
            Some(Json::Number(n)) => Program::compile(&n.to_string()).map(Some),
            Some(_) => Err(format!("{key} is an expression")),
        }
    };
    for key in object.keys() {
        if !matches!(key.as_str(), "red" | "green" | "blue") {
            return Err(format!("color_modifier {key} isn't read"));
        }
    }
    Ok(Some([channel("red")?, channel("green")?, channel("blue")?]))
}

fn modify(programs: &[Option<Program>; 3], color: u32) -> u32 {
    let base = [
        f64::from((color >> 16) & 0xFF),
        f64::from((color >> 8) & 0xFF),
        f64::from(color & 0xFF),
    ];
    let mut host = Channels(base);
    let mut out = 0u32;
    for (value, program) in base.iter().zip(programs) {
        let result = program
            .as_ref()
            .and_then(|p| p.eval(&mut host).ok())
            .map_or(*value, |v| v.num());
        let result = if result.is_nan() { *value } else { result };
        out = (out << 8) | result.round().clamp(0.0, 255.0) as u32;
    }
    out
}

#[derive(Debug)]
pub struct Colormap {
    per_biome: bool,
    mapper: Option<BiomeMapper>,
    modifier: Option<[Option<Program>; 3]>,
    x: Program,
    y: Program,
    triangular: bool,
    rounds: bool,
    blend: bool,
    default_color: Option<u32>,
    textures: BTreeMap<i32, Texture>,
    biome_only: bool,
}

impl Colormap {
    pub fn new(json: Option<&Json>, textures: BTreeMap<i32, Texture>) -> Result<Self, String> {
        if textures.is_empty() {
            return Err("no texture".to_owned());
        }
        let color_modifier = match json {
            Some(json) => modifier(json.get("color_modifier"))?,
            None => None,
        };
        let mapper = match json {
            Some(json) => BiomeMapper::parse(json.get("biome_id_mapper"))?,
            None => None,
        };
        let (x, y, triangular, rounds, blend, default_color) = match json {
            None => (
                axis(None, "TEMPERATURE")?,
                axis(None, "DOWNFALL")?,
                true,
                false,
                true,
                None,
            ),
            Some(json) => {
                if !json.is_object() {
                    return Err("a colormap is an object".to_owned());
                }
                let kind = match json.get("type") {
                    None | Some(Json::Null) => None,
                    Some(Json::String(s)) => Some(match s.trim() {
                        "grid" => (Some("1 - BIOME_ID"), Some("1 - POS_Y / 255")),
                        "damage" => (Some("1 - ITEM_DAMAGE"), Some("1")),
                        "fixed" => (Some("1"), Some("1")),
                        other => return Err(format!("the colormap type {other:?} isn't known")),
                    }),
                    Some(_) => return Err("a colormap type is a name".to_owned()),
                };
                let by_type =
                    |key: &str, default: &str, typed: Option<&str>| match (json.get(key), typed) {
                        (None, Some(source)) => {
                            axis(Some(&Json::String(source.to_owned())), default)
                        }
                        (given, _) => axis(given, default),
                    };
                (
                    by_type("x_axis", "TEMPERATURE", kind.and_then(|k| k.0))?,
                    by_type("y_axis", "DOWNFALL", kind.and_then(|k| k.1))?,
                    flag(json, "triangular", false),
                    flag(json, "rounds", true),
                    flag(json, "biome_blend", true),
                    json.get("default_color")
                        .and_then(parse_color)
                        .map(|c| c & 0xFF_FFFF),
                )
            }
        };
        let only_biome = |root: Root, name: &str| {
            root == Root::Object && matches!(name, "temperature" | "downfall")
        };
        let channels = |root: Root, name: &str| {
            root == Root::Object
                && matches!(name, "temperature" | "downfall" | "red" | "green" | "blue")
        };
        let per_biome_reads = |root: Root, name: &str| {
            root == Root::Object && matches!(name, "temperature" | "downfall" | "biomeId")
        };
        let modifier_ok = color_modifier
            .iter()
            .flatten()
            .flatten()
            .all(|p| p.reads_only(&channels));
        let biome_only = x.reads_only(&only_biome)
            && y.reads_only(&only_biome)
            && mapper.is_none()
            && modifier_ok;
        let per_biome =
            x.reads_only(&per_biome_reads) && y.reads_only(&per_biome_reads) && modifier_ok;
        Ok(Self {
            per_biome,
            mapper,
            modifier: color_modifier,
            x,
            y,
            triangular,
            rounds,
            blend,
            default_color,
            textures,
            biome_only,
        })
    }

    pub fn texture(&self, tint: i32) -> Option<&Texture> {
        self.textures
            .get(&tint)
            .or_else(|| self.textures.get(&ALL_TINTS))
    }

    pub fn tints(&self) -> impl Iterator<Item = i32> + '_ {
        self.textures.keys().copied()
    }

    pub fn blends(&self) -> bool {
        self.blend
    }

    pub fn depends_only_on_biome(&self) -> bool {
        self.biome_only
    }

    /// Whether the colour follows only which biome it is: its climate, name and registry index.
    pub fn depends_on_biome_alone(&self) -> bool {
        self.per_biome
    }

    pub fn sample_in_biome(
        &self,
        tint: i32,
        temperature: f64,
        downfall: f64,
        biome: &str,
        biome_index: u32,
    ) -> Option<u32> {
        if !self.per_biome {
            return None;
        }
        self.sample(
            tint,
            &SampleContext {
                temperature,
                downfall,
                biome,
                biome_index,
                ..SampleContext::default()
            },
        )
    }

    pub fn default_color(&self) -> Option<u32> {
        self.default_color
            .or_else(|| self.textures.values().next().map(Texture::center))
    }

    pub fn sample(&self, tint: i32, at: &SampleContext) -> Option<u32> {
        let texture = self.texture(tint)?;
        let mut host = Sampler {
            at,
            mapper: self.mapper.as_ref(),
        };
        let u = unit(self.x.eval(&mut host).ok()?.num());
        let mut v = unit(self.y.eval(&mut host).ok()?.num());
        if self.triangular {
            v *= u;
        }
        let color = texture.at(
            pick(u, texture.width, self.rounds),
            pick(v, texture.height, self.rounds),
        );
        Some(match &self.modifier {
            Some(programs) => modify(programs, color),
            None => color,
        })
    }

    pub fn sample_biome(&self, tint: i32, temperature: f64, downfall: f64) -> Option<u32> {
        if !self.biome_only {
            return None;
        }
        self.sample(
            tint,
            &SampleContext {
                temperature,
                downfall,
                ..SampleContext::default()
            },
        )
    }
}

#[derive(Debug, Default)]
pub struct Colormaps {
    maps: BTreeMap<String, Colormap>,
}

#[derive(Default)]
struct Files {
    json: Option<PathBuf>,
    textures: BTreeMap<i32, PathBuf>,
}

pub(crate) fn split_tint(stem: &str) -> (&str, i32) {
    if let Some((name, tail)) = stem.rsplit_once('_') {
        if !name.is_empty() {
            if let Ok(tint) = tail.parse::<i32>() {
                return (name, tint);
            }
        }
    }
    (stem, ALL_TINTS)
}

fn collect(base: &Path, namespace: &str, found: &mut BTreeMap<String, Files>) {
    let mut stack = vec![base.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let Ok(relative) = path.strip_prefix(base) else {
                continue;
            };
            let folder = relative
                .parent()
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .filter(|p| !p.is_empty())
                .map(|p| format!("{p}/"))
                .unwrap_or_default();
            match path.extension().and_then(|e| e.to_str()) {
                Some("png") => {
                    let (name, tint) = split_tint(stem);
                    found
                        .entry(format!("{namespace}:{folder}{name}"))
                        .or_default()
                        .textures
                        .insert(tint, path);
                }
                Some("json") => {
                    let id = format!("{namespace}:{folder}{stem}");
                    found.entry(id).or_default().json = Some(path);
                }
                _ => {}
            }
        }
    }
}

fn normalize(id: &str) -> String {
    let id = id.trim();
    if id.contains(':') {
        id.to_owned()
    } else {
        format!("minecraft:{id}")
    }
}

impl Colormaps {
    pub fn read(root: &Path) -> Self {
        let mut namespaces: Vec<_> = std::fs::read_dir(root.join("assets"))
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().is_dir())
            .collect();
        namespaces.sort_by_key(|e| e.file_name());
        let mut found = BTreeMap::new();
        for namespace in namespaces {
            let name = namespace.file_name().to_string_lossy().into_owned();
            collect(
                &namespace.path().join("polytone/colormaps"),
                &name,
                &mut found,
            );
        }
        let mut maps = BTreeMap::new();
        for (id, files) in found {
            if files.textures.is_empty() {
                continue;
            }
            match Self::build(&files) {
                Ok(map) => {
                    maps.insert(id, map);
                }
                Err(e) => log::warn!("colormap {id}: {e}"),
            }
        }
        Self { maps }
    }

    fn build(files: &Files) -> Result<Colormap, String> {
        let json = match &files.json {
            None => None,
            Some(path) => {
                let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
                Some(serde_json::from_str::<Json>(&text).map_err(|e| e.to_string())?)
            }
        };
        let mut textures = BTreeMap::new();
        for (tint, path) in &files.textures {
            let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
            let texture =
                Texture::decode_png(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
            textures.insert(*tint, texture);
        }
        Colormap::new(json.as_ref(), textures)
    }

    pub fn get(&self, id: &str) -> Option<&Colormap> {
        self.maps.get(&normalize(id))
    }

    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.maps.keys().map(String::as_str)
    }

    pub fn len(&self) -> usize {
        self.maps.len()
    }

    pub fn is_empty(&self) -> bool {
        self.maps.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn gradient() -> Texture {
        let pixels = (0..256u32)
            .flat_map(|y| (0..256u32).map(move |x| x << 16 | y << 8))
            .collect();
        Texture::new(256, 256, pixels).unwrap()
    }

    fn only(texture: Texture) -> BTreeMap<i32, Texture> {
        BTreeMap::from([(ALL_TINTS, texture)])
    }

    fn xy(color: u32) -> (u32, u32) {
        (color >> 16 & 0xFF, color >> 8 & 0xFF)
    }

    fn biome(map: &Colormap, temperature: f64, downfall: f64) -> (u32, u32) {
        xy(map.sample_biome(ALL_TINTS, temperature, downfall).unwrap())
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("polytone-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn png_bytes(width: u32, height: u32, color: png::ColorType, data: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut encoder = png::Encoder::new(&mut out, width, height);
        encoder.set_color(color);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(data)
            .unwrap();
        out
    }

    #[test]
    fn a_texture_needs_exactly_its_pixels() {
        assert!(Texture::new(2, 2, vec![0; 3]).is_none());
        assert!(Texture::new(0, 2, vec![]).is_none());
        assert!(Texture::new(2, 2, vec![0; 4]).is_some());
    }

    #[test]
    fn png_files_decode_to_rgb_whatever_their_colour_type() {
        let rgb = png_bytes(2, 1, png::ColorType::Rgb, &[1, 2, 3, 250, 251, 252]);
        let texture = Texture::decode_png(&rgb).unwrap();
        assert_eq!((texture.width(), texture.height()), (2, 1));
        assert_eq!(texture.at(0, 0), 0x010203);
        assert_eq!(texture.at(1, 0), 0xFAFBFC);

        let rgba = png_bytes(1, 1, png::ColorType::Rgba, &[9, 8, 7, 0]);
        assert_eq!(Texture::decode_png(&rgba).unwrap().at(0, 0), 0x090807);

        let grey = png_bytes(1, 1, png::ColorType::Grayscale, &[0x40]);
        assert_eq!(Texture::decode_png(&grey).unwrap().at(0, 0), 0x404040);

        assert!(Texture::decode_png(b"not a png").is_err());
    }

    #[test]
    fn a_texture_alone_samples_like_vanillas_grass() {
        let map = Colormap::new(None, only(gradient())).unwrap();
        for t in 0..=20 {
            for d in 0..=20 {
                let (temperature, downfall) = (f64::from(t) / 20.0, f64::from(d) / 20.0);
                let humidity = downfall * temperature;
                let x = ((1.0 - temperature) * 255.0) as u32;
                let y = ((1.0 - humidity) * 255.0) as u32;
                assert_eq!(
                    biome(&map, temperature, downfall),
                    (x, y),
                    "t={temperature} d={downfall}"
                );
            }
        }
    }

    #[test]
    fn the_corners_of_the_triangle() {
        let map = Colormap::new(None, only(gradient())).unwrap();
        assert_eq!(biome(&map, 1.0, 1.0), (0, 0));
        assert_eq!(biome(&map, 1.0, 0.0), (0, 255));
        assert_eq!(biome(&map, 0.0, 1.0), (255, 255));
        assert_eq!(biome(&map, 0.0, 0.0), (255, 255));
    }

    #[test]
    fn a_json_colormap_rounds_to_the_nearest_pixel_unless_told_not_to() {
        let rounded = Colormap::new(Some(&json!({})), only(gradient())).unwrap();
        assert_eq!(biome(&rounded, 0.5, 1.0), (128, 0));
        let floored = Colormap::new(Some(&json!({"rounds": false})), only(gradient())).unwrap();
        assert_eq!(biome(&floored, 0.5, 1.0), (127, 0));
    }

    #[test]
    fn triangular_samples_downfall_scaled_by_temperature() {
        let flat = Colormap::new(Some(&json!({})), only(gradient())).unwrap();
        let triangular =
            Colormap::new(Some(&json!({"triangular": true})), only(gradient())).unwrap();
        assert_eq!(biome(&flat, 0.5, 1.0), (128, 0));
        assert_eq!(biome(&triangular, 0.5, 1.0), (128, 128));
    }

    #[test]
    fn samplers_outside_zero_to_one_are_clamped() {
        let map = Colormap::new(Some(&json!({})), only(gradient())).unwrap();
        assert_eq!(biome(&map, 2.0, -3.0), (0, 255));
        assert_eq!(biome(&map, -1.0, 9.0), (255, 0));
        assert_eq!(biome(&map, f64::NAN, f64::NAN), (255, 255));
    }

    #[test]
    fn axes_are_expressions_over_the_position() {
        let map = Colormap::new(
            Some(&json!({"x_axis": "POS_X / 256", "y_axis": 0})),
            only(gradient()),
        )
        .unwrap();
        assert!(!map.depends_only_on_biome());
        assert!(map.sample_biome(ALL_TINTS, 0.5, 0.5).is_none());
        let at = SampleContext {
            x: 64.0,
            ..SampleContext::default()
        };
        assert_eq!(xy(map.sample(ALL_TINTS, &at).unwrap()), (191, 255));
        let scripted = Colormap::new(
            Some(&json!({"x_axis": "o.getX() / 256", "y_axis": "o.getZ() / 256"})),
            only(gradient()),
        )
        .unwrap();
        let at = SampleContext {
            x: 0.0,
            z: 256.0,
            ..SampleContext::default()
        };
        assert_eq!(xy(scripted.sample(ALL_TINTS, &at).unwrap()), (255, 0));
    }

    #[test]
    fn the_named_samplers_are_the_biome_variables() {
        let named = Colormap::new(
            Some(&json!({"x_axis": "temperature", "y_axis": "downfall"})),
            only(gradient()),
        )
        .unwrap();
        let bare = Colormap::new(Some(&json!({})), only(gradient())).unwrap();
        assert!(named.depends_only_on_biome());
        assert!(bare.depends_only_on_biome());
        assert_eq!(biome(&named, 0.3, 0.9), biome(&bare, 0.3, 0.9));
        let swapped = Colormap::new(
            Some(&json!({"x_axis": "DOWNFALL", "y_axis": "TEMPERATURE"})),
            only(gradient()),
        )
        .unwrap();
        assert_eq!(biome(&swapped, 0.0, 1.0), (0, 255));
    }

    #[test]
    fn tints_fall_back_to_the_all_tints_texture() {
        let tinted = |value: u32| Texture::new(1, 1, vec![value]).unwrap();
        let both = Colormap::new(
            Some(&json!({})),
            BTreeMap::from([(0, tinted(0xAA0000)), (1, tinted(0x00BB00))]),
        )
        .unwrap();
        assert_eq!(both.sample_biome(0, 0.5, 0.5), Some(0xAA0000));
        assert_eq!(both.sample_biome(1, 0.5, 0.5), Some(0x00BB00));
        assert_eq!(both.sample_biome(2, 0.5, 0.5), None);
        let shared = Colormap::new(
            Some(&json!({})),
            BTreeMap::from([(ALL_TINTS, tinted(0x123456)), (1, tinted(0x00BB00))]),
        )
        .unwrap();
        assert_eq!(shared.sample_biome(1, 0.5, 0.5), Some(0x00BB00));
        assert_eq!(shared.sample_biome(7, 0.5, 0.5), Some(0x123456));
        assert_eq!(shared.tints().collect::<Vec<_>>(), vec![-1, 1]);
    }

    #[test]
    fn the_default_colour_is_set_or_the_texture_centre() {
        let set =
            Colormap::new(Some(&json!({"default_color": "#ff00ff"})), only(gradient())).unwrap();
        assert_eq!(set.default_color(), Some(0xFF00FF));
        let centre = Colormap::new(Some(&json!({})), only(gradient())).unwrap();
        assert_eq!(xy(centre.default_color().unwrap()), (128, 128));
    }

    #[test]
    fn a_color_modifier_rewrites_the_sampled_channels() {
        let map = Colormap::new(
            Some(&json!({"color_modifier": {"red": "RED * 0.5", "blue": "255 - GREEN"}})),
            only(gradient()),
        )
        .unwrap();
        let plain = Colormap::new(Some(&json!({})), only(gradient())).unwrap();
        let sampled = plain.sample_biome(ALL_TINTS, 0.5, 0.5).unwrap();
        let (red, green) = ((sampled >> 16) & 0xFF, (sampled >> 8) & 0xFF);
        let modified = map.sample_biome(ALL_TINTS, 0.5, 0.5).unwrap();
        assert_eq!((modified >> 16) & 0xFF, (red as f64 * 0.5).round() as u32);
        assert_eq!((modified >> 8) & 0xFF, green);
        assert_eq!(modified & 0xFF, 255 - green);
        assert!(map.depends_only_on_biome());
    }

    #[test]
    fn the_legacy_temperature_sampler_cools_with_height() {
        let legacy = Colormap::new(
            Some(&json!({"x_axis": "legacy_temperature"})),
            only(gradient()),
        )
        .unwrap();
        let plain = Colormap::new(Some(&json!({})), only(gradient())).unwrap();
        let at = |y: f64| SampleContext {
            y,
            temperature: 0.8,
            downfall: 0.4,
            ..Default::default()
        };
        assert_eq!(
            legacy.sample(ALL_TINTS, &at(64.0)),
            plain.sample(ALL_TINTS, &at(64.0))
        );
        assert_ne!(
            legacy.sample(ALL_TINTS, &at(250.0)),
            plain.sample(ALL_TINTS, &at(250.0))
        );
        assert!(!legacy.depends_only_on_biome());
    }

    #[test]
    fn the_biome_id_sampler_reads_the_registry_index_or_a_mapper() {
        let by_index = Colormap::new(
            Some(&json!({"x_axis": "biome_id", "y_axis": 0})),
            only(gradient()),
        )
        .unwrap();
        let at = |index: u32, biome: &'static str| SampleContext {
            biome,
            biome_index: index,
            ..Default::default()
        };
        assert_ne!(
            by_index.sample(ALL_TINTS, &at(10, "minecraft:plains")),
            by_index.sample(ALL_TINTS, &at(200, "minecraft:plains"))
        );
        let mapped = Colormap::new(
            Some(&json!({
                "x_axis": "biome_id",
                "y_axis": 0,
                "biome_id_mapper": {"plains": 0, "minecraft:desert": 255, "texture_size": 255}
            })),
            only(gradient()),
        )
        .unwrap();
        assert_eq!(
            mapped.sample(ALL_TINTS, &at(99, "minecraft:plains")),
            mapped.sample(ALL_TINTS, &at(1, "minecraft:plains"))
        );
        assert_ne!(
            mapped.sample(ALL_TINTS, &at(99, "minecraft:plains")),
            mapped.sample(ALL_TINTS, &at(99, "minecraft:desert"))
        );
        assert!(!mapped.depends_only_on_biome());
    }

    #[test]
    fn config_reads_the_active_packs_entries_in_axes_and_modifiers() {
        let configs = crate::pack::Configs::from_files([(
            "assets/zz/polytone/config_entries/shift.json".to_owned(),
            br#"{"default_value": 0.25}"#.to_vec(),
        )]);
        crate::pack::set_active(configs);
        let shifted = Colormap::new(
            Some(&json!({"x_axis": "config('zz:shift')", "y_axis": 0})),
            only(gradient()),
        )
        .unwrap();
        let fixed = Colormap::new(
            Some(&json!({"x_axis": "0.25", "y_axis": 0})),
            only(gradient()),
        )
        .unwrap();
        let at = SampleContext::default();
        assert_eq!(shifted.sample(ALL_TINTS, &at), fixed.sample(ALL_TINTS, &at));
        let modified = Colormap::new(
            Some(&json!({"y_axis": 0, "color_modifier": {"red": "config('zz:shift') * 400"}})),
            only(gradient()),
        )
        .unwrap();
        assert_eq!((modified.sample(ALL_TINTS, &at).unwrap() >> 16) & 0xFF, 100);
        let missing = Colormap::new(
            Some(&json!({"x_axis": "config('zz:none')"})),
            only(gradient()),
        )
        .unwrap();
        assert_eq!(missing.sample(ALL_TINTS, &at), None);
    }

    #[test]
    fn the_named_colormap_types_pick_their_axes() {
        let damage = Colormap::new(Some(&json!({"type": "damage"})), only(gradient())).unwrap();
        let at = |damage: f64| SampleContext {
            item_damage: damage,
            ..Default::default()
        };
        assert_ne!(
            damage.sample(ALL_TINTS, &at(0.0)),
            damage.sample(ALL_TINTS, &at(1.0))
        );
        let fixed = Colormap::new(Some(&json!({"type": "fixed"})), only(gradient())).unwrap();
        assert_eq!(
            fixed.sample(ALL_TINTS, &at(0.0)),
            fixed.sample(ALL_TINTS, &at(1.0))
        );
        let grid = Colormap::new(Some(&json!({"type": "grid"})), only(gradient())).unwrap();
        let row = |y: f64| SampleContext {
            y,
            ..Default::default()
        };
        assert_ne!(
            grid.sample(ALL_TINTS, &row(0.0)),
            grid.sample(ALL_TINTS, &row(200.0))
        );
        assert!(Colormap::new(Some(&json!({"type": "spiral"})), only(gradient())).is_err());
    }

    #[test]
    fn biome_blend_is_on_unless_switched_off() {
        let on = Colormap::new(Some(&json!({})), only(gradient())).unwrap();
        let off = Colormap::new(Some(&json!({"biome_blend": false})), only(gradient())).unwrap();
        assert!(on.blends());
        assert!(!off.blends());
    }

    #[test]
    fn what_cannot_be_read_faithfully_is_refused() {
        let refused = |json: Json| Colormap::new(Some(&json), only(gradient())).is_err();
        assert!(refused(json!({"color_modifier": {"hue": "1"}})));
        assert!(refused(json!({"color_modifier": {"red": "RED *"}})));
        assert!(refused(json!({"x_axis": "POS_X +"})));
        assert!(refused(json!({"x_axis": "BIOME"})));
        assert!(refused(json!({"x_axis": ["a"]})));
        assert!(refused(json!([1])));
        assert!(Colormap::new(None, BTreeMap::new()).is_err());
    }

    #[test]
    fn tints_are_read_from_the_end_of_a_file_name() {
        assert_eq!(split_tint("lush_0"), ("lush", 0));
        assert_eq!(split_tint("lush_12"), ("lush", 12));
        assert_eq!(split_tint("lush_-1"), ("lush", -1));
        assert_eq!(split_tint("lush"), ("lush", ALL_TINTS));
        assert_eq!(split_tint("grass_color"), ("grass_color", ALL_TINTS));
        assert_eq!(split_tint("_3"), ("_3", ALL_TINTS));
    }

    #[test]
    fn a_pack_folder_gives_each_colormap_its_id_textures_and_settings() {
        let root = scratch("read");
        let base = root.join("assets/foo/polytone/colormaps");
        std::fs::create_dir_all(base.join("nested")).unwrap();
        let pixel = |r: u8, g: u8, b: u8| png_bytes(1, 1, png::ColorType::Rgb, &[r, g, b]);
        std::fs::write(base.join("lush_0.png"), pixel(1, 0, 0)).unwrap();
        std::fs::write(base.join("lush_1.png"), pixel(0, 2, 0)).unwrap();
        std::fs::write(base.join("lush.json"), r#"{"biome_blend": false}"#).unwrap();
        std::fs::write(base.join("plain.png"), pixel(0, 0, 3)).unwrap();
        std::fs::write(base.join("nested/deep.png"), pixel(4, 4, 4)).unwrap();
        std::fs::write(base.join("orphan.json"), "{}").unwrap();
        std::fs::write(base.join("bad.png"), b"nope").unwrap();
        std::fs::write(base.join("refused.png"), pixel(5, 5, 5)).unwrap();
        std::fs::write(
            base.join("refused.json"),
            r#"{"color_modifier": {"hue": "1"}}"#,
        )
        .unwrap();

        let maps = Colormaps::read(&root);
        let ids: Vec<_> = maps.ids().collect();
        assert_eq!(ids, ["foo:lush", "foo:nested/deep", "foo:plain"]);
        let lush = maps.get("foo:lush").unwrap();
        assert!(!lush.blends());
        assert_eq!(lush.sample_biome(0, 0.5, 0.5), Some(0x010000));
        assert_eq!(lush.sample_biome(1, 0.5, 0.5), Some(0x000200));
        assert_eq!(
            maps.get("foo:plain").unwrap().sample_biome(9, 0.2, 0.9),
            Some(3)
        );
        assert_eq!(
            maps.get("foo:nested/deep").unwrap().default_color(),
            Some(0x040404)
        );
        assert!(maps.get("lush").is_none());
        assert!(maps.get("foo:orphan").is_none());
        assert!(maps.get("foo:refused").is_none());
        assert_eq!(maps.len(), 3);
        assert!(!maps.is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn an_id_without_a_namespace_is_minecrafts() {
        let root = scratch("namespace");
        let base = root.join("assets/minecraft/polytone/colormaps");
        std::fs::create_dir_all(&base).unwrap();
        std::fs::write(
            base.join("sky.png"),
            png_bytes(1, 1, png::ColorType::Rgb, &[7, 7, 7]),
        )
        .unwrap();
        let maps = Colormaps::read(&root);
        assert!(maps.get("sky").is_some());
        assert!(maps.get("minecraft:sky").is_some());
        assert!(maps.get("other:sky").is_none());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_folder_without_packs_reads_nothing() {
        let root = scratch("empty");
        assert!(Colormaps::read(&root).is_empty());
        assert!(Colormaps::read(&root.join("missing")).is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }
}
