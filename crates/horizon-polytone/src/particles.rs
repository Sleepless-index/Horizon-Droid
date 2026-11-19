//! Custom particle types (`polytone/custom_particles/`, wiki "Custom Particle Types") and the
//! particle emitters blocks and entities carry (`block_modifiers`, `entity_modifiers`): what a
//! particle does is data, its numbers expressions compiled when the pack loads.
use crate::expr::Program;
use crate::target::Targets;
use serde_json::{Map, Value as Json};
use std::collections::BTreeMap;

/// An expression field: a string, or a plain number or boolean written as one.
fn program(v: &Json) -> Result<Program, String> {
    match v {
        Json::String(s) => Program::compile(s),
        Json::Number(n) => Program::compile(&n.to_string()),
        Json::Bool(b) => Program::compile(if *b { "1" } else { "0" }),
        _ => Err(format!("{v} is not an expression")),
    }
}

/// The expression fields of `object` named in `keys`, and its `customs` map.
fn expressions(
    object: &Map<String, Json>,
    keys: &[&str],
) -> Result<(BTreeMap<String, Program>, BTreeMap<String, Program>), String> {
    let mut fields = BTreeMap::new();
    for key in keys {
        if let Some(v) = object.get(*key) {
            fields.insert(
                (*key).to_owned(),
                program(v).map_err(|e| format!("{key}: {e}"))?,
            );
        }
    }
    let mut customs = BTreeMap::new();
    for (name, v) in object
        .get("customs")
        .and_then(Json::as_object)
        .into_iter()
        .flatten()
    {
        // Named variables are case insensitive.
        customs.insert(
            name.to_lowercase(),
            program(v).map_err(|e| format!("customs.{name}: {e}"))?,
        );
    }
    Ok((fields, customs))
}

/// The particle an emitter spawns: an id, or an object with that particle's own options
/// (`{"type": "minecraft:block", "block_state": ...}`).
#[derive(Clone, Debug, PartialEq)]
pub enum ParticleRef {
    Id(String),
    Options(Map<String, Json>),
}

impl ParticleRef {
    fn parse(v: &Json) -> Result<Self, String> {
        let id = |s: &str| {
            if s.contains(':') {
                s.to_owned()
            } else {
                format!("minecraft:{s}")
            }
        };
        match v {
            Json::String(s) => Ok(Self::Id(id(s))),
            Json::Object(o) if o.get("type").is_some_and(Json::is_string) => {
                Ok(Self::Options(o.clone()))
            }
            _ => Err(format!("{v} is no particle")),
        }
    }

    /// The particle type's id.
    pub fn id(&self) -> String {
        match self {
            Self::Id(id) => id.clone(),
            Self::Options(o) => {
                let t = o["type"].as_str().unwrap_or_default();
                if t.contains(':') {
                    t.to_owned()
                } else {
                    format!("minecraft:{t}")
                }
            }
        }
    }
}

/// A particle emitter: what it spawns and, as expressions, how often (`chance`, and for entity
/// emitters `count`), where (`x`, `y`, `z`) and with what motion and look.
pub struct Emitter {
    pub particle: ParticleRef,
    /// `chance`, `count`, `x`, `y`, `z`, `dx`, `dy`, `dz`, the colour (`red`... or `r`...),
    /// `roll`, `size`, `custom`: those present.
    pub fields: BTreeMap<String, Program>,
    /// Named variables seeded on the particle spawned.
    pub customs: BTreeMap<String, Program>,
    /// A rule test on the block state at the spawn position, kept as JSON.
    pub state_predicate: Option<Json>,
    /// Biome ids or tags where it spawns, kept as JSON.
    pub biomes: Option<Json>,
    /// Block emitters: where `x`, `y`, `z` count from (`lower_corner`, `center`...).
    pub spawn_location: Option<String>,
    /// Entity emitters: the model bone it sits on, and how far from the camera it still runs.
    pub bone: Option<String>,
    pub max_distance: Option<f64>,
    /// Block emitters: `animate_tick` (the default), `block_broken` or `block_cracking`.
    pub tick_source: Option<String>,
}

const EMITTER_FIELDS: [&str; 19] = [
    "chance", "count", "x", "y", "z", "dx", "dy", "dz", "red", "green", "blue", "alpha", "r", "g",
    "b", "a", "roll", "size", "custom",
];

impl Emitter {
    pub fn parse(v: &Json) -> Result<Self, String> {
        let o = v.as_object().ok_or("an emitter is not an object")?;
        let particle =
            ParticleRef::parse(o.get("particle").ok_or("an emitter without a particle")?)?;
        let (fields, customs) = expressions(o, &EMITTER_FIELDS)?;
        Ok(Self {
            particle,
            fields,
            customs,
            state_predicate: o.get("state_predicate").cloned(),
            biomes: o.get("biomes").cloned(),
            spawn_location: o
                .get("spawn_location")
                .and_then(Json::as_str)
                .map(str::to_owned),
            bone: o.get("bone").and_then(Json::as_str).map(str::to_owned),
            max_distance: o.get("max_distance").and_then(Json::as_f64),
            tick_source: o
                .get("tick_source")
                .and_then(Json::as_str)
                .map(str::to_owned),
        })
    }
}

