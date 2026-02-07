use tui_dispatch::DispatchResult;

use crate::action::Action;
use crate::effect::Effect;
use crate::llm::prompt;
use crate::llm::schema::ActionInterpretation;
use crate::rules::{
    ability_modifier, class_base_hp, clamp_score, difficulty_dc, parse_difficulty,
    parse_skill_or_ability, points_remaining, roll_d20, roll_damage, skill_to_ability, Ability,
    CheckKind, BACKGROUND_OPTIONS, CLASS_OPTIONS,
};
use crate::scenario::ScenarioRuntime;
use crate::state::{AppState, CombatState, Direction, LogSpeaker, PendingLlm, Trigger};

const MOVEMENT_PER_TURN: u8 = 4;

pub fn reducer(state: &mut AppState, action: Action) -> DispatchResult<Effect> {
    match action {
        Action::Init => DispatchResult::changed_with_many(vec![
            Effect::LoadScenario {
                path: state.scenario_dir.clone(),
            },
            Effect::LoadGame {
                path: state.save_path.clone(),
            },
        ]),
        Action::UiTerminalResize(width, height) => {
            state.terminal_size = (width, height);
            DispatchResult::changed()
        }
        Action::Tick => {
            if state.pending_llm.is_some() {
                state.spinner_frame = state.spinner_frame.wrapping_add(1);
                DispatchResult::changed()
            } else if state.spinner_frame != 0 {
                state.spinner_frame = 0;
                DispatchResult::changed()
            } else {
                DispatchResult::unchanged()
            }
        }
        Action::Move(direction) => handle_move(state, direction),
        Action::Interact => handle_interact(state),
        Action::Talk => handle_talk(state),
        Action::OpenInventory => {
            state.mode = crate::state::GameMode::Inventory;
            DispatchResult::changed()
        }
        Action::OpenCustomAction => {
            state.mode = crate::state::GameMode::CustomAction;
            state.custom_action.input.clear();
            DispatchResult::changed()
        }
        Action::CloseOverlay => {
            state.mode = crate::state::GameMode::Exploration;
            DispatchResult::changed()
        }
        Action::DialogueInputChanged(input) => {
            state.dialogue.input = input;
            DispatchResult::changed()
        }
        Action::DialogueSubmit => handle_dialogue_submit(state),
        Action::DialogueResponse { npc_id, line } => {
            state.pending_llm = None;
            state.dialogue.history.push(crate::state::DialogueLine {
                speaker: "assistant".to_string(),
                text: line.clone(),
            });
            state.push_log(LogSpeaker::Npc, format!("{npc_id}: {line}"));
            state.dialogue.active_npc = None;
            state.mode = crate::state::GameMode::Exploration;
            DispatchResult::changed_with(save_effect(state))
        }
        Action::CustomActionInputChanged(input) => {
            state.custom_action.input = input;
            DispatchResult::changed()
        }
        Action::CustomActionSubmit => handle_custom_action_submit(state),
        Action::CustomActionInterpreted(result) => {
            state.pending_llm = None;
            handle_custom_action_result(state, result)
        }
        Action::CombatAttack => handle_combat_attack(state),
        Action::CombatEndTurn => handle_combat_end_turn(state),
        Action::ScrollLog(delta) => {
            let current = state.log_scroll as i16;
            let next = (current + delta).max(0) as u16;
            state.log_scroll = next;
            DispatchResult::changed()
        }
        Action::CreationNameChanged(value) => {
            state.creation.name = value;
            DispatchResult::changed()
        }
        Action::CreationSelectClass(index) => {
            state.creation.class_index = index.min(CLASS_OPTIONS.len().saturating_sub(1));
            DispatchResult::changed()
        }
        Action::CreationSelectBackground(index) => {
            state.creation.background_index = index.min(BACKGROUND_OPTIONS.len().saturating_sub(1));
            DispatchResult::changed()
        }
        Action::CreationSelectStat(index) => {
            state.creation.selected_stat = index.min(5);
            DispatchResult::changed()
        }
        Action::CreationAdjustStat(delta) => {
            adjust_stat(state, delta);
            DispatchResult::changed()
        }
        Action::CreationNext => {
            advance_creation(state, true);
            DispatchResult::changed()
        }
        Action::CreationBack => {
            advance_creation(state, false);
            DispatchResult::changed()
        }
        Action::CreationConfirm => finalize_creation(state),
        Action::SaveComplete => {
            if let Some(pending) = state.pending_transcript_index.take() {
                state.transcript_index = pending;
            }
            DispatchResult::changed()
        }
        Action::SaveError(error) => {
            state.pending_transcript_index = None;
            state.push_log(LogSpeaker::System, format!("Save failed: {error}"));
            DispatchResult::changed()
        }
        Action::LoadComplete(loaded) => {
            let scenario_dir = state.scenario_dir.clone();
            let save_path = state.save_path.clone();
            let provider = state.provider.clone();
            let model = state.model.clone();
            *state = *loaded;
            state.scenario_dir = scenario_dir;
            state.save_path = save_path;
            state.provider = provider;
            state.model = model;
            DispatchResult::changed()
        }
        Action::LoadError(error) => {
            if !is_missing_save(&error) {
                state.push_log(LogSpeaker::System, format!("Load failed: {error}"));
            }
            DispatchResult::changed()
        }
        Action::ScenarioLoaded { scenario } => {
            apply_scenario(state, scenario);
            DispatchResult::changed()
        }
        Action::ScenarioLoadError { error } => {
            state.push_log(LogSpeaker::System, format!("Scenario load failed: {error}"));
            DispatchResult::changed()
        }
        Action::LlmError(error) => {
            state.pending_llm = None;
            state.push_log(LogSpeaker::System, format!("LLM error: {error}"));
            if matches!(
                state.mode,
                crate::state::GameMode::Dialogue | crate::state::GameMode::CustomAction
            ) {
                state.mode = crate::state::GameMode::Exploration;
            }
            DispatchResult::changed()
        }
        Action::Quit => DispatchResult::unchanged(),
    }
}

