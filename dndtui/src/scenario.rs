use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tui_map::core::TileKind;
use tui_map::parse::{parse_char_grid, Legend, ParseOptions, TrimMode};

use crate::state::{EncounterState, ItemState, MapState, NpcState, Trigger};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ScenarioRuntime {
    pub manifest: ScenarioManifest,
    pub map: MapState,
    pub npcs: Vec<NpcState>,
    pub items: Vec<ItemState>,
    pub encounters: Vec<EncounterState>,
    pub triggers: Vec<Trigger>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ScenarioManifest {
    pub id: String,
    pub name: String,
    pub map_path: String,
    pub legend: Vec<LegendEntry>,
    pub player_start: Point,
    #[serde(default)]
    pub npcs: Vec<NpcSpec>,
    #[serde(default)]
    pub items: Vec<ItemSpec>,
    #[serde(default)]
    pub encounters: Vec<EncounterSpec>,
    #[serde(default)]
    pub triggers: Vec<TriggerSpec>,
    #[serde(default)]
    pub lore: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct LegendEntry {
    pub ch: String,
    pub tile: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Point {
    pub x: u16,
    pub y: u16,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct NpcSpec {
    pub id: String,
    pub name: String,
    pub x: u16,
    pub y: u16,
    pub persona: String,
    pub dialogue_prompt: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ItemSpec {
    pub id: String,
    pub name: String,
    pub x: u16,
    pub y: u16,
    pub qty: u16,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EncounterSpec {
    pub id: String,
    pub name: String,
    pub x: u16,
    pub y: u16,
    pub hp: i32,
    pub atk: i32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TriggerSpec {
    OnEnter { x: u16, y: u16, message: String },
    OnInteract { x: u16, y: u16, message: String },
}

pub async fn load_scenario(path: &Path) -> Result<ScenarioRuntime, String> {
    let manifest_path = path.join("manifest.yaml");
    let manifest_str = tokio::fs::read_to_string(&manifest_path)
        .await
        .map_err(|e| format!("Failed to read {}: {}", manifest_path.display(), e))?;
    let manifest: ScenarioManifest = serde_yaml::from_str(&manifest_str)
        .map_err(|e| format!("Failed to parse manifest: {}", e))?;

    let map_path = path.join(&manifest.map_path);
    let map_str = tokio::fs::read_to_string(&map_path)
        .await
        .map_err(|e| format!("Failed to read {}: {}", map_path.display(), e))?;
    let map = parse_map(&manifest, &map_str)?;

    let npcs = manifest
        .npcs
        .iter()
        .map(|spec| NpcState {
            id: spec.id.clone(),
            name: spec.name.clone(),
            x: spec.x,
            y: spec.y,
            persona: spec.persona.clone(),
            dialogue_prompt: spec.dialogue_prompt.clone(),
        })
        .collect();

    let items = manifest
        .items
        .iter()
        .map(|spec| ItemState {
            id: spec.id.clone(),
            name: spec.name.clone(),
            x: spec.x,
            y: spec.y,
            qty: spec.qty,
        })
        .collect();

    let encounters = manifest
        .encounters
        .iter()
        .map(|spec| EncounterState {
            id: spec.id.clone(),
            name: spec.name.clone(),
            x: spec.x,
            y: spec.y,
            hp: spec.hp,
            atk: spec.atk,
            defeated: false,
        })
        .collect();

    let triggers = manifest
        .triggers
        .iter()
        .map(|spec| match spec {
            TriggerSpec::OnEnter { x, y, message } => Trigger::OnEnter {
                x: *x,
                y: *y,
                message: message.clone(),
            },
            TriggerSpec::OnInteract { x, y, message } => Trigger::OnInteract {
                x: *x,
                y: *y,
                message: message.clone(),
            },
        })
        .collect();

    Ok(ScenarioRuntime {
        manifest,
        map,
        npcs,
        items,
        encounters,
        triggers,
    })
}

fn parse_map(manifest: &ScenarioManifest, map_str: &str) -> Result<MapState, String> {
    let legend = build_legend(&manifest.legend)?;
    let grid = parse_char_grid(
        &manifest.name,
        map_str,
        &legend,
        &ParseOptions {
            trim_mode: TrimMode::PreserveRightWhitespace,
            default_char: ' ',
            default_tile: TileKind::Floor,
        },
    )
    .map_err(|e| format!("Failed to parse map: {}", e))?;

    Ok(MapState::from_grid(grid))
}

fn build_legend(entries: &[LegendEntry]) -> Result<Legend, String> {
    let mut legend = Legend::builder();
    for entry in entries {
        let ch = entry
            .ch
            .chars()
            .next()
            .ok_or_else(|| "Legend entry missing character".to_string())?;
        let tile =
            tile_from_name(&entry.tile).ok_or_else(|| format!("Unknown tile: {}", entry.tile))?;
        legend = legend.entry(ch, tile);
    }
    legend
        .build()
        .map_err(|e| format!("Failed to build map legend: {}", e))
}

fn tile_from_name(name: &str) -> Option<TileKind> {
    match name.to_lowercase().as_str() {
        "grass" => Some(TileKind::Grass),
        "road" => Some(TileKind::Trail),
        "floor" => Some(TileKind::Floor),
        "wall" => Some(TileKind::Wall),
        "water" => Some(TileKind::Water),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legend_parsing() {
        let entries = vec![LegendEntry {
            ch: "#".to_string(),
            tile: "wall".to_string(),
        }];
        let legend = build_legend(&entries).expect("legend");
        assert_eq!(legend.tile_for('#'), Some(TileKind::Wall));
    }
}
