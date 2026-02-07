use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tui_map::core::TileKind;
use tui_map::parse::{parse_char_grid, Legend, ParseOptions, TrimMode};

use crate::state::{ItemKind, MapState};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ScenarioRuntime {
    pub manifest: ScenarioManifest,
    pub map: MapState,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ScenarioManifest {
    pub id: String,
    pub name: String,
    pub map_path: String,
    #[serde(default)]
    pub starters: Vec<String>,
    #[serde(default)]
    pub wild_pool: Vec<String>,
    #[serde(default)]
    pub events: Vec<ScenarioEvent>,
    #[serde(default)]
    pub random_pickups: RandomPickupSpec,
    #[serde(default)]
    pub abilities: Vec<AbilitySpec>,
    #[serde(default)]
    pub species_abilities: Vec<SpeciesAbility>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ScenarioEvent {
    pub id: String,
    pub trigger: ScenarioTrigger,
    pub message: String,
    #[serde(default = "default_true")]
    pub once: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ScenarioTrigger {
    OnEnterTile { x: u16, y: u16 },
    OnDefeat { species: String, count: u16 },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RandomPickupSpec {
    pub count: u16,
    #[serde(default)]
    pub pool: Vec<ItemDrop>,
}

impl Default for RandomPickupSpec {
    fn default() -> Self {
        Self {
            count: 0,
            pool: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ItemDrop {
    pub kind: ItemKind,
    pub weight: u16,
    pub qty: u16,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AbilitySpec {
    pub id: String,
    pub name: String,
    pub cooldown: u8,
    pub effect: AbilityEffect,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AbilityEffect {
    Damage { power: u16 },
    Heal { amount: u16 },
    Guard { reduction_pct: u8, turns: u8 },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SpeciesAbility {
    pub species: String,
    pub ability_id: String,
}

pub async fn load_scenario(path: &Path) -> Result<ScenarioRuntime, String> {
    let manifest_path = path.join("manifest.ron");
    let manifest_str = tokio::fs::read_to_string(&manifest_path)
        .await
        .map_err(|e| format!("Failed to read {}: {}", manifest_path.display(), e))?;
    let manifest: ScenarioManifest =
        ron::de::from_str(&manifest_str).map_err(|e| format!("Failed to parse manifest: {}", e))?;
    let map_path = path.join(&manifest.map_path);
    let map_str = tokio::fs::read_to_string(&map_path)
        .await
        .map_err(|e| format!("Failed to read {}: {}", map_path.display(), e))?;
    let legend = Legend::builder()
        .entry('g', TileKind::Grass)
        .entry('G', TileKind::Grass)
        .entry('r', TileKind::Trail)
        .entry('R', TileKind::Trail)
        .entry('p', TileKind::Trail)
        .entry('P', TileKind::Trail)
        .entry('s', TileKind::Sand)
        .entry('S', TileKind::Sand)
        .entry('w', TileKind::Water)
        .entry('W', TileKind::Water)
        .entry('#', TileKind::Wall)
        .entry('x', TileKind::Wall)
        .entry('X', TileKind::Wall)
        .build()
        .map_err(|e| format!("Failed to build map legend: {}", e))?;
    let grid = parse_char_grid(
        &manifest.name,
        &map_str,
        &legend,
        &ParseOptions {
            trim_mode: TrimMode::TrimBoth,
            default_char: 'g',
            default_tile: TileKind::Grass,
        },
    )
    .map_err(|e| format!("Failed to parse map: {}", e))?;
    let map = MapState::from_grid(grid);
    Ok(ScenarioRuntime { manifest, map })
}

fn default_true() -> bool {
    true
}