fn handle_move(state: &mut AppState, direction: Direction) -> DispatchResult<Effect> {
    if state.mode == crate::state::GameMode::CharacterCreation {
        return DispatchResult::unchanged();
    }

    if state.mode == crate::state::GameMode::Combat {
        return handle_combat_move(state, direction);
    }

    if state.mode != crate::state::GameMode::Exploration {
        return DispatchResult::unchanged();
    }

    let (mut x, mut y) = state.player_pos();
    match direction {
        Direction::Up => y = y.saturating_sub(1),
        Direction::Down => y = y.saturating_add(1),
        Direction::Left => x = x.saturating_sub(1),
        Direction::Right => x = x.saturating_add(1),
    }
    if !state.map.is_walkable(x, y) {
        return DispatchResult::unchanged();
    }
    state.set_player_pos(x, y);
    check_triggers(state, TriggerKind::OnEnter);
    if let Some(enemy_id) = encounter_at(state, x, y) {
        return start_combat(state, enemy_id);
    }
    DispatchResult::changed()
}

fn handle_interact(state: &mut AppState) -> DispatchResult<Effect> {
    if state.mode != crate::state::GameMode::Exploration {
        return DispatchResult::unchanged();
    }
    let (x, y) = state.player_pos();
    if let Some(idx) = state.items.iter().position(|item| item.x == x && item.y == y) {
        let item = state.items.remove(idx);
        let name = item.name.clone();
        let qty = item.qty;
        add_item_to_inventory(state, item.id, item.name, item.qty);
        state.push_log(LogSpeaker::System, format!("Picked up {} x{}", name, qty));
    }
    check_triggers(state, TriggerKind::OnInteract);
    DispatchResult::changed_with(save_effect(state))
}