/// A sound emitter (wiki "Sound Emission"): the sound event, and as expressions how often
/// (`chance`), where (`x`, `y`, `z`) and how (`volume`, `pitch`).
pub struct SoundEmitter {
    pub sound: String,
    /// `chance`, `x`, `y`, `z`, `volume`, `pitch`: those present.
    pub fields: BTreeMap<String, Program>,
    /// Delayed by the distance, as fireworks' far bangs.
    pub distance_delay: bool,
    pub state_predicate: Option<Json>,
    pub biomes: Option<Json>,
}

impl SoundEmitter {
    pub fn parse(v: &Json) -> Result<Self, String> {
        let o = v.as_object().ok_or("a sound emitter is not an object")?;
        let sound = o
            .get("sound")
            .and_then(Json::as_str)
            .ok_or("a sound emitter without a sound")?;
        let sound = if sound.contains(':') {
            sound.to_owned()
        } else {
            format!("minecraft:{sound}")
        };
        let (fields, _) = expressions(o, &["chance", "x", "y", "z", "volume", "pitch"])?;
        Ok(Self {
            sound,
            fields,
            distance_delay: o.get("distance_delay").and_then(Json::as_bool) == Some(true),
            state_predicate: o.get("state_predicate").cloned(),
            biomes: o.get("biomes").cloned(),
        })
    }
}

fn sound_emitters(v: Option<&Json>) -> Result<Vec<SoundEmitter>, String> {
    v.and_then(Json::as_array)
        .into_iter()
        .flatten()
        .enumerate()
        .map(|(i, e)| SoundEmitter::parse(e).map_err(|err| format!("sound emitter {i}: {err}")))
        .collect()
}

fn emitters(v: Option<&Json>) -> Result<Vec<Emitter>, String> {
    v.and_then(Json::as_array)
        .into_iter()
        .flatten()
        .enumerate()
        .map(|(i, e)| Emitter::parse(e).map_err(|err| format!("emitter {i}: {err}")))
        .collect()
}

/// `type`: a full custom particle, one copying a vanilla particle's behaviour, or a GPU one.
#[derive(Clone, Debug, PartialEq)]
pub enum ParticleKind {
    Custom,
    SemiCustom { copy_from: String },
    Gpu,
}

/// `render_type`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderType {
    Opaque,
    Translucent,
    AdditiveTranslucent,
    /// Drawn nowhere: a particle that only runs logic (its emitters, its sounds).
    Invisible,
}

/// `liquid_affinity`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LiquidAffinity {
    Any,
    Liquids,
    NonLiquids,
}

/// A custom particle type.
pub struct CustomParticle {
    pub kind: ParticleKind,
    pub render_type: RenderType,
    /// Expressions setting the particle up when it spawns (`size`, `lifetime`, colour,
    /// `roll`, `friction`, `custom`, `hitbox_size`...), and its named variables.
    pub initializer: BTreeMap<String, Program>,
    pub initializer_customs: BTreeMap<String, Program>,
    /// Expressions run each tick (`x`... `dz`, `size`, colour, `roll`, `custom`,
    /// `remove_condition`), and named variables reassigned.
    pub ticker: BTreeMap<String, Program>,
    pub ticker_customs: BTreeMap<String, Program>,
    pub emitters: Vec<Emitter>,
    pub sound_emitters: Vec<SoundEmitter>,
    pub has_physics: bool,
    pub kill_on_contact: bool,
    pub kill_when_still: bool,
    pub kill_when_not_in_view: bool,
    pub random_sprite: bool,
    pub force_spawn: bool,
    pub liquid_affinity: LiquidAffinity,
    /// The most of this type alive at once.
    pub limit: Option<u32>,
    /// Ticks between two runs of the tick logic (`tick_interval`, or `tick_rate`).
    pub tick_interval: u32,
    /// No spawn this close to another particle of the type.
    pub exclusion_radius: f64,
    pub light_level: u8,
    pub rotation_mode: String,
    /// Fields kept as JSON for the renderer: `offset`, `colormap`, `model`, a GPU particle's
    /// `texture`, `shader` and `uniforms`.
    pub render: Map<String, Json>,
    /// A GPU particle's own fields (wiki "GPU Particles"), for `"type": "gpu"`.
    pub gpu: Option<Gpu>,
}

/// What a GPU particle (`"type": "gpu"`) adds: its whole motion and look are a function of
/// the spawn record and the time, so it never ticks.
#[derive(Clone, Debug, PartialEq)]
pub struct Gpu {
    /// The texture's full path (`ns:textures/particle/....png`), a vertical strip of `frames`.
    pub texture: String,
    pub frames: u32,
    /// Downward acceleration (blocks per tick squared) and the velocity kept per tick.
    pub gravity: f64,
    pub friction: f64,
    /// Half-width at death, and colour at death (ARGB).
    pub size_end: Option<f64>,
    pub color_end: Option<u32>,
    /// Fractions of the life spent fading in and out.
    pub fade: [f64; 2],
    /// Horizontal wobble amplitude (blocks), roll speed (radians per tick), height over width.
    pub sway: f64,
    pub spin: f64,
    pub aspect: f64,
    /// One spawn drawn as `count` quads spread over a `size` box.
    pub area: Option<([f64; 3], u32)>,
    /// Quads under the world surface are hidden.
    pub kill_below_heightmap: bool,
}

