use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::{HashSet};
use std::time::{SystemTime, UNIX_EPOCH};
use tui_dispatch_debug::debug::{DebugSection, DebugState};

use crate::llm::Provider;
use crate::rules::{Ability, AbilityScores};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum GameMode {
    CharacterCreation,
    Exploration,
    Dialogue,
    CustomAction,
    Inventory,
    Combat,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum Tile {
    Grass,
    Road,
    Floor,
    Wall,
    Water,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MapState {
    pub name: String,
    pub width: u16,
    pub height: u16,
    pub tiles: Vec<Tile>,
}

impl MapState {
    pub fn tile(&self, x: u16, y: u16) -> Tile {
        if x >= self.width || y >= self.height {
            return Tile::Wall;
        }
        let idx = self.index(x, y);
        self.tiles.get(idx).copied().unwrap_or(Tile::Wall)
    }

    pub fn is_walkable(&self, x: u16, y: u16) -> bool {
        matches!(self.tile(x, y), Tile::Grass | Tile::Road | Tile::Floor)
    }

    fn index(&self, x: u16, y: u16) -> usize {
        (y as usize * self.width as usize) + x as usize
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct NpcState {
    pub id: String,
    pub name: String,
    pub x: u16,
    pub y: u16,
    pub persona: String,
    pub dialogue_prompt: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ItemState {
    pub id: String,
    pub name: String,
    pub x: u16,
    pub y: u16,
    pub qty: u16,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EncounterState {
    pub id: String,
    pub name: String,
    pub x: u16,
    pub y: u16,
    pub hp: i32,
    pub atk: i32,
    #[serde(default)]
    pub defeated: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Trigger {
    OnEnter { x: u16, y: u16, message: String },
    OnInteract { x: u16, y: u16, message: String },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PlayerState {
    pub name: String,
    pub class_name: String,
    pub background: String,
    pub x: u16,
    pub y: u16,
    pub hp: i32,
    pub max_hp: i32,
    pub stats: AbilityScores,
    #[serde(default)]
    pub inventory: Vec<ItemStack>,
}

impl Default for PlayerState {
    fn default() -> Self {
        Self {
            name: String::new(),
            class_name: String::new(),
            background: String::new(),
            x: 0,
            y: 0,
            hp: 10,
            max_hp: 10,
            stats: AbilityScores::default(),
            inventory: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ItemStack {
    pub id: String,
    pub name: String,
    pub qty: u16,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DialogueState {
    pub active_npc: Option<String>,
    pub input: String,
    #[serde(default)]
    pub history: Vec<DialogueLine>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DialogueLine {
    pub speaker: String,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CustomActionState {
    pub input: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CombatState {
    pub enemy_id: String,
    pub player_turn: bool,
    pub movement_left: u8,
    pub round: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CharacterCreationState {
    pub step: CreationStep,
    pub name: String,
    pub class_index: usize,
    pub background_index: usize,
    pub stats: AbilityScores,
    pub selected_stat: usize,
    pub points_remaining: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum CreationStep {
    Name,
    Class,
    Background,
    Stats,
    Confirm,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct LogEntry {
    pub speaker: LogSpeaker,
    pub text: String,
    pub timestamp: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum LogSpeaker {
    System,
    Player,
    Npc,
    Combat,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ScenarioManifestSummary {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub lore: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub enum PendingLlm {
    Dialogue { npc_id: String },
    CustomAction,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AppState {
    pub terminal_size: (u16, u16),
    pub mode: GameMode,
    pub map: MapState,
    pub player: PlayerState,
    pub npcs: Vec<NpcState>,
    pub items: Vec<ItemState>,
    pub encounters: Vec<EncounterState>,
    pub triggers: Vec<Trigger>,
    pub fired_triggers: HashSet<String>,
    pub dialogue: DialogueState,
    pub custom_action: CustomActionState,
    pub combat: Option<CombatState>,
    pub creation: CharacterCreationState,
    pub log: Vec<LogEntry>,
    pub log_scroll: u16,
    pub scenario: Option<ScenarioManifestSummary>,
    pub pending_llm: Option<PendingLlm>,
    #[serde(default)]
    pub spinner_frame: u8,
    pub transcript_index: usize,
    pub pending_transcript_index: Option<usize>,
    pub rng_seed: u64,
    pub scenario_dir: String,
    pub save_path: String,
    pub provider: Provider,
    pub model: String,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new(
            "assets/scenarios/starter".to_string(),
            "save.json".to_string(),
            Provider::Openai,
            "gpt-4o-mini".to_string(),
        )
    }
}

impl AppState {
    pub fn new(scenario_dir: String, save_path: String, provider: Provider, model: String) -> Self {
        Self {
            terminal_size: (80, 24),
            mode: GameMode::CharacterCreation,
            map: MapState {
                name: "Loading...".to_string(),
                width: 1,
                height: 1,
                tiles: vec![Tile::Floor],
            },
            player: PlayerState::default(),
            npcs: Vec::new(),
            items: Vec::new(),
            encounters: Vec::new(),
            triggers: Vec::new(),
            fired_triggers: HashSet::new(),
            dialogue: DialogueState {
                active_npc: None,
                input: String::new(),
                history: Vec::new(),
            },
            custom_action: CustomActionState { input: String::new() },
            combat: None,
            creation: CharacterCreationState {
                step: CreationStep::Name,
                name: String::new(),
                class_index: 0,
                background_index: 0,
                stats: AbilityScores::default(),
                selected_stat: 0,
                points_remaining: 27,
            },
            log: Vec::new(),
            log_scroll: 0,
            scenario: None,
            pending_llm: None,
            spinner_frame: 0,
            transcript_index: 0,
            pending_transcript_index: None,
            rng_seed: seed_from_time(),
            scenario_dir,
            save_path,
            provider,
            model,
        }
    }

    pub fn push_log(&mut self, speaker: LogSpeaker, text: impl Into<String>) {
        self.log.push(LogEntry {
            speaker,
            text: text.into(),
            timestamp: current_timestamp(),
        });
    }

    pub fn npc_by_id(&self, id: &str) -> Option<&NpcState> {
        self.npcs.iter().find(|n| n.id == id)
    }

    pub fn npc_by_id_mut(&mut self, id: &str) -> Option<&mut NpcState> {
        self.npcs.iter_mut().find(|n| n.id == id)
    }

    pub fn encounter_by_id_mut(&mut self, id: &str) -> Option<&mut EncounterState> {
        self.encounters.iter_mut().find(|e| e.id == id)
    }

    pub fn player_pos(&self) -> (u16, u16) {
        (self.player.x, self.player.y)
    }

    pub fn set_player_pos(&mut self, x: u16, y: u16) {
        self.player.x = x;
        self.player.y = y;
    }

    pub fn ability_score(&self, ability: Ability) -> i32 {
        self.player.stats.get(ability)
    }
}

impl DebugState for AppState {
    fn debug_sections(&self) -> Vec<DebugSection> {
        vec![
            DebugSection::new("Mode")
                .entry("mode", format!("{:?}", self.mode))
                .entry("pending_llm", format!("{:?}", self.pending_llm)),
            DebugSection::new("Player")
                .entry("name", self.player.name.clone())
                .entry("class", self.player.class_name.clone())
                .entry("background", self.player.background.clone())
                .entry("pos", format!("{},{}", self.player.x, self.player.y))
                .entry("hp", format!("{}/{}", self.player.hp, self.player.max_hp)),
            DebugSection::new("Scenario")
                .entry("map", self.map.name.clone())
                .entry("npcs", self.npcs.len().to_string())
                .entry("items", self.items.len().to_string())
                .entry("encounters", self.encounters.len().to_string()),
            DebugSection::new("Log")
                .entry("entries", self.log.len().to_string())
                .entry("scroll", self.log_scroll.to_string())
                .entry("transcript_index", self.transcript_index.to_string()),
        ]
    }
}

fn seed_from_time() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(1)
}

fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