fn handle_talk(state: &mut AppState) -> DispatchResult<Effect> {
    if state.mode != crate::state::GameMode::Exploration {
        return DispatchResult::unchanged();
    }
    let (x, y) = state.player_pos();
    let npc = state
        .npcs
        .iter()
        .find(|npc| distance(npc.x, npc.y, x, y) <= 1);
    let Some(npc) = npc else {
        state.push_log(LogSpeaker::System, "No one nearby to talk to.");
        return DispatchResult::changed();
    };
    state.dialogue.active_npc = Some(npc.id.clone());
    state.dialogue.input.clear();
    state.mode = crate::state::GameMode::Dialogue;
    state.push_log(LogSpeaker::System, format!("You approach {}.", npc.name));
    DispatchResult::changed()
}

fn handle_dialogue_submit(state: &mut AppState) -> DispatchResult<Effect> {
    if state.pending_llm.is_some() {
        return DispatchResult::unchanged();
    }
    let npc_id = match state.dialogue.active_npc.clone() {
        Some(id) => id,
        None => return DispatchResult::unchanged(),
    };
    let input = state.dialogue.input.trim().to_string();
    if input.is_empty() {
        return DispatchResult::unchanged();
    }
    state.dialogue.input.clear();
    state.dialogue.history.push(crate::state::DialogueLine {
        speaker: "user".to_string(),
        text: input.clone(),
    });
    state.push_log(LogSpeaker::Player, input.clone());

    let npc = match state.npc_by_id(&npc_id) {
        Some(npc) => npc.clone(),
        None => return DispatchResult::unchanged(),
    };

    let request = prompt::build_dialogue_request(state, &npc, &input);
    state.pending_llm = Some(PendingLlm::Dialogue {
        npc_id: npc_id.clone(),
    });

    DispatchResult::changed_with(Effect::CallLlmDialogue { npc_id, request })
}

fn handle_custom_action_submit(state: &mut AppState) -> DispatchResult<Effect> {
    if state.pending_llm.is_some() {
        return DispatchResult::unchanged();
    }
    let input = state.custom_action.input.trim().to_string();
    if input.is_empty() {
        return DispatchResult::unchanged();
    }
    state.custom_action.input.clear();
    state.push_log(LogSpeaker::Player, format!("Action: {input}"));
    let request = prompt::build_action_request(state, &input);
    state.pending_llm = Some(PendingLlm::CustomAction);
    DispatchResult::changed_with(Effect::CallLlmInterpretAction { request })
}

fn handle_custom_action_result(
    state: &mut AppState,
    result: ActionInterpretation,
) -> DispatchResult<Effect> {
    if result.kind.to_lowercase() != "skill_check" {
        state.push_log(LogSpeaker::System, "Action could not be interpreted.");
        return DispatchResult::changed();
    }
    let check = parse_skill_or_ability(&result.skill);
    let difficulty = parse_difficulty(&result.difficulty);
    let dc = difficulty_dc(difficulty);
    let (label, modifier) = match check {
        CheckKind::Skill(skill) => {
            let ability = skill_to_ability(skill);
            (format!("{skill:?}"), ability_modifier(state.ability_score(ability)))
        }
        CheckKind::Ability(ability) => (
            format!("{ability:?}"),
            ability_modifier(state.ability_score(ability)),
        ),
    };
    let roll = roll_d20(&mut state.rng_seed);
    let total = roll + modifier;
    let success = total >= dc;
    state.push_log(
        LogSpeaker::System,
        format!(
            "Check {label} ({difficulty:?}) DC {dc}: rolled {roll} {modifier:+} = {total} => {}",
            if success { "success" } else { "failure" }
        ),
    );
    if success {
        state.push_log(LogSpeaker::System, result.on_success);
    } else {
        state.push_log(LogSpeaker::System, result.on_failure);
    }
    state.mode = crate::state::GameMode::Exploration;
    DispatchResult::changed_with(save_effect(state))
}