impl Gpu {
    fn parse(o: &Map<String, Json>) -> Result<Self, String> {
        let num = |key: &str, default: f64| o.get(key).and_then(Json::as_f64).unwrap_or(default);
        let color_end = match o.get("color_end").and_then(Json::as_str) {
            Some(c) => Some(
                u32::from_str_radix(c.trim_start_matches('#'), 16)
                    .map_err(|_| format!("color_end {c} is no #AARRGGBB colour"))?,
            ),
            None => None,
        };
        let pair =
            |v: &Json| -> Option<Vec<f64>> { v.as_array()?.iter().map(Json::as_f64).collect() };
        let fade = o.get("fade").and_then(pair).unwrap_or_default();
        let area = match o.get("area") {
            Some(a) => {
                let size = a.get("size").and_then(pair).filter(|s| s.len() == 3);
                let size = size.ok_or("area needs a size [x, y, z]")?;
                let count = a
                    .get("count")
                    .and_then(Json::as_u64)
                    .unwrap_or(1)
                    .clamp(1, 1024) as u32;
                Some(([size[0], size[1], size[2]], count))
            }
            None => None,
        };
        Ok(Self {
            texture: o
                .get("texture")
                .and_then(Json::as_str)
                .ok_or("a GPU particle needs a texture")?
                .to_owned(),
            frames: o.get("frames").and_then(Json::as_u64).unwrap_or(1).max(1) as u32,
            gravity: num("gravity", 0.0),
            friction: num("friction", 1.0),
            size_end: o.get("size_end").and_then(Json::as_f64),
            color_end,
            fade: [
                fade.first().copied().unwrap_or(0.0),
                fade.get(1).copied().unwrap_or(0.0),
            ],
            sway: num("sway", 0.0),
            spin: num("spin", 0.0),
            aspect: num("aspect", 1.0),
            area,
            kill_below_heightmap: o.get("kill_below_heightmap").and_then(Json::as_bool)
                == Some(true),
        })
    }
}

const INITIALIZER: [&str; 13] = [
    "size",
    "hitbox_size",
    "lifetime",
    "red",
    "green",
    "blue",
    "alpha",
    "roll",
    "friction",
    "custom",
    "dx",
    "dy",
    "dz",
];
const TICKER: [&str; 14] = [
    "x",
    "y",
    "z",
    "dx",
    "dy",
    "dz",
    "size",
    "red",
    "green",
    "blue",
    "alpha",
    "roll",
    "custom",
    "remove_condition",
];

impl CustomParticle {
    pub fn parse(json: &Json) -> Result<Self, String> {
        let o = json.as_object().ok_or("a particle is not an object")?;
        let text = |key: &str| o.get(key).and_then(Json::as_str);
        let flag = |key: &str, default: bool| o.get(key).and_then(Json::as_bool).unwrap_or(default);
        let kind = match (text("type"), text("copy_from")) {
            (Some("gpu"), _) => ParticleKind::Gpu,
            (Some("semi_custom") | None, Some(from)) => ParticleKind::SemiCustom {
                copy_from: from.to_owned(),
            },
            (Some("custom") | None, _) => ParticleKind::Custom,
            (Some(other), _) => return Err(format!("unknown particle type {other}")),
        };
        let render_type = match text("render_type").unwrap_or("opaque") {
            "opaque" => RenderType::Opaque,
            "translucent" => RenderType::Translucent,
            "additive_translucent" => RenderType::AdditiveTranslucent,
            "invisible" => RenderType::Invisible,
            other => return Err(format!("unknown render_type {other}")),
        };
        let liquid_affinity = match text("liquid_affinity").unwrap_or("any") {
            "any" => LiquidAffinity::Any,
            "liquids" => LiquidAffinity::Liquids,
            "non_liquids" => LiquidAffinity::NonLiquids,
            other => return Err(format!("unknown liquid_affinity {other}")),
        };
        let empty = Map::new();
        let section = |key: &str| o.get(key).and_then(Json::as_object).unwrap_or(&empty);
        let (initializer, initializer_customs) = expressions(section("initializer"), &INITIALIZER)
            .map_err(|e| format!("initializer.{e}"))?;
        let (ticker, ticker_customs) =
            expressions(section("ticker"), &TICKER).map_err(|e| format!("ticker.{e}"))?;
        let count = |key: &str| {
            o.get(key)
                .and_then(Json::as_u64)
                .and_then(|n| u32::try_from(n).ok())
        };
        let render = [
            "offset", "colormap", "model", "texture", "shader", "uniforms", "lit",
        ]
        .into_iter()
        .filter_map(|k| Some((k.to_owned(), o.get(k)?.clone())))
        .collect();
        let gpu = match kind {
            ParticleKind::Gpu => Some(Gpu::parse(o)?),
            _ => None,
        };
        Ok(Self {
            kind,
            render_type,
            initializer,
            initializer_customs,
            ticker,
            ticker_customs,
            emitters: emitters(o.get("particle_emitters"))?,
            sound_emitters: sound_emitters(o.get("sound_emitters"))?,
            has_physics: flag("has_physics", true),
            kill_on_contact: flag("kill_on_contact", false),
            kill_when_still: flag("kill_when_still", true),
            kill_when_not_in_view: flag("kill_when_not_in_view", true),
            random_sprite: flag("random_sprite", false),
            force_spawn: flag("force_spawn", false),
            liquid_affinity,
            limit: count("limit"),
            tick_interval: count("tick_interval")
                .or(count("tick_rate"))
                .unwrap_or(1)
                .max(1),
            exclusion_radius: o
                .get("exclusion_radius")
                .and_then(Json::as_f64)
                .unwrap_or(0.0),
            light_level: count("light_level").map_or(0, |l| l.min(15) as u8),
            rotation_mode: text("rotation_mode").unwrap_or("look_at_xyz").to_owned(),
            render,
            gpu,
        })
    }
}