fn handle_combat_move(state: &mut AppState, direction: Direction) -> DispatchResult<Effect> {
    let (player_turn, movement_left) = match state.combat.as_ref() {
        Some(combat) => (combat.player_turn, combat.movement_left),
        None => return DispatchResult::unchanged(),
    };
    if !player_turn || movement_left == 0 {
        return DispatchResult::unchanged();
    }
    let (mut x, mut y) = state.player_pos();
    match direction {
        Direction::Up => y = y.saturating_sub(1),
        Direction::Down => y = y.saturating_add(1),
        Direction::Left => x = x.saturating_sub(1),
        Direction::Right => x = x.saturating_add(1),
    }
    if !state.map.is_walkable(x, y) {
        return DispatchResult::unchanged();
    }
    state.set_player_pos(x, y);
    if let Some(combat) = state.combat.as_mut() {
        combat.movement_left = combat.movement_left.saturating_sub(1);
    }
    DispatchResult::changed()
}

fn handle_combat_attack(state: &mut AppState) -> DispatchResult<Effect> {
    if state.mode != crate::state::GameMode::Combat {
        return DispatchResult::unchanged();
    }
    let (player_turn, enemy_id) = match state.combat.as_ref() {
        Some(combat) => (combat.player_turn, combat.enemy_id.clone()),
        None => return DispatchResult::unchanged(),
    };
    if !player_turn {
        return DispatchResult::unchanged();
    }
    let (px, py) = state.player_pos();
    let enemy_index = match state.encounters.iter().position(|e| e.id == enemy_id) {
        Some(index) => index,
        None => return DispatchResult::unchanged(),
    };
    let (enemy_name, enemy_x, enemy_y, enemy_defeated) = {
        let enemy = &state.encounters[enemy_index];
        (
            enemy.name.clone(),
            enemy.x,
            enemy.y,
            enemy.defeated,
        )
    };
    if enemy_defeated {
        return DispatchResult::unchanged();
    }
    if distance(px, py, enemy_x, enemy_y) > 1 {
        state.push_log(LogSpeaker::Combat, "Enemy out of range.");
        return DispatchResult::changed();
    }

    let roll = roll_d20(&mut state.rng_seed);
    let modifier = ability_modifier(state.ability_score(Ability::Strength));
    let total = roll + modifier;
    let hit = total >= 10;
    if hit {
        let damage = (roll_damage(&mut state.rng_seed, 6) + modifier).max(1);
        let mut defeated = false;
        {
            let enemy = &mut state.encounters[enemy_index];
            enemy.hp -= damage;
            if enemy.hp <= 0 {
                enemy.defeated = true;
                defeated = true;
            }
        }
        state.push_log(
            LogSpeaker::Combat,
            format!("You hit {} for {} damage.", enemy_name, damage),
        );
        if defeated {
            state.push_log(LogSpeaker::Combat, format!("{} is defeated.", enemy_name));
            state.combat = None;
            state.mode = crate::state::GameMode::Exploration;
            return DispatchResult::changed_with(save_effect(state));
        }
    } else {
        state.push_log(LogSpeaker::Combat, "You miss.");
    }

    handle_combat_end_turn(state)
}