/// A block modifier's particle side: the blocks it targets and their emitters.
pub struct BlockEmitters {
    pub targets: Targets,
    pub priority: i32,
    pub particle_emitters: Vec<Emitter>,
    pub sound_emitters: Vec<SoundEmitter>,
    /// `disable_particles`: the block's own particles (its `Block.animateTick`, as a torch's
    /// flame) don't spawn (observed on the real mod); the emitters of block modifiers are apart.
    pub disable_particles: bool,
}

impl BlockEmitters {
    /// `None` when the file neither has emitters nor disables the block's particles (it modifies
    /// other things), or is switched off.
    pub fn parse(json: &Json, implicit: Option<&str>) -> Result<Option<Self>, String> {
        if json.get("polytone_ignore").and_then(Json::as_bool) == Some(true) {
            return Ok(None);
        }
        let particle_emitters = emitters(json.get("particle_emitters"))?;
        let sound_emitters = sound_emitters(json.get("sound_emitters"))?;
        let disable_particles = json.get("disable_particles").and_then(Json::as_bool) == Some(true);
        if particle_emitters.is_empty() && sound_emitters.is_empty() && !disable_particles {
            return Ok(None);
        }
        Ok(Some(Self {
            targets: Targets::parse(json.get("targets"), implicit)?,
            priority: json.get("priority").and_then(Json::as_i64).unwrap_or(0) as i32,
            particle_emitters,
            sound_emitters,
            disable_particles,
        }))
    }
}

/// An entity modifier: the entities it targets and the emitters on their model bones.
pub struct EntityEmitters {
    pub targets: Targets,
    pub emitters: Vec<Emitter>,
}

impl EntityEmitters {
    pub fn parse(json: &Json, implicit: Option<&str>) -> Result<Option<Self>, String> {
        if json.get("polytone_ignore").and_then(Json::as_bool) == Some(true) {
            return Ok(None);
        }
        Ok(Some(Self {
            targets: Targets::parse(json.get("targets"), implicit)?,
            emitters: emitters(json.get("emitters"))?,
        }))
    }
}

/// A particle modifier (wiki "Particle Modifiers"): expressions run once on each particle of the
/// targeted types as it spawns. The real mod reads them in the legacy dialect (`AGE`, `SIZE`);
/// `size` multiplies the particle's size and `speed` its motion, as vanilla's `Particle.scale`
/// and `setPower` do (`size` observed on the real mod, #11486).
pub struct ParticleModifier {
    pub targets: Targets,
    /// `color` (packed RGB), `red`, `green`, `blue`, `alpha`, `size`, `life`, `speed`: those
    /// present.
    pub fields: BTreeMap<String, Program>,
    /// `colormap`, kept as JSON for the runtime.
    pub colormap: Option<Json>,
    /// `filter` (a block or item the particle carries), kept as JSON.
    pub filter: Option<Json>,
}

const MODIFIER_FIELDS: [&str; 8] = [
    "color", "red", "green", "blue", "alpha", "size", "life", "speed",
];

impl ParticleModifier {
    /// `None` when the file is switched off or changes nothing.
    pub fn parse(json: &Json, implicit: Option<&str>) -> Result<Option<Self>, String> {
        if json.get("polytone_ignore").and_then(Json::as_bool) == Some(true) {
            return Ok(None);
        }
        let object = json
            .as_object()
            .ok_or("a particle modifier is not an object")?;
        let (fields, _) = expressions(object, &MODIFIER_FIELDS)?;
        let colormap = object.get("colormap").cloned();
        if fields.is_empty() && colormap.is_none() {
            return Ok(None);
        }
        Ok(Some(Self {
            targets: Targets::parse(json.get("targets"), implicit)?,
            fields,
            colormap,
            filter: object.get("filter").cloned(),
        }))
    }
}

/// A particle's state as modifiers read and write it: colour channels 0 to 1, `size` the quad
/// scale, `life` the lifetime in ticks, `xd`/`yd`/`zd` its motion.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ParticleState {
    pub age: f64,
    pub life: f64,
    pub position: [f64; 3],
    pub motion: [f64; 3],
    pub size: f64,
    pub roll: f64,
    pub rgba: [f64; 4],
    pub custom: f64,
    pub temperature: f64,
    pub downfall: f64,
    pub sky_light: f64,
    pub block_light: f64,
}

/// What a modifier reads of the world around the particle.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ParticleWorld {
    pub player: [f64; 3],
    pub player_speed_sq: f64,
    pub time: f64,
    pub day_time: f64,
    pub rain: f64,
    pub render_distance: f64,
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }

    fn gaussian(&mut self) -> f64 {
        let (a, b) = (self.next().max(f64::MIN_POSITIVE), self.next());
        (-2.0 * a.ln()).sqrt() * (std::f64::consts::TAU * b).cos()
    }
}