fn handle_combat_end_turn(state: &mut AppState) -> DispatchResult<Effect> {
    let (player_turn, enemy_id) = match state.combat.as_ref() {
        Some(combat) => (combat.player_turn, combat.enemy_id.clone()),
        None => return DispatchResult::unchanged(),
    };
    if !player_turn {
        return DispatchResult::unchanged();
    }

    if let Some(combat) = state.combat.as_mut() {
        combat.player_turn = false;
    }

    let enemy_index = match state.encounters.iter().position(|e| e.id == enemy_id) {
        Some(index) => index,
        None => return DispatchResult::unchanged(),
    };
    let (enemy_name, enemy_atk, enemy_defeated) = {
        let enemy = &state.encounters[enemy_index];
        (enemy.name.clone(), enemy.atk, enemy.defeated)
    };
    if !enemy_defeated {
        let roll = roll_d20(&mut state.rng_seed);
        let player_ac = 10 + ability_modifier(state.ability_score(Ability::Dexterity));
        let hit = roll + enemy_atk >= player_ac;
        if hit {
            let damage = (roll_damage(&mut state.rng_seed, 6) + enemy_atk).max(1);
            state.player.hp -= damage;
            state.push_log(
                LogSpeaker::Combat,
                format!("{} hits you for {} damage.", enemy_name, damage),
            );
            if state.player.hp <= 0 {
                state.push_log(LogSpeaker::Combat, "You fall unconscious.");
                state.combat = None;
                state.mode = crate::state::GameMode::Exploration;
                return DispatchResult::changed_with(save_effect(state));
            }
        } else {
            state.push_log(LogSpeaker::Combat, format!("{} misses.", enemy_name));
        }
    }

    if let Some(combat) = state.combat.as_mut() {
        combat.player_turn = true;
        combat.movement_left = MOVEMENT_PER_TURN;
        combat.round = combat.round.saturating_add(1);
    }
    DispatchResult::changed()
}

fn adjust_stat(state: &mut AppState, delta: i8) {
    let ability = match state.creation.selected_stat {
        0 => Ability::Strength,
        1 => Ability::Dexterity,
        2 => Ability::Constitution,
        3 => Ability::Intelligence,
        4 => Ability::Wisdom,
        _ => Ability::Charisma,
    };
    let current = state.creation.stats.get(ability);
    let next = clamp_score(current + delta as i32);
    state.creation.stats.set(ability, next);
    state.creation.points_remaining = points_remaining(&state.creation.stats, 27);
}

fn advance_creation(state: &mut AppState, forward: bool) {
    use crate::state::CreationStep::*;
    state.creation.step = match (state.creation.step, forward) {
        (Name, true) => Class,
        (Class, true) => Background,
        (Background, true) => Stats,
        (Stats, true) => Confirm,
        (Confirm, true) => Confirm,
        (Confirm, false) => Stats,
        (Stats, false) => Background,
        (Background, false) => Class,
        (Class, false) => Name,
        (Name, false) => Name,
    };
}

fn finalize_creation(state: &mut AppState) -> DispatchResult<Effect> {
    if state.creation.points_remaining < 0 {
        state.push_log(LogSpeaker::System, "Point buy is invalid.");
        return DispatchResult::changed();
    }
    let class_name = CLASS_OPTIONS
        .get(state.creation.class_index)
        .copied()
        .unwrap_or("Adventurer");
    let background = BACKGROUND_OPTIONS
        .get(state.creation.background_index)
        .copied()
        .unwrap_or("Wanderer");
    state.player.name = state.creation.name.trim().to_string();
    if state.player.name.is_empty() {
        state.player.name = "Adventurer".to_string();
    }
    state.player.class_name = class_name.to_string();
    state.player.background = background.to_string();
    state.player.stats = state.creation.stats.clone();
    let con_mod = ability_modifier(state.player.stats.get(Ability::Constitution));
    state.player.max_hp = (class_base_hp(class_name) + con_mod).max(1);
    state.player.hp = state.player.max_hp;
    state.mode = crate::state::GameMode::Exploration;
    state.push_log(
        LogSpeaker::System,
        format!("Welcome, {} the {}.", state.player.name, class_name),
    );
    DispatchResult::changed_with(save_effect(state))
}