fn lattice(x: i64, y: i64) -> f64 {
    let mut h = (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (y as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    h ^= h >> 29;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^= h >> 32;
    (h >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
}

fn noise(x: f64, y: f64) -> f64 {
    let (x0, y0) = (x.floor(), y.floor());
    let (fx, fy) = (x - x0, y - y0);
    let smooth = |t: f64| t * t * (3.0 - 2.0 * t);
    let (u, v) = (smooth(fx), smooth(fy));
    let (xi, yi) = (x0 as i64, y0 as i64);
    let top = lattice(xi, yi) + (lattice(xi + 1, yi) - lattice(xi, yi)) * u;
    let bottom = lattice(xi, yi + 1) + (lattice(xi + 1, yi + 1) - lattice(xi, yi + 1)) * u;
    top + (bottom - top) * v
}

struct ModifierHost<'a> {
    state: &'a ParticleState,
    world: &'a ParticleWorld,
    configs: &'a crate::pack::Configs,
    rng: &'a mut Rng,
}

impl ModifierHost<'_> {
    fn packed(&self) -> f64 {
        let channel = |c: f64| (c.clamp(0.0, 1.0) * 255.0).round() as u32;
        let [r, g, b, _] = self.state.rgba;
        f64::from(channel(r) << 16 | channel(g) << 8 | channel(b))
    }

    fn position(&self, root: crate::expr::Root, axis: usize) -> f64 {
        use crate::expr::Root;
        match root {
            Root::Object => self.state.position[axis],
            _ => self.world.player[axis],
        }
    }
}

impl crate::expr::Host for ModifierHost<'_> {
    fn root(&mut self, root: crate::expr::Root) -> crate::expr::Value {
        use crate::expr::{Root, Value};
        match root {
            Root::Value => Value::Num(0.0),
            other => Value::Object(other as u32),
        }
    }

    fn method(
        &mut self,
        target: &crate::expr::Value,
        name: &str,
        args: &[crate::expr::Value],
    ) -> Result<crate::expr::Value, String> {
        use crate::expr::{Root, Value};
        let Value::Object(id) = target else {
            return Err(format!("{name} needs a context object"));
        };
        let root = [
            Root::Object,
            Root::Player,
            Root::Camera,
            Root::Global,
            Root::Random,
        ]
        .into_iter()
        .find(|r| *r as u32 == *id)
        .ok_or_else(|| format!("no object {id}"))?;
        let s = self.state;
        let number = match (root, name) {
            (Root::Object, "age") => s.age,
            (Root::Object, "life") => s.life,
            (Root::Object, "size") => s.size,
            (Root::Object, "roll") => s.roll,
            (Root::Object, "custom") => s.custom,
            (Root::Object, "xd") => s.motion[0],
            (Root::Object, "yd") => s.motion[1],
            (Root::Object, "zd") => s.motion[2],
            (Root::Object, "red") => s.rgba[0],
            (Root::Object, "green") => s.rgba[1],
            (Root::Object, "blue") => s.rgba[2],
            (Root::Object, "alpha") => s.rgba[3],
            (Root::Object, "color") => self.packed(),
            (Root::Object, "temperature") => s.temperature,
            (Root::Object, "downfall") => s.downfall,
            (Root::Object, "skyLight") => s.sky_light,
            (Root::Object, "blockLight") => s.block_light,
            (Root::Object, "distSq") => {
                let other = match args.first() {
                    Some(Value::Object(o)) if *o == Root::Object as u32 => s.position,
                    _ => self.world.player,
                };
                (0..3).map(|i| (s.position[i] - other[i]).powi(2)).sum()
            }
            (Root::Object | Root::Player | Root::Camera, "x") => self.position(root, 0),
            (Root::Object | Root::Player | Root::Camera, "y") => self.position(root, 1),
            (Root::Object | Root::Player | Root::Camera, "z") => self.position(root, 2),
            (Root::Player, "speedSq") => self.world.player_speed_sq,
            (Root::Global, "time") => self.world.time,
            (Root::Global, "dayTime") => self.world.day_time,
            (Root::Global, "rain") => self.world.rain,
            (Root::Global, "seasonNumber") => 0.0,
            (Root::Global, "renderDistance") => self.world.render_distance,
            (Root::Random, "rand") => self.rng.next(),
            (Root::Random, "gaussian") => self.rng.gaussian(),
            (Root::Random, "noise") => {
                let at = |i: usize| args.get(i).map_or(0.0, Value::num);
                noise(at(0), at(1))
            }
            _ => return Err(format!("a particle modifier has no {name}")),
        };
        Ok(Value::Num(number))
    }

    fn function(
        &mut self,
        name: &str,
        args: &[crate::expr::Value],
    ) -> Result<crate::expr::Value, String> {
        match (name, args) {
            ("config", [crate::expr::Value::Str(id)]) => self
                .configs
                .get(id)
                .map(crate::pack::ConfigValue::to_value)
                .ok_or_else(|| format!("no config {id}")),
            _ => Err(format!("a particle modifier has no function {name}")),
        }
    }
}

impl ParticleModifier {
    /// Runs the expressions on a particle that was just made: colour first (`color`, then the
    /// channels), then `size`, `life` and `speed`, each seeing what the ones before set.
    pub fn apply(
        &self,
        state: &mut ParticleState,
        world: &ParticleWorld,
        configs: &crate::pack::Configs,
        seed: u64,
    ) {
        let mut rng = Rng(seed | 1);
        for key in MODIFIER_FIELDS {
            let Some(program) = self.fields.get(key) else {
                continue;
            };
            let value = {
                let mut host = ModifierHost {
                    state,
                    world,
                    configs,
                    rng: &mut rng,
                };
                match program.eval(&mut host) {
                    Ok(v) => v.num(),
                    Err(e) => {
                        log::warn!("particle modifier {key}: {e}");
                        continue;
                    }
                }
            };
            if !value.is_finite() {
                continue;
            }
            match key {
                "color" => {
                    let packed = value as i64 as u32;
                    for (i, shift) in [16, 8, 0].into_iter().enumerate() {
                        state.rgba[i] = f64::from(packed >> shift & 255) / 255.0;
                    }
                }
                "red" => state.rgba[0] = value.clamp(0.0, 1.0),
                "green" => state.rgba[1] = value.clamp(0.0, 1.0),
                "blue" => state.rgba[2] = value.clamp(0.0, 1.0),
                "alpha" => state.rgba[3] = value.clamp(0.0, 1.0),
                "size" => state.size *= value.max(0.0),
                "life" => state.life = value.max(1.0).round(),
                _ => state.motion = state.motion.map(|m| m * value),
            }
        }
    }
}

/// Every particle modifier of a pack.
#[derive(Default)]
pub struct ParticleModifiers(Vec<ParticleModifier>, crate::pack::Configs);