fn start_combat(state: &mut AppState, enemy_id: String) -> DispatchResult<Effect> {
    let enemy = match state.encounters.iter().find(|e| e.id == enemy_id) {
        Some(enemy) => enemy,
        None => return DispatchResult::unchanged(),
    };
    if enemy.defeated {
        return DispatchResult::unchanged();
    }
    let player_init =
        roll_d20(&mut state.rng_seed) + ability_modifier(state.ability_score(Ability::Dexterity));
    let enemy_init = roll_d20(&mut state.rng_seed);
    let player_turn = player_init >= enemy_init;
    state.combat = Some(CombatState {
        enemy_id: enemy_id.clone(),
        player_turn,
        movement_left: MOVEMENT_PER_TURN,
        round: 1,
    });
    state.mode = crate::state::GameMode::Combat;
    state.push_log(LogSpeaker::Combat, format!("Combat begins with {}!", enemy.name));
    DispatchResult::changed()
}

fn apply_scenario(state: &mut AppState, scenario: ScenarioRuntime) {
    if state.map.width > 1 && !state.npcs.is_empty() {
        state.scenario = Some(crate::state::ScenarioManifestSummary {
            id: scenario.manifest.id.clone(),
            name: scenario.manifest.name.clone(),
            lore: scenario.manifest.lore.clone(),
        });
        return;
    }
    state.map = scenario.map.clone();
    state.npcs = scenario.npcs.clone();
    state.items = scenario.items.clone();
    state.encounters = scenario.encounters.clone();
    state.triggers = scenario.triggers.clone();
    state.scenario = Some(crate::state::ScenarioManifestSummary {
        id: scenario.manifest.id.clone(),
        name: scenario.manifest.name.clone(),
        lore: scenario.manifest.lore.clone(),
    });
    if state.player.x == 0 && state.player.y == 0 {
        state.player.x = scenario.manifest.player_start.x;
        state.player.y = scenario.manifest.player_start.y;
    }
}

fn add_item_to_inventory(state: &mut AppState, id: String, name: String, qty: u16) {
    if let Some(stack) = state.player.inventory.iter_mut().find(|item| item.id == id) {
        stack.qty = stack.qty.saturating_add(qty);
    } else {
        state.player
            .inventory
            .push(crate::state::ItemStack { id, name, qty });
    }
}

fn check_triggers(state: &mut AppState, kind: TriggerKind) {
    let (x, y) = state.player_pos();
    let triggers = state.triggers.clone();
    for trigger in triggers {
        match (trigger, kind) {
            (Trigger::OnEnter { x: tx, y: ty, message }, TriggerKind::OnEnter)
                if tx == x && ty == y =>
            {
                let id = format!("enter:{tx}:{ty}");
                if state.fired_triggers.insert(id) {
                    state.push_log(LogSpeaker::System, message.clone());
                }
            }
            (Trigger::OnInteract { x: tx, y: ty, message }, TriggerKind::OnInteract)
                if tx == x && ty == y =>
            {
                let id = format!("interact:{tx}:{ty}");
                if state.fired_triggers.insert(id) {
                    state.push_log(LogSpeaker::System, message.clone());
                }
            }
            _ => {}
        }
    }
}

fn encounter_at(state: &AppState, x: u16, y: u16) -> Option<String> {
    state
        .encounters
        .iter()
        .find(|e| e.x == x && e.y == y && !e.defeated)
        .map(|e| e.id.clone())
}

fn distance(ax: u16, ay: u16, bx: u16, by: u16) -> u16 {
    ax.abs_diff(bx) + ay.abs_diff(by)
}

fn save_effect(state: &mut AppState) -> Effect {
    let since = state.transcript_index;
    state.pending_transcript_index = Some(state.log.len());
    Effect::SaveGame {
        state: Box::new(state.clone()),
        since,
    }
}

fn is_missing_save(error: &str) -> bool {
    let lower = error.to_lowercase();
    lower.contains("no such file") || lower.contains("not found")
}

#[derive(Copy, Clone, Debug)]
enum TriggerKind {
    OnEnter,
    OnInteract,
}