impl ParticleModifiers {
    pub fn read(root: &std::path::Path) -> Self {
        let mut out = Vec::new();
        for (id, color) in crate::colors::PackColors::read(root).particles() {
            let color = match color.strip_prefix('#') {
                Some(hex) => format!("0x{hex}"),
                None => color,
            };
            let json = serde_json::json!({"targets": id, "color": color});
            match ParticleModifier::parse(&json, None) {
                Ok(Some(m)) => out.push(m),
                Ok(None) => {}
                Err(e) => log::warn!("colors.json particle.{id}: {e}"),
            }
        }
        for entry in crate::files::scan(root, "particle_modifiers") {
            let Some(json) = &entry.json else { continue };
            match ParticleModifier::parse(json, entry.id.as_deref()) {
                Ok(Some(m)) => out.push(m),
                Ok(None) => {}
                Err(e) => log::warn!("{}: {e}", entry.label),
            }
        }
        Self(out, crate::pack::Configs::read(root))
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Applies every modifier that targets the particle type `id` (`namespace:path`).
    pub fn apply(&self, id: &str, state: &mut ParticleState, world: &ParticleWorld, seed: u64) {
        for modifier in &self.0 {
            if modifier.targets.matches(id, |_| false) {
                modifier.apply(state, world, &self.1, seed);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_custom_particle_reads_its_defaults_and_expressions() {
        let p = CustomParticle::parse(&json!({
            "initializer": {"lifetime": "10+r.rand()*10", "size": 4.0, "customs": {"Phase": "1"}},
            "ticker": {"dx": "o.xd*1.01", "remove_condition": "o.age > 20"},
            "render_type": "translucent",
            "liquid_affinity": "non_liquids",
            "limit": 512,
            "tick_rate": 2
        }))
        .unwrap();
        assert_eq!(p.kind, ParticleKind::Custom);
        assert_eq!(p.render_type, RenderType::Translucent);
        assert_eq!(p.liquid_affinity, LiquidAffinity::NonLiquids);
        assert_eq!((p.limit, p.tick_interval), (Some(512), 2));
        assert!(p.has_physics && p.kill_when_still && !p.kill_on_contact);
        let keys: Vec<&str> = p.initializer.keys().map(String::as_str).collect();
        assert_eq!(keys, ["lifetime", "size"]);
        // Named variables are case insensitive.
        assert!(p.initializer_customs.contains_key("phase"));
        assert!(p.ticker.contains_key("remove_condition"));
    }

    #[test]
    fn particle_kinds_and_render_types() {
        let kind = |v: Json| CustomParticle::parse(&v).map(|p| p.kind);
        assert_eq!(
            kind(json!({"copy_from": "minecraft:flame"})).unwrap(),
            ParticleKind::SemiCustom {
                copy_from: "minecraft:flame".into()
            }
        );
        assert_eq!(
            kind(json!({"type": "gpu", "texture": "a:textures/particle/b.png"})).unwrap(),
            ParticleKind::Gpu
        );
        assert!(kind(json!({"type": "laser"})).is_err());
        assert!(CustomParticle::parse(&json!({"render_type": "neon"})).is_err());
    }

    #[test]
    fn emitters_spawn_ids_or_particles_with_options() {
        let p = CustomParticle::parse(&json!({"particle_emitters": [
            {"particle": "flame", "chance": "(o.age == 1)", "y": 2, "state_predicate": {"predicate_type": "block_match", "block": "minecraft:air"}},
            {"particle": {"type": "minecraft:block", "block_state": "minecraft:stone"}, "chance": 1}
        ]}))
        .unwrap();
        assert_eq!(
            p.emitters[0].particle,
            ParticleRef::Id("minecraft:flame".into())
        );
        assert!(
            p.emitters[0].fields.contains_key("chance") && p.emitters[0].fields.contains_key("y")
        );
        assert!(p.emitters[0].state_predicate.is_some());
        assert_eq!(p.emitters[1].particle.id(), "minecraft:block");
        // A broken expression fails the file when the pack loads.
        assert!(
            CustomParticle::parse(
                &json!({"particle_emitters": [{"particle": "flame", "chance": "1 +"}]})
            )
            .is_err()
        );
    }

    #[test]
    fn block_and_entity_modifiers_keep_their_emitters() {
        let block = BlockEmitters::parse(
            &json!({"targets": ["#minecraft:leaves"], "particle_emitters": [
                {"particle": "mypack:leaf", "chance": "0.015 + g.rain*0.015", "spawn_location": "lower_corner"}]}),
            None,
        )
        .unwrap()
        .unwrap();
        assert!(
            block
                .targets
                .matches("minecraft:oak_leaves", |t| t == "minecraft:leaves")
        );
        assert_eq!(
            block.particle_emitters[0].spawn_location.as_deref(),
            Some("lower_corner")
        );
        // A block modifier that only changes other things has no emitters.
        assert!(
            BlockEmitters::parse(&json!({"targets": "stone", "sound_type": "metal"}), None)
                .unwrap()
                .is_none()
        );
        let entity = EntityEmitters::parse(
            &json!({"targets": ["minecraft:player"], "emitters": [
                {"particle": "mypack:fog", "max_distance": 32, "chance": 0.1, "count": "30", "x": "r.rand(-25.0, 25.0)"}]}),
            None,
        )
        .unwrap()
        .unwrap();
        assert_eq!(entity.emitters[0].max_distance, Some(32.0));
        assert!(entity.emitters[0].fields.contains_key("count"));
    }

    #[test]
    fn a_gpu_particle_keeps_its_motion_and_look_fields() {
        let p = CustomParticle::parse(&serde_json::json!({
            "type": "gpu",
            "texture": "my_pack:textures/particle/rain.png",
            "frames": 4,
            "gravity": 0.1,
            "fade": [0.4, 0.2],
            "color_end": "#80FF0000",
            "area": {"size": [6, 6, 6], "count": 2},
            "kill_below_heightmap": true
        }))
        .unwrap();
        let gpu = p.gpu.unwrap();
        assert_eq!(gpu.texture, "my_pack:textures/particle/rain.png");
        assert_eq!((gpu.frames, gpu.gravity, gpu.friction), (4, 0.1, 1.0));
        assert_eq!(gpu.fade, [0.4, 0.2]);
        assert_eq!(gpu.color_end, Some(0x80FF_0000));
        assert_eq!(gpu.area, Some(([6.0, 6.0, 6.0], 2)));
        assert!(gpu.kill_below_heightmap);
        // Without a texture a GPU particle is no particle.
        assert!(CustomParticle::parse(&serde_json::json!({"type": "gpu"})).is_err());
    }

    #[test]
    fn sound_emitters_compile_their_expressions() {
        let p = CustomParticle::parse(&serde_json::json!({
            "sound_emitters": [
                {"sound": "block.bubble_column.upwards_ambient", "chance": "r.rand()*0.3",
                 "volume": "sqrt(square(o.xd)+square(o.yd)+square(o.zd))", "distance_delay": true}
            ]
        }))
        .unwrap();
        let e = &p.sound_emitters[0];
        assert_eq!(e.sound, "minecraft:block.bubble_column.upwards_ambient");
        assert_eq!(e.fields.keys().collect::<Vec<_>>(), ["chance", "volume"]);
        assert!(e.distance_delay);
        assert!(
            CustomParticle::parse(&serde_json::json!({"sound_emitters": [{"chance": "1"}]}))
                .is_err()
        );
    }

    #[test]
    fn a_particle_modifier_reads_its_expressions_and_skips_empty_files() {
        let m = ParticleModifier::parse(
            &json!({"size": "SIZE * 4", "life": "20 + rand() * 20", "colormap": "pack:cmap"}),
            Some("minecraft:flame"),
        )
        .unwrap()
        .unwrap();
        assert!(m.targets.matches("minecraft:flame", |_| false));
        let keys: Vec<&str> = m.fields.keys().map(String::as_str).collect();
        assert_eq!(keys, ["life", "size"]);
        assert_eq!(m.colormap, Some(json!("pack:cmap")));
        // Nothing to change, or switched off: no modifier.
        assert!(
            ParticleModifier::parse(&json!({"targets": ["minecraft:smoke"]}), None)
                .unwrap()
                .is_none()
        );
        assert!(
            ParticleModifier::parse(
                &json!({"size": "2", "polytone_ignore": true}),
                Some("minecraft:flame")
            )
            .unwrap()
            .is_none()
        );
    }

    #[test]
    fn a_modifier_recolours_resizes_and_slows_a_new_particle() {
        let modifier = ParticleModifier::parse(
            &json!({
                "targets": ["minecraft:smoke"],
                "color": "0xFF8000",
                "blue": "o.red() * 0.5",
                "size": 2,
                "life": "o.life() + 10",
                "speed": 0.5
            }),
            None,
        )
        .unwrap()
        .unwrap();
        let mut state = ParticleState {
            life: 20.0,
            size: 0.1,
            motion: [0.2, 0.4, -0.2],
            rgba: [0.0, 0.0, 0.0, 1.0],
            ..Default::default()
        };
        modifier.apply(
            &mut state,
            &ParticleWorld::default(),
            &Default::default(),
            7,
        );
        assert!((state.rgba[0] - 1.0).abs() < 1e-9);
        assert!((state.rgba[1] - 128.0 / 255.0).abs() < 1e-9);
        assert!((state.rgba[2] - 0.5).abs() < 1e-9);
        assert_eq!((state.size, state.life), (0.2, 30.0));
        assert_eq!(state.motion, [0.1, 0.2, -0.1]);
    }

    #[test]
    fn modifiers_only_touch_the_particles_they_target() {
        let modifiers = {
            let m = ParticleModifier::parse(
                &json!({"targets": "minecraft:flame", "alpha": 0.25}),
                None,
            )
            .unwrap()
            .unwrap();
            ParticleModifiers(vec![m], Default::default())
        };
        let fresh = || ParticleState {
            rgba: [1.0; 4],
            ..Default::default()
        };
        let (mut flame, mut smoke) = (fresh(), fresh());
        modifiers.apply("minecraft:flame", &mut flame, &ParticleWorld::default(), 1);
        modifiers.apply("minecraft:smoke", &mut smoke, &ParticleWorld::default(), 1);
        assert_eq!(flame.rgba[3], 0.25);
        assert_eq!(smoke.rgba[3], 1.0);
    }

    #[test]
    fn a_failing_expression_leaves_the_particle_unchanged() {
        let modifier = ParticleModifier::parse(
            &json!({"targets": "minecraft:smoke", "alpha": "o.nonsense()"}),
            None,
        )
        .unwrap()
        .unwrap();
        let mut state = ParticleState {
            rgba: [1.0; 4],
            ..Default::default()
        };
        modifier.apply(
            &mut state,
            &ParticleWorld::default(),
            &Default::default(),
            3,
        );
        assert_eq!(state.rgba, [1.0; 4]);
    }

    #[test]
    fn colors_json_particle_entries_become_color_modifiers() {
        let dir =
            std::env::temp_dir().join(format!("polytone-particle-colors-{}", std::process::id()));
        let polytone = dir.join("assets/minecraft/polytone");
        std::fs::create_dir_all(&polytone).unwrap();
        std::fs::write(
            polytone.join("colors.json"),
            br##"{"particle.flame": "#336699", "particle.minecraft:smoke": "0x102030"}"##,
        )
        .unwrap();
        let modifiers = ParticleModifiers::read(&dir);
        let (mut flame, mut smoke, mut other) = (
            ParticleState::default(),
            ParticleState::default(),
            ParticleState::default(),
        );
        let world = ParticleWorld::default();
        modifiers.apply("minecraft:flame", &mut flame, &world, 1);
        modifiers.apply("minecraft:smoke", &mut smoke, &world, 1);
        modifiers.apply("minecraft:cloud", &mut other, &world, 1);
        let byte = |c: f64| (c * 255.0).round() as u8;
        assert_eq!(flame.rgba.map(byte)[..3], [0x33, 0x66, 0x99]);
        assert_eq!(smoke.rgba.map(byte)[..3], [0x10, 0x20, 0x30]);
        assert_eq!(other.rgba, [0.0; 4]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn config_values_drive_size_and_colour_through_conditionals_and_text_compares() {
        let modifier = ParticleModifier::parse(
            &json!({
                "targets": "minecraft:flame",
                "size": "config('p:size')",
                "color": "config('p:on') ? 0xFF00FF : (config('p:mode') == 'green' ? 0x00FF00 : 0x00FFFF)"
            }),
            None,
        )
        .unwrap()
        .unwrap();
        let mut configs = crate::pack::Configs::default();
        configs.add_entry(
            "p:size".into(),
            &json!({"default_value": 3, "min": 1, "max": 4, "step": 1}),
        );
        configs.add_entry("p:on".into(), &json!({"default_value": false}));
        configs.add_entry(
            "p:mode".into(),
            &json!({"default_value": "green", "allowed_values": ["cyan", "green"]}),
        );
        let run = |configs: &crate::pack::Configs| {
            let mut state = ParticleState {
                size: 1.0,
                ..Default::default()
            };
            modifier.apply(&mut state, &ParticleWorld::default(), configs, 1);
            state
        };
        let state = run(&configs);
        assert_eq!(state.size, 3.0);
        assert_eq!(state.rgba[..3], [0.0, 1.0, 0.0]);
        configs.set("p:on", crate::pack::ConfigValue::Bool(true));
        assert_eq!(run(&configs).rgba[..3], [1.0, 0.0, 1.0]);
        configs.set("p:on", crate::pack::ConfigValue::Bool(false));
        configs.set("p:mode", crate::pack::ConfigValue::Text("cyan".into()));
        assert_eq!(run(&configs).rgba[..3], [0.0, 1.0, 1.0]);
    }
}
