use tui_dispatch::DispatchResult;

use crate::action::Action;
use crate::effect::Effect;
use crate::scenario::{AbilityEffect, AbilitySpec, ScenarioRuntime, ScenarioTrigger};
use crate::state::{
    calc_hp, calc_stat, exp_for_level, AppState, BattleActor, BattleEnemyState, BattleKind,
    BattlePartyState, BattleStage, Direction, GameMode, ItemKind, MenuState, PartyMember,
    PendingAction, PendingActionKind, Pickup, PokemonSelectState, SpriteState, SpriteTarget, Tile,
    MAX_LEVEL,
};

const DEFAULT_STARTERS: [&str; 4] = ["pikachu", "charmander", "bulbasaur", "squirtle"];
const DEFAULT_WILD_POOL: [&str; 6] = ["pidgey", "rattata", "caterpie", "weedle", "oddish", "zubat"];
const BOSS_NAME: &str = "onix";
const RELIC_WINS: u16 = 3;
const BOSS_WINS: u16 = 5;
const MOVE_POWER: u32 = 40;
const ATB_MAX: u16 = 1000;
const ATB_BASE_GAIN: u16 = 35;
const ATB_SPEED_DIV: u16 = 6;
const PARTY_LIMIT: usize = 3;

pub fn reducer(state: &mut AppState, action: Action) -> DispatchResult<Effect> {
    match action {
        Action::Init => {
            // Start at main menu and check if save exists
            state.mode = GameMode::MainMenu;
            state.overworld_full_map = false;
            state.menu = Some(MenuState {
                selected: 0,
                has_save: false,
            });
            state.message = None;
            state.message_queue.clear();
            state.message_timer = 0;
            DispatchResult::changed_with(Effect::LoadScenario {
                path: state.scenario_dir.clone(),
            })
        }
        Action::UiTerminalResize(width, height) => {
            if state.terminal_size != (width, height) {
                state.terminal_size = (width, height);
                DispatchResult::changed()
            } else {
                DispatchResult::unchanged()
            }
        }
        Action::Tick => tick_animation(state),
        Action::Move(direction) => move_player(state, direction),
        Action::OverworldToggleFullMap => {
            if state.mode == GameMode::Overworld {
                state.overworld_full_map = !state.overworld_full_map;
                DispatchResult::changed()
            } else {
                DispatchResult::unchanged()
            }
        }
        Action::BattleMenuNext => battle_menu_change(state, 1),
        Action::BattleMenuPrev => battle_menu_change(state, -1),
        Action::BattleConfirm => battle_confirm(state),
        Action::MessageNext => message_next(state),
        Action::BattleItemCancel => {
            let stage = state.battle.as_ref().map(|battle| battle.stage);
            if matches!(stage, Some(BattleStage::ItemMenu)) {
                if let Some(battle) = state.battle.as_mut() {
                    battle.stage = BattleStage::CommandMenu;
                    battle.pending_action = None;
                }
                set_battle_menu_prompt(state);
                return DispatchResult::changed();
            }
            DispatchResult::unchanged()
        }
        Action::PokemonDidLoad { target, info } => pokemon_loaded(state, target, info),
        Action::PokemonDidError {
            target,
            name,
            error,
        } => pokemon_error(state, target, &name, &error),
        Action::SpriteDidLoad { target, sprite } => sprite_loaded(state, target, sprite),
        Action::SpriteDidError { target, error } => sprite_error(state, target, &error),

        // Main menu actions
        Action::MenuSelect(index) => {
            if let Some(menu) = state.menu.as_mut() {
                menu.selected = index;
            }
            DispatchResult::changed()
        }
        Action::MenuConfirm => menu_confirm(state),
        Action::SaveExists(exists) => {
            if let Some(menu) = state.menu.as_mut() {
                menu.has_save = exists;
            }
            DispatchResult::changed()
        }
        Action::ScenarioLoaded { scenario } => {
            apply_scenario(state, scenario);
            DispatchResult::changed_with(Effect::CheckSaveExists)
        }
        Action::ScenarioLoadError { error } => {
            state.scenario = None;
            push_message(state, format!("Scenario load failed: {}", error));
            DispatchResult::changed_with(Effect::CheckSaveExists)
        }
        Action::PartySpriteLoaded { index, sprite } => {
            ensure_party_sprites(state);
            if let Some(slot) = state.party_sprites.get_mut(index) {
                slot.sprite = Some(sprite.clone());
                slot.sprite_flipped = Some(sprite.flipped());
                slot.frame_index = 0;
                slot.frame_tick = 0;
                slot.loading = false;
            }
            DispatchResult::changed()
        }
        Action::PartySpriteError { index, error } => {
            if let Some(slot) = state.party_sprites.get_mut(index) {
                slot.loading = false;
            }
            push_message(state, format!("Party sprite error: {}", error));
            DispatchResult::changed()
        }

        // Pokemon selection actions
        Action::StarterSelect(index) => starter_select(state, index),
        Action::StarterConfirm => starter_confirm(state),
        Action::StarterPreviewLoaded { info } => {
            if let Some(select) = state.pokemon_select.as_mut() {
                select.preview_info = Some(info.clone());
                if let Some(url) = info.sprite_front_animated.or(info.sprite_front_default) {
                    select.preview_sprite.loading = true;
                    return DispatchResult::changed_with(Effect::LoadStarterSprite { url });
                }
            }
            DispatchResult::changed()
        }
        Action::StarterPreviewSpriteLoaded { sprite } => {
            if let Some(select) = state.pokemon_select.as_mut() {
                select.preview_sprite.sprite = Some(sprite.clone());
                select.preview_sprite.sprite_flipped = Some(sprite.flipped());
                select.preview_sprite.frame_index = 0;
                select.preview_sprite.frame_tick = 0;
                select.preview_sprite.loading = false;
            }
            DispatchResult::changed()
        }
        Action::StarterPreviewError { error } => {
            if let Some(select) = state.pokemon_select.as_mut() {
                select.preview_sprite.loading = false;
            }
            push_message(state, format!("Preview error: {}", error));
            DispatchResult::changed()
        }

        // Pause menu actions
        Action::PauseOpen => {
            if state.mode == GameMode::Overworld || state.mode == GameMode::Battle {
                state.pause_menu.is_open = true;
                state.pause_menu.selected = 0;
            }
            DispatchResult::changed()
        }
        Action::PauseClose => {
            state.pause_menu.is_open = false;
            DispatchResult::changed()
        }
        Action::PauseSelect(index) => {
            state.pause_menu.selected = index;
            DispatchResult::changed()
        }
        Action::PauseConfirm => pause_confirm(state),

        // Save/Load actions
        Action::SaveGame => DispatchResult::changed_with(Effect::SaveGame {
            state: Box::new(state.clone()),
        }),
        Action::SaveComplete => {
            push_message(state, "Game saved!");
            state.pause_menu.is_open = false;
            DispatchResult::changed()
        }
        Action::SaveError(error) => {
            push_message(state, format!("Save failed: {}", error));
            DispatchResult::changed()
        }
        Action::LoadGame => DispatchResult::changed_with(Effect::LoadGame),
        Action::LoadComplete(loaded_state) => {
            // Replace entire state with loaded state
            let scenario_dir = state.scenario_dir.clone();
            *state = *loaded_state;
            if state.scenario_dir.is_empty() {
                state.scenario_dir = scenario_dir;
            } else if state.scenario_dir != scenario_dir {
                state.scenario_dir = scenario_dir;
            }
            normalize_loaded_state(state);
            push_message(state, "Game loaded!");
            if state.scenario.is_none() {
                DispatchResult::changed_with(Effect::LoadScenario {
                    path: state.scenario_dir.clone(),
                })
            } else {
                DispatchResult::changed()
            }
        }
        Action::LoadError(error) => {
            push_message(state, format!("Load failed: {}", error));
            DispatchResult::changed()
        }

        Action::Quit => DispatchResult::unchanged(),
    }
}

fn move_player(state: &mut AppState, direction: Direction) -> DispatchResult<Effect> {
    if state.mode != GameMode::Overworld {
        return DispatchResult::unchanged();
    }

    // Always update facing direction
    state.player.facing = direction;

    let (mut next_x, mut next_y) = (state.player.x, state.player.y);
    match direction {
        Direction::Up => next_y = next_y.saturating_sub(1),
        Direction::Down => next_y = next_y.saturating_add(1),
        Direction::Left => next_x = next_x.saturating_sub(1),
        Direction::Right => next_x = next_x.saturating_add(1),
    }
    if next_x == state.player.x && next_y == state.player.y {
        return DispatchResult::changed(); // Still changed because facing might have changed
    }
    if next_x >= state.map.width || next_y >= state.map.height {
        return DispatchResult::changed();
    }
    if !state.map.is_walkable(next_x, next_y) {
        return DispatchResult::changed();
    }

    state.player.x = next_x;
    state.player.y = next_y;
    state.player.steps = state.player.steps.wrapping_add(1);
    state.steps_since_encounter = state.steps_since_encounter.saturating_add(1);

    collect_pickup(state, next_x, next_y);
    trigger_tile_events(state, next_x, next_y);

    if state.map.is_grass(next_x, next_y) && state.steps_since_encounter >= 3 {
        let roll = next_rand(state) % 100;
        if roll < 18 {
            state.steps_since_encounter = 0;
            if state.boss_defeated {
                return DispatchResult::changed();
            }
            if state.has_relic && state.wild_wins >= BOSS_WINS {
                return start_boss_battle(state);
            }
            return start_wild_battle(state);
        }
    }

    DispatchResult::changed()
}

fn start_wild_battle(state: &mut AppState) -> DispatchResult<Effect> {
    let scenario_pool = state
        .scenario
        .as_ref()
        .map(|scenario| scenario.manifest.wild_pool.clone())
        .filter(|pool| !pool.is_empty())
        .unwrap_or_else(|| DEFAULT_WILD_POOL.iter().map(|s| s.to_string()).collect());
    let max_group = state.party.len().saturating_add(1).clamp(1, 3) as u32;
    let group_size = ((next_rand(state) % max_group) + 1) as usize;
    let levels = roll_enemy_levels_for_group(state, group_size);
    let mut enemies = Vec::with_capacity(group_size);
    for idx in 0..group_size {
        let index = (next_rand(state) as usize) % scenario_pool.len();
        enemies.push((scenario_pool[index].clone(), levels[idx]));
    }
    start_battle(state, enemies, BattleKind::Wild)
}

fn start_boss_battle(state: &mut AppState) -> DispatchResult<Effect> {
    let enemy_name = BOSS_NAME.to_string();
    let enemy_level = boss_level(state);
    start_battle(state, vec![(enemy_name, enemy_level)], BattleKind::Boss)
}

fn start_battle(
    state: &mut AppState,
    enemy_specs: Vec<(String, u8)>,
    kind: BattleKind,
) -> DispatchResult<Effect> {
    state.mode = GameMode::Battle;
    state.enemy_info = None;
    state.enemy_sprite.reset();
    state.enemy_sprites.clear();
    state.enemy_sprite_loading_species = None;
    state.enemy_info_cache.clear();
    if state.party.is_empty() {
        if let Some(info) = state.player_info.clone() {
            let level = state.player_level.max(1);
            let max_hp = calc_hp(info.hp, level).max(1);
            state.party.push(PartyMember {
                info,
                level,
                exp: state.player_exp.max(exp_for_level(level)),
                hp: max_hp,
                ability_id: None,
                ability_cd: 0,
            });
            state.active_party_index = 0;
        }
    }
    if state.party.is_empty() || enemy_specs.is_empty() {
        return DispatchResult::unchanged();
    }

    let mut party = build_battle_party_states(state);
    let mut enemies = enemy_specs
        .into_iter()
        .enumerate()
        .map(|(enemy_index, (species, level))| {
            let hp_max = calc_hp(10, level).max(1);
            BattleEnemyState {
                enemy_index,
                species,
                level,
                hp: hp_max,
                hp_max,
                atb: 0,
                ready: false,
                defeated: false,
                attack: 10,
                defense: 10,
                speed: 10,
                base_experience: 60,
                info_loaded: false,
                info_loading: false,
            }
        })
        .collect::<Vec<_>>();
    seed_initial_atb(state, &mut party, &mut enemies);
    let intro_name = enemies
        .first()
        .map(|enemy| format_name(&enemy.species))
        .unwrap_or_else(|| "Enemy".to_string());
    let message = match kind {
        BattleKind::Boss => format!("Boss {} appears!", intro_name),
        BattleKind::Wild => format!("A wild {} appeared!", intro_name),
    };
    state.battle = Some(crate::state::BattleState::new(
        kind, enemies, party, message,
    ));
    ensure_enemy_sprites(state);

    let mut effects = Vec::new();
    if let Some(effect) = load_player_sprite_for_active(state) {
        effects.push(effect);
    }
    if let Some(effect) = try_load_next_enemy_species(state) {
        effects.push(effect);
    }
    match effects.len() {
        0 => DispatchResult::changed(),
        1 => DispatchResult::changed_with(effects.remove(0)),
        _ => DispatchResult::changed_with_many(effects),
    }
}

fn battle_menu_change(state: &mut AppState, delta: i16) -> DispatchResult<Effect> {
    let Some(stage) = state.battle.as_ref().map(|battle| battle.stage) else {
        return DispatchResult::unchanged();
    };
    let menu_len = match stage {
        BattleStage::CommandMenu => 5usize,
        BattleStage::ItemMenu => available_items(state).len(),
        BattleStage::EnemyTargetMenu => living_enemy_indexes(state).len(),
        BattleStage::AllyTargetMenu => living_party_indexes(state).len(),
        _ => 0,
    };
    if menu_len == 0 {
        return DispatchResult::unchanged();
    }
    let current_index = match stage {
        BattleStage::CommandMenu => state
            .battle
            .as_ref()
            .map(|battle| battle.menu_index)
            .unwrap_or(0),
        BattleStage::ItemMenu => state
            .battle
            .as_ref()
            .map(|battle| battle.item_index)
            .unwrap_or(0),
        BattleStage::EnemyTargetMenu => state
            .battle
            .as_ref()
            .map(|battle| battle.enemy_target_index)
            .unwrap_or(0),
        BattleStage::AllyTargetMenu => state
            .battle
            .as_ref()
            .map(|battle| battle.ally_target_index)
            .unwrap_or(0),
        _ => 0,
    } as i16;
    let menu_len = menu_len as i16;
    let mut next = current_index + delta;
    if next < 0 {
        next = menu_len - 1;
    }
    if next >= menu_len {
        next = 0;
    }
    if next as usize == current_index as usize {
        return DispatchResult::unchanged();
    }
    if let Some(battle) = state.battle.as_mut() {
        match stage {
            BattleStage::CommandMenu => battle.menu_index = next as usize,
            BattleStage::ItemMenu => battle.item_index = next as usize,
            BattleStage::EnemyTargetMenu => battle.enemy_target_index = next as usize,
            BattleStage::AllyTargetMenu => battle.ally_target_index = next as usize,
            _ => {}
        }
    }
    DispatchResult::changed()
}

fn set_battle_menu_prompt(state: &mut AppState) {
    let player_name = state
        .battle
        .as_ref()
        .and_then(|battle| match battle.current_actor {
            Some(BattleActor::Party(index)) => state
                .party
                .get(index)
                .map(|member| format_name(&member.info.name)),
            _ => None,
        })
        .unwrap_or_else(|| format_name(&state.player_name()));
    if let Some(battle) = state.battle.as_mut() {
        battle.message = format!("What will {} do?", player_name);
    }
}

fn set_battle_item_prompt(state: &mut AppState) {
    if let Some(battle) = state.battle.as_mut() {
        battle.message = "Choose an item.".to_string();
    }
}

fn set_battle_enemy_target_prompt(state: &mut AppState) {
    if let Some(battle) = state.battle.as_mut() {
        battle.message = "Choose an enemy target.".to_string();
    }
}

fn set_battle_ally_target_prompt(state: &mut AppState) {
    if let Some(battle) = state.battle.as_mut() {
        battle.message = "Choose an ally target.".to_string();
    }
}

fn battle_confirm(state: &mut AppState) -> DispatchResult<Effect> {
    let Some(stage) = state.battle.as_ref().map(|battle| battle.stage) else {
        return DispatchResult::unchanged();
    };

    match stage {
        BattleStage::Intro => {
            if let Some(battle) = state.battle.as_mut() {
                battle.stage = BattleStage::Waiting;
                battle.message = "Turn gauges are charging...".to_string();
            }
            DispatchResult::changed()
        }
        BattleStage::CommandMenu => handle_command_confirm(state),
        BattleStage::ItemMenu => handle_item_confirm(state),
        BattleStage::EnemyTargetMenu => handle_enemy_target_confirm(state),
        BattleStage::AllyTargetMenu => handle_ally_target_confirm(state),
        BattleStage::Waiting => DispatchResult::unchanged(),
        BattleStage::Victory | BattleStage::Escape | BattleStage::Defeat => end_battle(state),
    }
}

fn handle_command_confirm(state: &mut AppState) -> DispatchResult<Effect> {
    let (menu_index, actor, kind) = match state.battle.as_ref() {
        Some(battle) => (battle.menu_index, battle.current_actor, battle.kind),
        None => return DispatchResult::unchanged(),
    };
    let BattleActor::Party(member_index) = (match actor {
        Some(actor) => actor,
        None => return DispatchResult::unchanged(),
    }) else {
        return DispatchResult::unchanged();
    };
    match menu_index {
        0 => {
            if let Some(battle) = state.battle.as_mut() {
                battle.pending_action = Some(PendingAction {
                    actor: BattleActor::Party(member_index),
                    kind: PendingActionKind::Fight,
                    ability_name: None,
                });
                battle.stage = BattleStage::EnemyTargetMenu;
                battle.enemy_target_index = 0;
            }
            set_battle_enemy_target_prompt(state);
            DispatchResult::changed()
        }
        1 => {
            let items = available_items(state);
            if items.is_empty() {
                if let Some(battle) = state.battle.as_mut() {
                    battle.message = "Your bag is empty.".to_string();
                }
                return DispatchResult::changed();
            }
            if let Some(battle) = state.battle.as_mut() {
                battle.stage = BattleStage::ItemMenu;
                battle.item_index = 0;
                battle.pending_action = Some(PendingAction {
                    actor: BattleActor::Party(member_index),
                    kind: PendingActionKind::Item { kind: items[0].0 },
                    ability_name: None,
                });
            }
            set_battle_item_prompt(state);
            DispatchResult::changed()
        }
        2 => {
            let ability = ability_spec_for_member(state, member_index);
            if ability.is_none() {
                if let Some(battle) = state.battle.as_mut() {
                    battle.message = "No ability available.".to_string();
                }
                return DispatchResult::changed();
            }
            let ability = ability.expect("checked ability");
            let ability_cd = state
                .party
                .get(member_index)
                .map(|member| member.ability_cd)
                .unwrap_or(0);
            if ability_cd > 0 {
                if let Some(battle) = state.battle.as_mut() {
                    battle.message = format!("Ability recharging ({}).", ability_cd);
                }
                return DispatchResult::changed();
            }
            if let Some(member) = state.party.get_mut(member_index) {
                member.ability_cd = ability.cooldown;
            }
            if let Some(battle) = state.battle.as_mut() {
                battle.pending_action = Some(match ability.effect {
                    AbilityEffect::Damage { power } => PendingAction {
                        actor: BattleActor::Party(member_index),
                        kind: PendingActionKind::AbilityDamage {
                            power: power.max(1),
                        },
                        ability_name: Some(ability.name.clone()),
                    },
                    AbilityEffect::Heal { amount } => PendingAction {
                        actor: BattleActor::Party(member_index),
                        kind: PendingActionKind::AbilityHeal {
                            amount: amount.max(1),
                        },
                        ability_name: Some(ability.name.clone()),
                    },
                    AbilityEffect::Guard { reduction_pct, .. } => PendingAction {
                        actor: BattleActor::Party(member_index),
                        kind: PendingActionKind::AbilityGuard {
                            reduction_pct: reduction_pct.min(90),
                        },
                        ability_name: Some(ability.name.clone()),
                    },
                });
            }
            match ability.effect {
                AbilityEffect::Damage { .. } => {
                    if let Some(battle) = state.battle.as_mut() {
                        battle.stage = BattleStage::EnemyTargetMenu;
                        battle.enemy_target_index = 0;
                    }
                    set_battle_enemy_target_prompt(state);
                }
                AbilityEffect::Heal { .. } | AbilityEffect::Guard { .. } => {
                    if let Some(battle) = state.battle.as_mut() {
                        battle.stage = BattleStage::AllyTargetMenu;
                        battle.ally_target_index = 0;
                    }
                    set_battle_ally_target_prompt(state);
                }
            }
            DispatchResult::changed()
        }
        3 => {
            if let Some(slot) = state.battle.as_mut().and_then(|battle| {
                battle
                    .party
                    .iter_mut()
                    .find(|slot| slot.member_index == member_index)
            }) {
                slot.guard_active = true;
                slot.guard_pct = 50;
            }
            if let Some(battle) = state.battle.as_mut() {
                battle.message = "Defending until next turn.".to_string();
            }
            finish_actor_turn(state);
            DispatchResult::changed()
        }
        _ => {
            if kind == BattleKind::Boss {
                if let Some(battle) = state.battle.as_mut() {
                    battle.message = "Can't run from a boss!".to_string();
                }
                return DispatchResult::changed();
            }
            let chance = run_chance_pct(state);
            let roll = (next_rand(state) % 100) as i32 + 1;
            if roll <= chance {
                if let Some(battle) = state.battle.as_mut() {
                    battle.stage = BattleStage::Escape;
                    battle.message = "Got away safely!".to_string();
                }
            } else {
                if let Some(battle) = state.battle.as_mut() {
                    battle.run_attempts = battle.run_attempts.saturating_add(1);
                    battle.message = format!("Couldn't escape! ({}%)", chance);
                }
                finish_actor_turn(state);
            }
            DispatchResult::changed()
        }
    }
}

fn handle_item_confirm(state: &mut AppState) -> DispatchResult<Effect> {
    let items = available_items(state);
    if items.is_empty() {
        if let Some(battle) = state.battle.as_mut() {
            battle.stage = BattleStage::CommandMenu;
            battle.pending_action = None;
            battle.message = "Your bag is empty.".to_string();
        }
        return DispatchResult::changed();
    }
    let item_index = state
        .battle
        .as_ref()
        .map(|battle| battle.item_index)
        .unwrap_or(0)
        .min(items.len().saturating_sub(1));
    let (kind, _) = items[item_index];
    if let Some(battle) = state.battle.as_mut() {
        if let Some(pending) = battle.pending_action.as_mut() {
            pending.kind = PendingActionKind::Item { kind };
        } else {
            let actor = battle
                .current_actor
                .unwrap_or(BattleActor::Party(state.active_party_index));
            battle.pending_action = Some(PendingAction {
                actor,
                kind: PendingActionKind::Item { kind },
                ability_name: None,
            });
        }
        battle.stage = BattleStage::AllyTargetMenu;
        battle.ally_target_index = 0;
    }
    set_battle_ally_target_prompt(state);
    DispatchResult::changed()
}

fn handle_enemy_target_confirm(state: &mut AppState) -> DispatchResult<Effect> {
    let targets = living_enemy_indexes(state);
    if targets.is_empty() {
        return DispatchResult::changed();
    }
    let (target_slot, pending) = match state.battle.as_ref() {
        Some(battle) => {
            let slot = battle
                .enemy_target_index
                .min(targets.len().saturating_sub(1));
            (targets[slot], battle.pending_action.clone())
        }
        None => return DispatchResult::unchanged(),
    };
    let pending = match pending {
        Some(pending) => pending,
        None => return DispatchResult::unchanged(),
    };
    let BattleActor::Party(member_index) = pending.actor else {
        return DispatchResult::unchanged();
    };
    let (actor_name, level, attack) = match state.party.get(member_index) {
        Some(member) => (
            format_name(&member.info.name),
            member.level.max(1),
            member.info.attack,
        ),
        None => return DispatchResult::unchanged(),
    };
    let enemy_name = state
        .battle
        .as_ref()
        .and_then(|battle| battle.enemies.get(target_slot))
        .map(|enemy| format_name(&enemy.species))
        .unwrap_or_else(|| "Enemy".to_string());
    let damage = match pending.kind {
        PendingActionKind::Fight => calc_damage(
            state,
            level,
            calc_stat(attack, level),
            enemy_defense_value(state, target_slot),
        ),
        PendingActionKind::AbilityDamage { power } => power.max(1),
        _ => return DispatchResult::unchanged(),
    };
    let defeated = apply_damage_to_enemy(state, target_slot, damage);
    let message = match pending.kind {
        PendingActionKind::AbilityDamage { .. } => {
            let ability_name = pending
                .ability_name
                .unwrap_or_else(|| "Ability".to_string());
            format!("{actor_name} used {ability_name} on {enemy_name} for {damage}!")
        }
        _ => format!("{actor_name} hit {enemy_name} for {damage}!"),
    };
    if let Some(battle) = state.battle.as_mut() {
        battle.message = if defeated {
            format!("{message} {enemy_name} fainted!")
        } else {
            message
        };
    }
    if all_enemies_defeated(state) {
        if let Some(battle) = state.battle.as_mut() {
            battle.stage = BattleStage::Victory;
        }
    } else {
        finish_actor_turn(state);
    }
    DispatchResult::changed_with(Effect::PlayAttackSound)
}

fn handle_ally_target_confirm(state: &mut AppState) -> DispatchResult<Effect> {
    let targets = living_party_indexes(state);
    if targets.is_empty() {
        if let Some(battle) = state.battle.as_mut() {
            battle.stage = BattleStage::Defeat;
            battle.message = "No one can fight!".to_string();
        }
        return DispatchResult::changed();
    }
    let (target_member_index, pending) = match state.battle.as_ref() {
        Some(battle) => {
            let slot = battle
                .ally_target_index
                .min(targets.len().saturating_sub(1));
            (targets[slot], battle.pending_action.clone())
        }
        None => return DispatchResult::unchanged(),
    };
    let pending = match pending {
        Some(pending) => pending,
        None => return DispatchResult::unchanged(),
    };
    let target_name = state
        .party
        .get(target_member_index)
        .map(|member| format_name(&member.info.name))
        .unwrap_or_else(|| "Ally".to_string());
    match pending.kind {
        PendingActionKind::Item { kind } => {
            if !take_item(state, kind) {
                if let Some(battle) = state.battle.as_mut() {
                    battle.stage = BattleStage::CommandMenu;
                    battle.message = "No items left.".to_string();
                }
                return DispatchResult::changed();
            }
            let healed = heal_party_member(state, target_member_index, kind.heal_amount());
            if let Some(battle) = state.battle.as_mut() {
                battle.message = format!(
                    "Used {} on {}. Restored {} HP.",
                    kind.label(),
                    target_name,
                    healed
                );
            }
        }
        PendingActionKind::AbilityHeal { amount } => {
            let healed = heal_party_member(state, target_member_index, amount);
            let ability_name = pending
                .ability_name
                .unwrap_or_else(|| "Ability".to_string());
            if let Some(battle) = state.battle.as_mut() {
                battle.message = format!("{ability_name} restored {healed} HP to {target_name}.");
            }
        }
        PendingActionKind::AbilityGuard { reduction_pct } => {
            if let Some(slot) = state.battle.as_mut().and_then(|battle| {
                battle
                    .party
                    .iter_mut()
                    .find(|slot| slot.member_index == target_member_index)
            }) {
                slot.guard_active = true;
                slot.guard_pct = reduction_pct.min(90);
            }
            let ability_name = pending.ability_name.unwrap_or_else(|| "Guard".to_string());
            if let Some(battle) = state.battle.as_mut() {
                battle.message = format!(
                    "{} shielded {} (-{}% until next turn).",
                    ability_name, target_name, reduction_pct
                );
            }
        }
        _ => return DispatchResult::unchanged(),
    }
    finish_actor_turn(state);
    DispatchResult::changed()
}

fn tick_battle_atb(state: &mut AppState) -> (bool, Option<Effect>) {
    let Some(stage) = state.battle.as_ref().map(|battle| battle.stage) else {
        return (false, None);
    };
    if !matches!(stage, BattleStage::Waiting) {
        return (false, None);
    }

    let party_speeds: Vec<u16> = state
        .party
        .iter()
        .map(|member| calc_stat(member.info.speed, member.level.max(1)))
        .collect();
    let mut changed = false;
    let mut ready_candidates: Vec<(BattleActor, u16, u16, usize)> = Vec::new();

    if let Some(battle) = state.battle.as_mut() {
        for slot in &mut battle.party {
            if slot.hp == 0 || slot.ready {
                continue;
            }
            if matches!(battle.current_actor, Some(BattleActor::Party(idx)) if idx == slot.member_index)
            {
                continue;
            }
            let speed = party_speeds
                .get(slot.member_index)
                .copied()
                .unwrap_or_else(|| calc_stat(10, 1));
            let gain = atb_gain(speed, (slot.member_index as u16) % 3);
            let new_atb = slot.atb as u32 + gain as u32;
            if new_atb >= ATB_MAX as u32 {
                let overflow = (new_atb - ATB_MAX as u32) as u16;
                slot.atb = ATB_MAX;
                slot.ready = true;
                ready_candidates.push((
                    BattleActor::Party(slot.member_index),
                    overflow,
                    speed,
                    slot.member_index,
                ));
            } else {
                slot.atb = new_atb as u16;
            }
            changed = true;
        }
        for enemy in &mut battle.enemies {
            if enemy.defeated || enemy.ready {
                continue;
            }
            if matches!(battle.current_actor, Some(BattleActor::Enemy(idx)) if idx == enemy.enemy_index)
            {
                continue;
            }
            let speed = calc_stat(enemy.speed.max(1), enemy.level.max(1));
            let gain = atb_gain(speed, enemy_tempo_bonus(enemy));
            let new_atb = enemy.atb as u32 + gain as u32;
            if new_atb >= ATB_MAX as u32 {
                let overflow = (new_atb - ATB_MAX as u32) as u16;
                enemy.atb = ATB_MAX;
                enemy.ready = true;
                ready_candidates.push((
                    BattleActor::Enemy(enemy.enemy_index),
                    overflow,
                    speed,
                    enemy.enemy_index,
                ));
            } else {
                enemy.atb = new_atb as u16;
            }
            changed = true;
        }
        ready_candidates.sort_by(|a, b| {
            b.1.cmp(&a.1)
                .then_with(|| b.2.cmp(&a.2))
                .then_with(|| a.3.cmp(&b.3))
        });
        for (actor, _, _, _) in ready_candidates {
            if !battle.ready_queue.contains(&actor) {
                battle.ready_queue.push(actor);
            }
        }
    }

    let mut effect = None;
    if let Some(start_effect) = maybe_start_next_actor(state) {
        effect = start_effect;
        changed = true;
    }
    (changed, effect)
}

fn maybe_start_next_actor(state: &mut AppState) -> Option<Option<Effect>> {
    let actor = pop_next_ready_actor(state)?;
    if let Some(battle) = state.battle.as_mut() {
        battle.current_actor = Some(actor);
        battle.pending_action = None;
    }
    match actor {
        BattleActor::Party(member_index) => {
            let slot = state.battle.as_mut().and_then(|battle| {
                battle
                    .party
                    .iter_mut()
                    .find(|slot| slot.member_index == member_index)
            });
            let Some(slot) = slot else {
                finish_actor_turn(state);
                return Some(None);
            };
            if slot.hp == 0 {
                finish_actor_turn(state);
                return Some(None);
            }
            slot.guard_active = false;
            slot.guard_pct = 0;
            if let Some(member) = state.party.get_mut(member_index) {
                if member.ability_cd > 0 {
                    member.ability_cd = member.ability_cd.saturating_sub(1);
                }
            }
            state.active_party_index = member_index.min(state.party.len().saturating_sub(1));
            sync_legacy_from_active(state);
            if let Some(battle) = state.battle.as_mut() {
                battle.stage = BattleStage::CommandMenu;
                battle.menu_index = 0;
            }
            set_battle_menu_prompt(state);
            Some(load_player_sprite_for_active(state))
        }
        BattleActor::Enemy(enemy_index) => {
            resolve_enemy_turn(state, enemy_index);
            Some(None)
        }
    }
}

fn pop_next_ready_actor(state: &mut AppState) -> Option<BattleActor> {
    let battle = state.battle.as_mut()?;
    while !battle.ready_queue.is_empty() {
        let actor = battle.ready_queue.remove(0);
        let alive = match actor {
            BattleActor::Party(member_index) => battle
                .party
                .iter()
                .find(|slot| slot.member_index == member_index)
                .map(|slot| slot.hp > 0)
                .unwrap_or(false),
            BattleActor::Enemy(enemy_index) => battle
                .enemies
                .iter()
                .find(|enemy| enemy.enemy_index == enemy_index)
                .map(|enemy| !enemy.defeated && enemy.hp > 0)
                .unwrap_or(false),
        };
        if alive {
            return Some(actor);
        }
    }
    None
}

fn resolve_enemy_turn(state: &mut AppState, enemy_index: usize) {
    let targets = living_party_indexes(state);
    if targets.is_empty() {
        if let Some(battle) = state.battle.as_mut() {
            battle.stage = BattleStage::Defeat;
            battle.message = "No one can fight!".to_string();
        }
        return;
    }
    let target_slot = targets[(next_rand(state) as usize) % targets.len()];
    let (enemy_name, enemy_level, enemy_kind, enemy_attack) = match state.battle.as_ref() {
        Some(battle) => battle
            .enemies
            .iter()
            .find(|enemy| enemy.enemy_index == enemy_index)
            .map(|enemy| {
                (
                    format_name(&enemy.species),
                    enemy.level,
                    battle.kind,
                    calc_stat(enemy.attack.max(1), enemy.level.max(1)),
                )
            })
            .unwrap_or_else(|| ("Enemy".to_string(), 5, BattleKind::Wild, calc_stat(10, 5))),
        None => return,
    };
    let target_name = state
        .party
        .get(target_slot)
        .map(|member| format_name(&member.info.name))
        .unwrap_or_else(|| "ally".to_string());
    let mut damage = calc_damage(
        state,
        enemy_level,
        enemy_attack,
        party_defense_value(state, target_slot),
    );
    if let Some(slot) = state.battle.as_mut().and_then(|battle| {
        battle
            .party
            .iter_mut()
            .find(|slot| slot.member_index == target_slot)
    }) {
        if slot.guard_active && slot.guard_pct > 0 {
            let reduction = (damage as u32 * slot.guard_pct as u32) / 100;
            damage = damage.saturating_sub(reduction as u16);
        }
    }
    apply_damage_to_party(state, target_slot, damage);
    if let Some(battle) = state.battle.as_mut() {
        let prefix = match enemy_kind {
            BattleKind::Boss => "Boss",
            BattleKind::Wild => "Wild",
        };
        battle.message = format!("{prefix} {enemy_name} hit {target_name} for {damage}!");
    }
    if all_party_defeated(state) {
        if let Some(battle) = state.battle.as_mut() {
            battle.stage = BattleStage::Defeat;
            battle.message = "Your party was defeated!".to_string();
        }
    } else {
        finish_actor_turn(state);
    }
}

fn finish_actor_turn(state: &mut AppState) {
    let actor = state
        .battle
        .as_ref()
        .and_then(|battle| battle.current_actor)
        .unwrap_or(BattleActor::Party(state.active_party_index));
    if let Some(battle) = state.battle.as_mut() {
        match actor {
            BattleActor::Party(member_index) => {
                if let Some(slot) = battle
                    .party
                    .iter_mut()
                    .find(|slot| slot.member_index == member_index)
                {
                    slot.ready = false;
                    slot.atb = 0;
                }
            }
            BattleActor::Enemy(enemy_index) => {
                if let Some(enemy) = battle
                    .enemies
                    .iter_mut()
                    .find(|enemy| enemy.enemy_index == enemy_index)
                {
                    enemy.ready = false;
                    enemy.atb = 0;
                }
            }
        }
        battle.current_actor = None;
        battle.pending_action = None;
        if !matches!(
            battle.stage,
            BattleStage::Victory | BattleStage::Escape | BattleStage::Defeat
        ) {
            battle.stage = BattleStage::Waiting;
        }
    }
}

fn living_enemy_indexes(state: &AppState) -> Vec<usize> {
    state
        .battle
        .as_ref()
        .map(|battle| {
            battle
                .enemies
                .iter()
                .filter(|enemy| !enemy.defeated && enemy.hp > 0)
                .map(|enemy| enemy.enemy_index)
                .collect()
        })
        .unwrap_or_default()
}

fn living_party_indexes(state: &AppState) -> Vec<usize> {
    state
        .battle
        .as_ref()
        .map(|battle| {
            battle
                .party
                .iter()
                .filter(|slot| slot.hp > 0)
                .map(|slot| slot.member_index)
                .collect()
        })
        .unwrap_or_default()
}

fn all_enemies_defeated(state: &AppState) -> bool {
    state
        .battle
        .as_ref()
        .map(|battle| {
            battle
                .enemies
                .iter()
                .all(|enemy| enemy.defeated || enemy.hp == 0)
        })
        .unwrap_or(false)
}

fn all_party_defeated(state: &AppState) -> bool {
    state
        .battle
        .as_ref()
        .map(|battle| battle.party.iter().all(|slot| slot.hp == 0))
        .unwrap_or(false)
}

fn apply_damage_to_enemy(state: &mut AppState, enemy_index: usize, damage: u16) -> bool {
    let mut defeated = false;
    if let Some(enemy) = state.battle.as_mut().and_then(|battle| {
        battle
            .enemies
            .iter_mut()
            .find(|enemy| enemy.enemy_index == enemy_index)
    }) {
        enemy.hp = enemy.hp.saturating_sub(damage);
        enemy.defeated = enemy.hp == 0;
        defeated = enemy.defeated;
    }
    defeated
}

fn apply_damage_to_party(state: &mut AppState, member_index: usize, damage: u16) {
    if let Some(member) = state.party.get_mut(member_index) {
        member.hp = member.hp.saturating_sub(damage);
    }
    if let Some(slot) = state.battle.as_mut().and_then(|battle| {
        battle
            .party
            .iter_mut()
            .find(|slot| slot.member_index == member_index)
    }) {
        slot.hp = slot.hp.saturating_sub(damage);
    }
}

fn heal_party_member(state: &mut AppState, member_index: usize, amount: u16) -> u16 {
    let mut healed = 0;
    if let Some(member) = state.party.get_mut(member_index) {
        let max_hp = calc_hp(member.info.hp, member.level).max(1);
        let before = member.hp;
        member.hp = member.hp.saturating_add(amount).min(max_hp);
        healed = member.hp.saturating_sub(before);
    }
    if let Some(slot) = state.battle.as_mut().and_then(|battle| {
        battle
            .party
            .iter_mut()
            .find(|slot| slot.member_index == member_index)
    }) {
        slot.hp = slot.hp.saturating_add(healed).min(slot.hp_max);
    }
    healed
}

fn party_defense_value(state: &AppState, member_index: usize) -> u16 {
    state
        .party
        .get(member_index)
        .map(|member| calc_stat(member.info.defense, member.level.max(1)))
        .unwrap_or_else(|| calc_stat(10, 1))
}

fn enemy_defense_value(state: &AppState, enemy_index: usize) -> u16 {
    state
        .battle
        .as_ref()
        .and_then(|battle| {
            battle
                .enemies
                .iter()
                .find(|enemy| enemy.enemy_index == enemy_index)
        })
        .map(|enemy| calc_stat(enemy.defense.max(1), enemy.level.max(1)))
        .unwrap_or_else(|| calc_stat(10, 5))
}

fn run_chance_pct(state: &AppState) -> i32 {
    let Some(battle) = state.battle.as_ref() else {
        return 35;
    };
    let party_alive = battle
        .party
        .iter()
        .filter(|slot| slot.hp > 0)
        .count()
        .max(1) as i32;
    let enemy_alive = battle
        .enemies
        .iter()
        .filter(|enemy| !enemy.defeated && enemy.hp > 0)
        .count()
        .max(1) as i32;
    let party_speed_sum: i32 = battle
        .party
        .iter()
        .filter(|slot| slot.hp > 0)
        .map(|slot| {
            state
                .party
                .get(slot.member_index)
                .map(|member| calc_stat(member.info.speed, member.level.max(1)) as i32)
                .unwrap_or(calc_stat(10, 1) as i32)
        })
        .sum();
    let enemy_speed_sum: i32 = battle
        .enemies
        .iter()
        .filter(|enemy| !enemy.defeated && enemy.hp > 0)
        .map(|enemy| calc_stat(enemy.speed.max(1), enemy.level.max(1)) as i32)
        .sum();
    let avg_party = party_speed_sum / party_alive;
    let avg_enemy = enemy_speed_sum / enemy_alive;
    (35 + (battle.run_attempts as i32 * 10) + ((avg_party - avg_enemy) / 4)).clamp(10, 95)
}

fn build_battle_party_states(state: &mut AppState) -> Vec<BattlePartyState> {
    let mut slots = Vec::with_capacity(state.party.len());
    for (member_index, member) in state.party.iter_mut().enumerate() {
        let hp_max = calc_hp(member.info.hp, member.level).max(1);
        if member.hp == 0 || member.hp > hp_max {
            member.hp = hp_max;
        }
        slots.push(BattlePartyState {
            member_index,
            hp: member.hp,
            hp_max,
            atb: 0,
            ready: false,
            guard_pct: 0,
            guard_active: false,
        });
    }
    slots
}

fn try_load_next_enemy_species(state: &mut AppState) -> Option<Effect> {
    let species = {
        let battle = state.battle.as_mut()?;
        let species = battle
            .enemies
            .iter()
            .find(|enemy| !enemy.info_loaded && !enemy.info_loading)
            .map(|enemy| enemy.species.clone())?;
        for enemy in &mut battle.enemies {
            if normalize_species(&enemy.species) == normalize_species(&species) {
                enemy.info_loading = true;
            }
        }
        species
    };
    Some(Effect::LoadPokemon {
        target: SpriteTarget::Enemy,
        name: species,
    })
}

fn ability_spec_for_member(state: &AppState, member_index: usize) -> Option<AbilitySpec> {
    let member = state.party.get(member_index)?;
    let ability_id = member.ability_id.as_ref()?;
    let scenario = state.scenario.as_ref()?;
    scenario
        .manifest
        .abilities
        .iter()
        .find(|ability| ability.id == *ability_id)
        .cloned()
}

fn cached_enemy_info(state: &AppState, species: &str) -> Option<crate::state::PokemonInfo> {
    let key = normalize_species(species);
    state.enemy_info_cache.get(&key).cloned().or_else(|| {
        state
            .enemy_info
            .as_ref()
            .filter(|info| normalize_species(&info.name) == key)
            .cloned()
    })
}

fn fallback_info_for_enemy(enemy: &BattleEnemyState) -> crate::state::PokemonInfo {
    let level = enemy.level.max(1) as i32;
    let hp_term = (enemy.hp_max as i32 - level - 10).max(1);
    let hp_base =
        (((hp_term * 100) / level).saturating_sub(31) / 2).clamp(1, u16::MAX as i32) as u16;
    crate::state::PokemonInfo {
        name: enemy.species.clone(),
        base_experience: enemy.base_experience.max(1),
        hp: hp_base.max(10),
        attack: enemy.attack.max(10),
        defense: enemy.defense.max(10),
        sp_attack: enemy.attack.max(10),
        sp_defense: enemy.defense.max(10),
        speed: enemy.speed.max(10),
        sprite_front_default: None,
        sprite_back_default: None,
        sprite_front_animated: None,
        sprite_back_animated: None,
    }
}

fn recruit_wild_victory_member(
    state: &mut AppState,
    battle: &crate::state::BattleState,
) -> Option<String> {
    if battle.kind != BattleKind::Wild || state.party.len() >= PARTY_LIMIT {
        return None;
    }
    let existing_species: std::collections::HashSet<String> = state
        .party
        .iter()
        .map(|member| normalize_species(&member.info.name))
        .collect();
    for avoid_duplicates in [true, false] {
        for enemy in battle
            .enemies
            .iter()
            .filter(|enemy| enemy.defeated || enemy.hp == 0)
        {
            let species_key = normalize_species(&enemy.species);
            if avoid_duplicates && existing_species.contains(&species_key) {
                continue;
            }
            let info = cached_enemy_info(state, &enemy.species)
                .unwrap_or_else(|| fallback_info_for_enemy(enemy));
            let level = enemy.level.max(1).min(MAX_LEVEL);
            let max_hp = calc_hp(info.hp, level).max(1);
            let ability_id = ability_id_for_species(state, &info.name);
            state.party.push(PartyMember {
                info: info.clone(),
                level,
                exp: exp_for_level(level),
                hp: max_hp,
                ability_id,
                ability_cd: 0,
            });
            ensure_party_sprites(state);
            return Some(format_name(&info.name));
        }
    }
    None
}

fn end_battle(state: &mut AppState) -> DispatchResult<Effect> {
    if let Some(battle) = state.battle.take() {
        let mut message = match battle.stage {
            BattleStage::Victory => battle.message.clone(),
            BattleStage::Escape => "Back on the route.".to_string(),
            BattleStage::Defeat => "You recovered and returned to the path.".to_string(),
            _ => "Battle ended.".to_string(),
        };
        for slot in &battle.party {
            if let Some(member) = state.party.get_mut(slot.member_index) {
                member.hp = slot.hp.min(slot.hp_max);
            }
        }
        match battle.stage {
            BattleStage::Victory => {
                for enemy in &battle.enemies {
                    if enemy.defeated || enemy.hp == 0 {
                        record_defeat(state, &enemy.species);
                    }
                }
                let mut relic_triggered = false;
                if battle.kind == BattleKind::Wild {
                    state.wild_wins = state.wild_wins.saturating_add(1);
                    if state.wild_wins == RELIC_WINS && !state.has_relic {
                        state.has_relic = true;
                        relic_triggered = true;
                    }
                }
                if battle.kind == BattleKind::Boss {
                    state.boss_defeated = true;
                    message = "Demo complete! You beat the boss!".to_string();
                } else {
                    let total_xp: u32 = battle
                        .enemies
                        .iter()
                        .map(|enemy| {
                            ((enemy.base_experience as u32 * enemy.level as u32) / 7).max(1)
                        })
                        .sum();
                    let (awarded, levels) = award_group_exp(state, total_xp);
                    if levels.is_empty() {
                        message = format!("Party won! Gained {} XP.", awarded);
                    } else {
                        message = format!(
                            "Party won! Gained {} XP. Level ups: {}.",
                            awarded,
                            levels.join(", ")
                        );
                    }
                    if let Some(joined) = recruit_wild_victory_member(state, &battle) {
                        message = format!("{message} {joined} joined your party!");
                    }
                }
                if relic_triggered && battle.kind != BattleKind::Boss {
                    message = format!("{message} You found a relic!");
                }
            }
            BattleStage::Escape => {}
            BattleStage::Defeat => {
                for member in &mut state.party {
                    member.hp = calc_hp(member.info.hp, member.level).max(1);
                }
            }
            _ => {}
        }
        if state.active_party_index >= state.party.len() {
            state.active_party_index = 0;
        }
        sync_legacy_from_active(state);
        push_message(state, message);
    }
    state.mode = GameMode::Overworld;
    state.enemy_info = None;
    state.enemy_sprite.reset();
    state.enemy_sprites.clear();
    state.enemy_sprite_loading_species = None;
    state.enemy_info_cache.clear();
    state.steps_since_encounter = 0;
    DispatchResult::changed()
}

fn pokemon_loaded(
    state: &mut AppState,
    target: SpriteTarget,
    info: crate::state::PokemonInfo,
) -> DispatchResult<Effect> {
    match target {
        SpriteTarget::Player => {
            state.player_info = Some(info.clone());
            let ability_id = ability_id_for_species(state, &info.name);
            if state.party.is_empty() {
                let level = state.player_level.max(1);
                let max_hp = calc_hp(info.hp, level).max(1);
                let exp = state.player_exp.max(exp_for_level(level));
                state.party.push(PartyMember {
                    info: info.clone(),
                    level,
                    exp,
                    hp: max_hp,
                    ability_id,
                    ability_cd: 0,
                });
                state.active_party_index = 0;
            } else if let Some(member) = state.active_member_mut() {
                member.info = info.clone();
                let max_hp = calc_hp(member.info.hp, member.level).max(1);
                if member.hp == 0 || member.hp > max_hp {
                    member.hp = max_hp;
                }
                if member.ability_id.is_none() {
                    member.ability_id = ability_id;
                }
            }
            sync_legacy_from_active(state);
            if let Some(sprite_url) = sprite_url_for(&info, target) {
                state.player_sprite.loading = true;
                return DispatchResult::changed_with(Effect::LoadSprite {
                    target,
                    url: sprite_url,
                });
            }
            state.player_sprite.loading = false;
        }
        SpriteTarget::Enemy => {
            let species_key = normalize_species(&info.name);
            state
                .enemy_info_cache
                .insert(species_key.clone(), info.clone());
            if let Some(battle) = state.battle.as_mut() {
                for enemy in &mut battle.enemies {
                    if normalize_species(&enemy.species) != species_key {
                        continue;
                    }
                    let old_max = enemy.hp_max.max(1);
                    let hp_ratio = enemy.hp as f32 / old_max as f32;
                    let new_max = calc_hp(info.hp, enemy.level).max(1);
                    let new_hp = ((new_max as f32) * hp_ratio).round() as u16;
                    enemy.hp_max = new_max;
                    enemy.hp = new_hp.max(1).min(new_max);
                    enemy.attack = info.attack.max(1);
                    enemy.defense = info.defense.max(1);
                    enemy.speed = info.speed.max(1);
                    enemy.base_experience = info.base_experience.max(1);
                    enemy.info_loaded = true;
                    enemy.info_loading = false;
                }
            }
            if state.enemy_info.is_none() {
                state.enemy_info = Some(info.clone());
            }
            let mut effects = Vec::new();
            if let Some(effect) = try_load_next_enemy_sprite(state) {
                effects.push(effect);
            }
            if let Some(effect) = try_load_next_enemy_species(state) {
                effects.push(effect);
            }
            return match effects.len() {
                0 => DispatchResult::changed(),
                1 => DispatchResult::changed_with(effects.remove(0)),
                _ => DispatchResult::changed_with_many(effects),
            };
        }
    }
    DispatchResult::changed()
}

fn pokemon_error(
    state: &mut AppState,
    target: SpriteTarget,
    name: &str,
    error: &str,
) -> DispatchResult<Effect> {
    match target {
        SpriteTarget::Player => {
            push_message(state, format!("Player load error: {error}"));
            state.player_sprite.loading = false;
        }
        SpriteTarget::Enemy => {
            if let Some(battle) = state.battle.as_mut() {
                for enemy in &mut battle.enemies {
                    if normalize_species(&enemy.species) == normalize_species(name) {
                        enemy.info_loading = false;
                        enemy.info_loaded = true;
                    }
                }
                battle.message = format!("{} data unavailable.", format_name(name));
            }
            state.enemy_sprite.loading = false;
            if let Some(effect) = try_load_next_enemy_species(state) {
                return DispatchResult::changed_with(effect);
            }
        }
    }
    DispatchResult::changed()
}

fn sprite_loaded(
    state: &mut AppState,
    target: SpriteTarget,
    sprite: crate::sprite::SpriteData,
) -> DispatchResult<Effect> {
    match target {
        SpriteTarget::Player => {
            state.player_sprite.sprite = Some(sprite.clone());
            state.player_sprite.sprite_flipped = Some(sprite.flipped());
            state.player_sprite.frame_index = 0;
            state.player_sprite.frame_tick = 0;
            state.player_sprite.loading = false;
            DispatchResult::changed()
        }
        SpriteTarget::Enemy => {
            state.enemy_sprite.sprite = Some(sprite.clone());
            state.enemy_sprite.sprite_flipped = Some(sprite.flipped());
            state.enemy_sprite.frame_index = 0;
            state.enemy_sprite.frame_tick = 0;
            state.enemy_sprite.loading = false;
            if let Some(species_key) = state.enemy_sprite_loading_species.take() {
                ensure_enemy_sprites(state);
                if let Some(battle) = state.battle.as_ref() {
                    for enemy in &battle.enemies {
                        if normalize_species(&enemy.species) != species_key {
                            continue;
                        }
                        if let Some(slot) = state.enemy_sprites.get_mut(enemy.enemy_index) {
                            slot.sprite = Some(sprite.clone());
                            slot.sprite_flipped = Some(sprite.flipped());
                            slot.frame_index = 0;
                            slot.frame_tick = 0;
                            slot.loading = false;
                        }
                    }
                }
            }
            if let Some(effect) = try_load_next_enemy_sprite(state) {
                DispatchResult::changed_with(effect)
            } else {
                DispatchResult::changed()
            }
        }
    }
}

fn sprite_error(state: &mut AppState, target: SpriteTarget, error: &str) -> DispatchResult<Effect> {
    match target {
        SpriteTarget::Player => {
            push_message(state, format!("Player sprite error: {error}"));
            state.player_sprite.loading = false;
            DispatchResult::changed()
        }
        SpriteTarget::Enemy => {
            if let Some(battle) = state.battle.as_mut() {
                battle.message = format!("Sprite error: {error}");
            }
            if let Some(species_key) = state.enemy_sprite_loading_species.take() {
                ensure_enemy_sprites(state);
                if let Some(battle) = state.battle.as_ref() {
                    for enemy in &battle.enemies {
                        if normalize_species(&enemy.species) != species_key {
                            continue;
                        }
                        if let Some(slot) = state.enemy_sprites.get_mut(enemy.enemy_index) {
                            slot.loading = false;
                        }
                    }
                }
            }
            state.enemy_sprite.loading = false;
            if let Some(effect) = try_load_next_enemy_sprite(state) {
                DispatchResult::changed_with(effect)
            } else {
                DispatchResult::changed()
            }
        }
    }
}

fn sprite_url_for(info: &crate::state::PokemonInfo, _target: SpriteTarget) -> Option<String> {
    // Always use front sprites - looks better on overworld map
    info.sprite_front_animated
        .clone()
        .or_else(|| info.sprite_front_default.clone())
}

fn tick_animation(state: &mut AppState) -> DispatchResult<Effect> {
    state.tick = state.tick.wrapping_add(1);
    let mut changed = tick_messages(state);
    let mut effects = Vec::new();
    let mut sprite_changed = advance_sprite(&mut state.enemy_sprite);

    if state.mode != GameMode::Overworld {
        sprite_changed = advance_sprite(&mut state.player_sprite) || sprite_changed;
    } else if state.player_sprite.frame_index != 0 || state.player_sprite.frame_tick != 0 {
        state.player_sprite.frame_index = 0;
        state.player_sprite.frame_tick = 0;
        sprite_changed = true;
    }

    ensure_party_sprites(state);
    for sprite in &mut state.party_sprites {
        sprite_changed = advance_sprite(sprite) || sprite_changed;
    }
    ensure_enemy_sprites(state);
    for sprite in &mut state.enemy_sprites {
        sprite_changed = advance_sprite(sprite) || sprite_changed;
    }

    // Advance starter preview sprite if in pokemon select screen
    if let Some(select) = state.pokemon_select.as_mut() {
        sprite_changed = advance_sprite(&mut select.preview_sprite) || sprite_changed;
    }

    if sprite_changed {
        changed = true;
    }

    let (battle_changed, battle_effect) = tick_battle_atb(state);
    if battle_changed {
        changed = true;
    }
    if let Some(effect) = battle_effect {
        effects.push(effect);
    }

    if let Some(effect) = maybe_request_party_sprite(state) {
        effects.push(effect);
    }
    if let Some(effect) = try_load_next_enemy_sprite(state) {
        effects.push(effect);
    }
    match effects.len() {
        0 => {}
        1 => return DispatchResult::changed_with(effects.remove(0)),
        _ => return DispatchResult::changed_with_many(effects),
    }
    if changed {
        DispatchResult::changed()
    } else {
        DispatchResult::unchanged()
    }
}

fn advance_sprite(sprite: &mut crate::state::SpriteState) -> bool {
    let Some(data) = sprite.sprite.as_ref() else {
        return false;
    };
    if data.frames.len() <= 1 {
        return false;
    }
    const FRAME_STEP: u64 = 1;
    sprite.frame_tick = sprite.frame_tick.wrapping_add(1);
    if sprite.frame_tick % FRAME_STEP == 0 {
        sprite.frame_index = (sprite.frame_index + 1) % data.frames.len();
        return true;
    }
    false
}

fn next_rand(state: &mut AppState) -> u32 {
    state.rng_seed = state
        .rng_seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1);
    (state.rng_seed >> 32) as u32
}

fn push_message(state: &mut AppState, message: impl Into<String>) {
    let msg = message.into();
    if msg.is_empty() {
        return;
    }
    if state.message.is_none() {
        state.message = Some(msg);
    } else {
        state.message_queue.push_back(msg);
    }
}

fn tick_messages(_state: &mut AppState) -> bool {
    false
}

fn message_next(state: &mut AppState) -> DispatchResult<Effect> {
    if state.message.is_none() {
        return DispatchResult::unchanged();
    }
    if let Some(next) = state.message_queue.pop_front() {
        state.message = Some(next);
    } else {
        state.message = None;
    }
    DispatchResult::changed()
}

fn scenario_starters(state: &AppState) -> Vec<String> {
    if let Some(scenario) = state
        .scenario
        .as_ref()
        .filter(|scenario| !scenario.manifest.starters.is_empty())
    {
        scenario.manifest.starters.clone()
    } else {
        DEFAULT_STARTERS.iter().map(|s| s.to_string()).collect()
    }
}

fn normalize_species(name: &str) -> String {
    name.trim().to_ascii_lowercase()
}

fn ability_id_for_species(state: &AppState, name: &str) -> Option<String> {
    let scenario = state.scenario.as_ref()?;
    ability_id_from_list(&scenario.manifest.species_abilities, name)
}

fn ability_id_from_list(list: &[crate::scenario::SpeciesAbility], target: &str) -> Option<String> {
    let target = normalize_species(target);
    list.iter()
        .find(|entry| normalize_species(&entry.species) == target)
        .map(|entry| entry.ability_id.clone())
}

fn ensure_party_sprites(state: &mut AppState) {
    let len = state.party.len();
    if state.party_sprites.len() < len {
        state
            .party_sprites
            .extend((0..(len - state.party_sprites.len())).map(|_| SpriteState::default()));
    } else if state.party_sprites.len() > len {
        state.party_sprites.truncate(len);
    }
}

fn ensure_enemy_sprites(state: &mut AppState) {
    let len = state
        .battle
        .as_ref()
        .map(|battle| battle.enemies.len())
        .unwrap_or(0);
    if len == 0 {
        state.enemy_sprites.clear();
        state.enemy_sprite_loading_species = None;
        return;
    }
    if state.enemy_sprites.len() < len {
        state
            .enemy_sprites
            .extend((0..(len - state.enemy_sprites.len())).map(|_| SpriteState::default()));
    } else if state.enemy_sprites.len() > len {
        state.enemy_sprites.truncate(len);
    }
}

fn try_load_next_enemy_sprite(state: &mut AppState) -> Option<Effect> {
    if state.enemy_sprite_loading_species.is_some() {
        return None;
    }
    ensure_enemy_sprites(state);
    let species_key = {
        let battle = state.battle.as_ref()?;
        battle
            .enemies
            .iter()
            .find(|enemy| {
                let idx = enemy.enemy_index;
                let needs_load = state
                    .enemy_sprites
                    .get(idx)
                    .map(|slot| slot.sprite.is_none() && !slot.loading)
                    .unwrap_or(false);
                if !needs_load {
                    return false;
                }
                let key = normalize_species(&enemy.species);
                state
                    .enemy_info_cache
                    .get(&key)
                    .and_then(|info| sprite_url_for(info, SpriteTarget::Enemy))
                    .is_some()
            })
            .map(|enemy| normalize_species(&enemy.species))?
    };
    let info = state.enemy_info_cache.get(&species_key)?;
    let url = sprite_url_for(info, SpriteTarget::Enemy)?;

    let indexes: Vec<usize> = state
        .battle
        .as_ref()
        .map(|battle| {
            battle
                .enemies
                .iter()
                .filter(|enemy| normalize_species(&enemy.species) == species_key)
                .map(|enemy| enemy.enemy_index)
                .collect()
        })
        .unwrap_or_default();
    for idx in indexes {
        if let Some(slot) = state.enemy_sprites.get_mut(idx) {
            slot.loading = true;
        }
    }
    state.enemy_sprite_loading_species = Some(species_key);
    state.enemy_sprite.loading = true;
    Some(Effect::LoadSprite {
        target: SpriteTarget::Enemy,
        url,
    })
}

fn maybe_request_party_sprite(state: &mut AppState) -> Option<Effect> {
    if state.party.is_empty() {
        return None;
    }
    ensure_party_sprites(state);
    for idx in 0..state.party.len() {
        let needs_load = state
            .party_sprites
            .get(idx)
            .map(|sprite| sprite.sprite.is_none() && !sprite.loading)
            .unwrap_or(false);
        if !needs_load {
            continue;
        }
        let url = state
            .party
            .get(idx)
            .and_then(|member| sprite_url_for(&member.info, SpriteTarget::Player))?;
        if let Some(sprite) = state.party_sprites.get_mut(idx) {
            sprite.loading = true;
        }
        return Some(Effect::LoadPartySprite { index: idx, url });
    }
    None
}

fn roll_enemy_level(state: &mut AppState) -> u8 {
    let base = state.active_level().max(2) as i16;
    let offset = (next_rand(state) % 5) as i16 - 2;
    let level = (base + offset).clamp(2, MAX_LEVEL as i16);
    level as u8
}

fn nearest_unused_level(base: u8, used: &[u8]) -> u8 {
    if !used.contains(&base) {
        return base;
    }
    for delta in 1..=8 {
        let up = (base as i16 + delta).clamp(2, MAX_LEVEL as i16) as u8;
        if !used.contains(&up) {
            return up;
        }
        let down = (base as i16 - delta).clamp(2, MAX_LEVEL as i16) as u8;
        if !used.contains(&down) {
            return down;
        }
    }
    base
}

fn roll_enemy_levels_for_group(state: &mut AppState, count: usize) -> Vec<u8> {
    let mut levels = Vec::with_capacity(count);
    for _ in 0..count {
        let mut selected = None;
        for _ in 0..10 {
            let level = roll_enemy_level(state);
            if !levels.contains(&level) {
                selected = Some(level);
                break;
            }
        }
        let level =
            selected.unwrap_or_else(|| nearest_unused_level(roll_enemy_level(state), &levels));
        levels.push(level);
    }
    levels
}

fn boss_level(state: &AppState) -> u8 {
    let base = state.active_level().max(5) as i16;
    let level = (base + 3).clamp(5, MAX_LEVEL as i16);
    level as u8
}

fn calc_damage(state: &mut AppState, level: u8, attack: u16, defense: u16) -> u16 {
    let level = level.max(1) as u32;
    let attack = attack.max(1) as u32;
    let defense = defense.max(1) as u32;
    let base = (((2 * level / 5 + 2) * MOVE_POWER * attack) / defense) / 50 + 2;
    let variance = 85 + (next_rand(state) % 16); // 85..=100
    let damage = base * variance / 100;
    damage.max(1) as u16
}

fn atb_gain(speed: u16, tempo_bonus: u16) -> u16 {
    ATB_BASE_GAIN
        .saturating_add(speed / ATB_SPEED_DIV.max(1))
        .saturating_add(tempo_bonus)
}

fn enemy_tempo_bonus(enemy: &BattleEnemyState) -> u16 {
    enemy
        .species
        .bytes()
        .fold(0u16, |acc, byte| acc.wrapping_add(byte as u16))
        % 5
}

fn seed_initial_atb(
    state: &mut AppState,
    party: &mut [BattlePartyState],
    enemies: &mut [BattleEnemyState],
) {
    for slot in party {
        // Give party a slight head start so the first command is readable.
        let base: u16 = if slot.member_index == state.active_party_index {
            220
        } else {
            160
        };
        let roll = (next_rand(state) % 260) as u16;
        slot.atb = base.saturating_add(roll).min(ATB_MAX.saturating_sub(1));
        slot.ready = false;
    }
    for enemy in enemies {
        let roll = (next_rand(state) % 240) as u16;
        enemy.atb = roll.min(ATB_MAX.saturating_sub(1));
        enemy.ready = false;
    }
}

fn collect_pickup(state: &mut AppState, x: u16, y: u16) {
    if let Some(index) = state.pickups.iter().position(|p| p.x == x && p.y == y) {
        let pickup = state.pickups.remove(index);
        add_item_to_inventory(state, pickup.kind, pickup.qty);
        push_message(
            state,
            format!("Found {} x{}!", pickup.kind.label(), pickup.qty),
        );
    }
}

fn add_item_to_inventory(state: &mut AppState, kind: ItemKind, qty: u16) {
    if let Some(stack) = state.inventory.iter_mut().find(|stack| stack.kind == kind) {
        stack.qty = stack.qty.saturating_add(qty);
    } else {
        state.inventory.push(crate::state::ItemStack { kind, qty });
    }
}

fn ensure_pickups(state: &mut AppState) {
    if state.pickups.is_empty() {
        spawn_pickups(state);
    }
}

fn spawn_pickups(state: &mut AppState) {
    let (count, pool) = match state.scenario.as_ref() {
        Some(scenario) => (
            scenario.manifest.random_pickups.count as usize,
            scenario.manifest.random_pickups.pool.clone(),
        ),
        None => return,
    };
    if count == 0 {
        return;
    }
    if pool.is_empty() {
        return;
    }
    let (start_x, start_y) = state.map.start_pos();
    let mut occupied = std::collections::HashSet::new();
    occupied.insert((start_x, start_y));
    for pickup in &state.pickups {
        occupied.insert((pickup.x, pickup.y));
    }
    let mut attempts = 0;
    while state.pickups.len() < count && attempts < count * 200 {
        attempts += 1;
        let x = (next_rand(state) as u16) % state.map.width;
        let y = (next_rand(state) as u16) % state.map.height;
        if occupied.contains(&(x, y)) {
            continue;
        }
        let tile = state.map.tile(x, y);
        if !matches!(tile, Tile::Grass | Tile::Path | Tile::Sand) {
            continue;
        }
        if let Some(drop) = roll_pickup_from_pool(state, &pool) {
            state.pickups.push(Pickup {
                x,
                y,
                kind: drop.kind,
                qty: drop.qty.max(1),
            });
            occupied.insert((x, y));
        }
    }
}

fn roll_pickup_from_pool(
    state: &mut AppState,
    pool: &[crate::scenario::ItemDrop],
) -> Option<crate::scenario::ItemDrop> {
    let total: u32 = pool.iter().map(|drop| drop.weight as u32).sum();
    if total == 0 {
        return None;
    }
    let mut roll = next_rand(state) % total;
    for drop in pool {
        let weight = drop.weight as u32;
        if roll < weight {
            return Some(drop.clone());
        }
        roll = roll.saturating_sub(weight);
    }
    pool.first().cloned()
}

fn trigger_tile_events(state: &mut AppState, x: u16, y: u16) {
    let events = match state.scenario.as_ref() {
        Some(scenario) => scenario.manifest.events.clone(),
        None => return,
    };
    for event in events {
        if let ScenarioTrigger::OnEnterTile { x: tx, y: ty } = event.trigger {
            if tx == x && ty == y {
                if event.once && state.fired_event_ids.contains(&event.id) {
                    continue;
                }
                push_message(state, event.message.clone());
                if event.once {
                    state.fired_event_ids.insert(event.id.clone());
                }
            }
        }
    }
}

fn record_defeat(state: &mut AppState, enemy_name: &str) {
    let key = normalize_species(enemy_name);
    let entry = state.defeat_counts.entry(key.clone()).or_insert(0);
    *entry = entry.saturating_add(1);
    trigger_defeat_events(state, &key);
}

fn trigger_defeat_events(state: &mut AppState, species_key: &str) {
    let events = match state.scenario.as_ref() {
        Some(scenario) => scenario.manifest.events.clone(),
        None => return,
    };
    let count = state.defeat_counts.get(species_key).copied().unwrap_or(0);
    for event in events {
        if let ScenarioTrigger::OnDefeat {
            species,
            count: req,
        } = event.trigger
        {
            if normalize_species(&species) == species_key && count >= req {
                if event.once && state.fired_event_ids.contains(&event.id) {
                    continue;
                }
                push_message(state, event.message.clone());
                if event.once {
                    state.fired_event_ids.insert(event.id.clone());
                }
            }
        }
    }
}

fn apply_scenario(state: &mut AppState, scenario: ScenarioRuntime) {
    let species_abilities = scenario.manifest.species_abilities.clone();
    state.scenario = Some(scenario.clone());
    state.map = scenario.map.clone();
    if state.player.steps == 0 && state.mode == GameMode::MainMenu {
        let (start_x, start_y) = state.map.start_pos();
        state.player.x = start_x;
        state.player.y = start_y;
    }
    for member in &mut state.party {
        if member.ability_id.is_none() {
            member.ability_id = ability_id_from_list(&species_abilities, &member.info.name);
        }
    }
    ensure_pickups(state);
}

fn available_items(state: &AppState) -> Vec<(ItemKind, u16)> {
    state
        .inventory
        .iter()
        .filter(|stack| stack.qty > 0)
        .filter(|stack| matches!(stack.kind, ItemKind::Potion | ItemKind::SuperPotion))
        .map(|stack| (stack.kind, stack.qty))
        .collect()
}

fn take_item(state: &mut AppState, kind: ItemKind) -> bool {
    if let Some(stack) = state.inventory.iter_mut().find(|stack| stack.kind == kind) {
        if stack.qty > 0 {
            stack.qty = stack.qty.saturating_sub(1);
            return true;
        }
    }
    false
}

fn sync_legacy_from_active(state: &mut AppState) {
    let member = match state.active_member() {
        Some(member) => member.clone(),
        None => return,
    };
    state.player_info = Some(member.info.clone());
    state.player_level = member.level;
    state.player_exp = member.exp;
    state.player_hp = member.hp;
}

fn load_player_sprite_for_active(state: &mut AppState) -> Option<Effect> {
    let member = state.active_member()?;
    let url = sprite_url_for(&member.info, SpriteTarget::Player)?;
    state.player_sprite.loading = true;
    Some(Effect::LoadSprite {
        target: SpriteTarget::Player,
        url,
    })
}

fn award_group_exp(state: &mut AppState, total_exp: u32) -> (u32, Vec<String>) {
    let alive: Vec<usize> = state
        .party
        .iter()
        .enumerate()
        .filter(|(_, member)| member.hp > 0)
        .map(|(idx, _)| idx)
        .collect();
    if alive.is_empty() || total_exp == 0 {
        return (0, Vec::new());
    }
    let split = total_exp / alive.len() as u32;
    let remainder = total_exp % alive.len() as u32;
    let mut awarded: u32 = 0;
    let mut level_ups = Vec::new();
    for (pos, member_index) in alive.iter().enumerate() {
        let gain = split + if pos < remainder as usize { 1 } else { 0 };
        awarded = awarded.saturating_add(gain);
        let Some(member) = state.party.get_mut(*member_index) else {
            continue;
        };
        let old_max_hp = calc_hp(member.info.hp, member.level).max(1);
        let old_level = member.level;
        member.exp = member.exp.saturating_add(gain);
        while member.level < MAX_LEVEL
            && member.exp >= exp_for_level(member.level.saturating_add(1))
        {
            member.level = member.level.saturating_add(1);
        }
        let new_max_hp = calc_hp(member.info.hp, member.level).max(1);
        let hp_bonus = new_max_hp.saturating_sub(old_max_hp);
        member.hp = member.hp.saturating_add(hp_bonus).min(new_max_hp);
        if member.level > old_level {
            level_ups.push(format!(
                "{} Lv {}",
                format_name(&member.info.name),
                member.level
            ));
        }
    }
    (awarded, level_ups)
}

fn starting_inventory() -> Vec<crate::state::ItemStack> {
    vec![
        crate::state::ItemStack {
            kind: ItemKind::Potion,
            qty: 3,
        },
        crate::state::ItemStack {
            kind: ItemKind::SuperPotion,
            qty: 1,
        },
    ]
}

fn normalize_loaded_state(state: &mut AppState) {
    if state.player_level == 0 {
        state.player_level = 5;
    }
    if state.player_level > MAX_LEVEL {
        state.player_level = MAX_LEVEL;
    }
    state.pause_menu.is_open = false;
    let min_exp = exp_for_level(state.player_level);
    if state.player_exp < min_exp {
        state.player_exp = min_exp;
    }
    if state.inventory.is_empty() {
        state.inventory = starting_inventory();
    }

    if state.party.is_empty() {
        if let Some(info) = state.player_info.clone() {
            let level = state.player_level.max(1);
            let max_hp = calc_hp(info.hp, level).max(1);
            let hp = if state.player_hp == 0 || state.player_hp > max_hp {
                max_hp
            } else {
                state.player_hp
            };
            let exp = state.player_exp.max(exp_for_level(level));
            let ability_id = ability_id_for_species(state, &info.name);
            state.party.push(PartyMember {
                info,
                level,
                exp,
                hp,
                ability_id,
                ability_cd: 0,
            });
            state.active_party_index = 0;
        }
    }

    if !state.party.is_empty() {
        if state.active_party_index >= state.party.len() {
            state.active_party_index = 0;
        }
        let species_abilities = state
            .scenario
            .as_ref()
            .map(|scenario| scenario.manifest.species_abilities.clone())
            .unwrap_or_default();
        for member in &mut state.party {
            let max_hp = calc_hp(member.info.hp, member.level).max(1);
            if member.hp == 0 || member.hp > max_hp {
                member.hp = max_hp;
            }
            let min_exp = exp_for_level(member.level);
            if member.exp < min_exp {
                member.exp = min_exp;
            }
            if member.ability_id.is_none() {
                member.ability_id = ability_id_from_list(&species_abilities, &member.info.name);
            }
        }
        ensure_party_sprites(state);
        sync_legacy_from_active(state);
    } else {
        let max_hp = state.player_max_hp();
        if state.player_hp == 0 || state.player_hp > max_hp {
            state.player_hp = max_hp;
        }
    }
    if state.battle.is_some() {
        ensure_enemy_sprites(state);
        if state.enemy_info_cache.is_empty() {
            if let Some(info) = state.enemy_info.clone() {
                state
                    .enemy_info_cache
                    .insert(normalize_species(&info.name), info);
            }
        }
    } else {
        state.enemy_sprites.clear();
        state.enemy_sprite_loading_species = None;
    }

    if state.scenario.is_some() {
        ensure_pickups(state);
    }
}

fn format_name(name: &str) -> String {
    name.split('-')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => {
                    let rest = chars.as_str();
                    format!("{}{}", first.to_ascii_uppercase(), rest)
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn menu_confirm(state: &mut AppState) -> DispatchResult<Effect> {
    let Some(menu) = state.menu.as_ref() else {
        return DispatchResult::unchanged();
    };

    match menu.selected {
        0 => {
            // New Game -> Pokemon Select
            state.mode = GameMode::PokemonSelect;
            state.overworld_full_map = false;
            state.menu = None;
            let starters = scenario_starters(state);
            state.pokemon_select = Some(PokemonSelectState {
                starters: starters.clone(),
                selected: 0,
                preview_info: None,
                preview_sprite: SpriteState::default(),
            });
            // Load preview for first starter
            if let Some(first) = starters.first() {
                DispatchResult::changed_with(Effect::LoadStarterPreview {
                    name: first.clone(),
                })
            } else {
                DispatchResult::changed()
            }
        }
        1 if menu.has_save => {
            // Continue -> Load Game
            state.menu = None;
            DispatchResult::changed_with(Effect::LoadGame)
        }
        1 if !menu.has_save => {
            // No save, this shouldn't be selectable but handle gracefully
            push_message(state, "No save file found.");
            DispatchResult::changed()
        }
        2 | _ => {
            // Quit
            DispatchResult::unchanged() // Will be handled by UI to exit
        }
    }
}

fn starter_select(state: &mut AppState, index: usize) -> DispatchResult<Effect> {
    let Some(select) = state.pokemon_select.as_mut() else {
        return DispatchResult::unchanged();
    };

    if index >= select.starters.len() {
        return DispatchResult::unchanged();
    }

    if select.selected == index {
        return DispatchResult::unchanged();
    }

    select.selected = index;
    select.preview_info = None;
    select.preview_sprite.reset();
    select.preview_sprite.loading = true;

    DispatchResult::changed_with(Effect::LoadStarterPreview {
        name: select.starters[index].clone(),
    })
}

fn starter_confirm(state: &mut AppState) -> DispatchResult<Effect> {
    let Some(select) = state.pokemon_select.take() else {
        return DispatchResult::unchanged();
    };

    let chosen_name = select.starters.get(select.selected).cloned();
    let Some(name) = chosen_name else {
        return DispatchResult::unchanged();
    };

    // Start the game with chosen Pokemon
    state.mode = GameMode::Overworld;
    state.overworld_full_map = false;
    state.player_sprite.reset();
    state.player_sprite.loading = true;
    state.enemy_sprite.reset();
    state.enemy_info = None;
    state.battle = None;
    state.steps_since_encounter = 0;
    state.party.clear();
    state.party_sprites.clear();
    state.active_party_index = 0;
    state.player_info = None;
    state.player_level = 5;
    state.player_exp = exp_for_level(state.player_level);
    state.player_hp = 0;
    state.inventory = starting_inventory();
    state.message_queue.clear();
    state.message_timer = 0;
    state.wild_wins = 0;
    state.has_relic = false;
    state.boss_defeated = false;
    state.fired_event_ids.clear();
    state.defeat_counts.clear();
    state.pickups.clear();
    ensure_pickups(state);
    push_message(
        state,
        format!("You chose {}! Let's go!", format_name(&name)),
    );

    DispatchResult::changed_with(Effect::LoadPokemon {
        target: SpriteTarget::Player,
        name,
    })
}

fn pause_confirm(state: &mut AppState) -> DispatchResult<Effect> {
    match state.pause_menu.selected {
        0 => {
            // Resume
            state.pause_menu.is_open = false;
            DispatchResult::changed()
        }
        1 => {
            // Save Game
            DispatchResult::changed_with(Effect::SaveGame {
                state: Box::new(state.clone()),
            })
        }
        2 | _ => {
            // Quit to Menu
            state.pause_menu.is_open = false;
            state.overworld_full_map = false;
            state.mode = GameMode::MainMenu;
            state.menu = Some(MenuState {
                selected: 0,
                has_save: false,
            });
            state.battle = None;
            DispatchResult::changed_with(Effect::CheckSaveExists)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::PokemonInfo;
    use std::collections::HashSet;

    fn info(
        name: &str,
        hp: u16,
        attack: u16,
        defense: u16,
        speed: u16,
        base_exp: u16,
    ) -> PokemonInfo {
        PokemonInfo {
            name: name.to_string(),
            base_experience: base_exp,
            hp,
            attack,
            defense,
            sp_attack: attack,
            sp_defense: defense,
            speed,
            sprite_front_default: None,
            sprite_back_default: None,
            sprite_front_animated: None,
            sprite_back_animated: None,
        }
    }

    fn member(
        name: &str,
        level: u8,
        hp_base: u16,
        attack: u16,
        defense: u16,
        speed: u16,
        exp: u32,
    ) -> PartyMember {
        let info = info(name, hp_base, attack, defense, speed, 60);
        let hp = calc_hp(info.hp, level).max(1);
        PartyMember {
            info,
            level,
            exp,
            hp,
            ability_id: None,
            ability_cd: 0,
        }
    }

    fn test_state() -> AppState {
        let mut state = AppState::new();
        state.mode = GameMode::Overworld;
        state.party = vec![
            member("pikachu", 12, 35, 55, 35, 160, exp_for_level(12)),
            member("bulbasaur", 12, 45, 49, 49, 45, exp_for_level(12)),
        ];
        state.active_party_index = 0;
        sync_legacy_from_active(&mut state);
        state.rng_seed = 7;
        state
    }

    #[test]
    fn wild_and_boss_spawn_sizes_match_spec() {
        let mut state = test_state();
        let _ = start_wild_battle(&mut state);
        let wild_count = state
            .battle
            .as_ref()
            .map(|battle| battle.enemies.len())
            .unwrap_or(0);
        assert!((1..=3).contains(&wild_count));

        state.mode = GameMode::Overworld;
        state.battle = None;
        let _ = start_boss_battle(&mut state);
        let boss_count = state
            .battle
            .as_ref()
            .map(|battle| battle.enemies.len())
            .unwrap_or(0);
        assert_eq!(boss_count, 1);
    }

    #[test]
    fn wild_groups_use_distinct_levels_when_multi_spawn() {
        let mut state = test_state();
        let mut saw_multi = false;
        for seed in 1..50u64 {
            state.mode = GameMode::Overworld;
            state.battle = None;
            state.rng_seed = seed;
            let _ = start_wild_battle(&mut state);
            let levels: Vec<u8> = state
                .battle
                .as_ref()
                .map(|battle| battle.enemies.iter().map(|enemy| enemy.level).collect())
                .unwrap_or_default();
            if levels.len() > 1 {
                saw_multi = true;
                let unique: HashSet<u8> = levels.iter().copied().collect();
                assert_eq!(unique.len(), levels.len());
            }
        }
        assert!(saw_multi);
    }

    #[test]
    fn battle_starts_with_staggered_atb_values() {
        let mut state = test_state();
        state.rng_seed = 12345;
        let _ = start_battle(
            &mut state,
            vec![("zubat".to_string(), 9), ("oddish".to_string(), 10)],
            BattleKind::Wild,
        );
        let Some(battle) = state.battle.as_ref() else {
            panic!("battle should exist");
        };
        assert!(battle.party.iter().all(|slot| slot.atb > 0));
        let mut seen = HashSet::new();
        for slot in &battle.party {
            seen.insert(slot.atb);
        }
        for enemy in &battle.enemies {
            seen.insert(enemy.atb);
        }
        assert!(seen.len() > 2);
    }

    #[test]
    fn atb_prefers_faster_party_member() {
        let mut state = test_state();
        let _ = start_battle(
            &mut state,
            vec![("zubat".to_string(), 10)],
            BattleKind::Wild,
        );
        let _ = battle_confirm(&mut state); // Intro -> Waiting

        for _ in 0..120 {
            let _ = tick_battle_atb(&mut state);
            if state
                .battle
                .as_ref()
                .map(|battle| battle.stage == BattleStage::CommandMenu)
                .unwrap_or(false)
            {
                break;
            }
        }
        let stage = state
            .battle
            .as_ref()
            .map(|battle| battle.stage)
            .unwrap_or(BattleStage::Defeat);
        assert_eq!(stage, BattleStage::CommandMenu);
        assert_eq!(state.active_party_index, 0);
    }

    #[test]
    fn atb_wait_mode_pauses_in_command_menu() {
        let mut state = test_state();
        let _ = start_battle(
            &mut state,
            vec![("rattata".to_string(), 10)],
            BattleKind::Wild,
        );
        let _ = battle_confirm(&mut state);

        for _ in 0..120 {
            let _ = tick_battle_atb(&mut state);
            if state
                .battle
                .as_ref()
                .map(|battle| battle.stage == BattleStage::CommandMenu)
                .unwrap_or(false)
            {
                break;
            }
        }
        let before = state
            .battle
            .as_ref()
            .and_then(|battle| battle.enemies.first().map(|enemy| enemy.atb))
            .unwrap_or(0);
        let _ = tick_battle_atb(&mut state);
        let after = state
            .battle
            .as_ref()
            .and_then(|battle| battle.enemies.first().map(|enemy| enemy.atb))
            .unwrap_or(0);
        assert_eq!(before, after);
    }

    #[test]
    fn manual_enemy_targeting_hits_selected_enemy() {
        let mut state = test_state();
        let _ = start_battle(
            &mut state,
            vec![("oddish".to_string(), 10), ("zubat".to_string(), 10)],
            BattleKind::Wild,
        );
        if let Some(battle) = state.battle.as_mut() {
            battle.stage = BattleStage::CommandMenu;
            battle.current_actor = Some(BattleActor::Party(0));
            battle.menu_index = 0;
        }
        let _ = battle_confirm(&mut state);
        if let Some(battle) = state.battle.as_mut() {
            battle.enemy_target_index = 1;
        }
        let hp0_before = state
            .battle
            .as_ref()
            .and_then(|battle| battle.enemies.first().map(|enemy| enemy.hp))
            .unwrap_or(0);
        let hp1_before = state
            .battle
            .as_ref()
            .and_then(|battle| battle.enemies.get(1).map(|enemy| enemy.hp))
            .unwrap_or(0);
        let _ = battle_confirm(&mut state);
        let hp0_after = state
            .battle
            .as_ref()
            .and_then(|battle| battle.enemies.first().map(|enemy| enemy.hp))
            .unwrap_or(0);
        let hp1_after = state
            .battle
            .as_ref()
            .and_then(|battle| battle.enemies.get(1).map(|enemy| enemy.hp))
            .unwrap_or(0);
        assert_eq!(hp0_before, hp0_after);
        assert!(hp1_after < hp1_before);
    }

    #[test]
    fn manual_ally_targeting_item_heals_selected_member() {
        let mut state = test_state();
        state.party[1].hp = state.party[1].hp.saturating_sub(20);
        let _ = start_battle(
            &mut state,
            vec![("pidgey".to_string(), 10)],
            BattleKind::Wild,
        );
        if let Some(battle) = state.battle.as_mut() {
            if let Some(slot) = battle.party.iter_mut().find(|slot| slot.member_index == 1) {
                slot.hp = state.party[1].hp;
            }
            battle.stage = BattleStage::CommandMenu;
            battle.current_actor = Some(BattleActor::Party(0));
            battle.menu_index = 1; // ITEM
        }
        let potion_before = state
            .inventory
            .iter()
            .find(|stack| stack.kind == ItemKind::Potion)
            .map(|stack| stack.qty)
            .unwrap_or(0);
        let hp_before = state.party[1].hp;

        let _ = battle_confirm(&mut state); // Command -> Item menu
        let _ = battle_confirm(&mut state); // Item -> Ally target
        if let Some(battle) = state.battle.as_mut() {
            battle.ally_target_index = 1;
        }
        let _ = battle_confirm(&mut state); // Use on selected ally

        let potion_after = state
            .inventory
            .iter()
            .find(|stack| stack.kind == ItemKind::Potion)
            .map(|stack| stack.qty)
            .unwrap_or(0);
        assert!(state.party[1].hp > hp_before);
        assert_eq!(potion_after + 1, potion_before);
    }

    #[test]
    fn enemy_turn_targets_only_living_allies() {
        let mut state = test_state();
        let _ = start_battle(
            &mut state,
            vec![("zubat".to_string(), 10)],
            BattleKind::Wild,
        );
        state.party[0].hp = 0;
        if let Some(battle) = state.battle.as_mut() {
            if let Some(slot) = battle.party.iter_mut().find(|slot| slot.member_index == 0) {
                slot.hp = 0;
            }
            if let Some(slot) = battle.party.iter_mut().find(|slot| slot.member_index == 1) {
                slot.hp = state.party[1].hp;
            }
            battle.stage = BattleStage::Waiting;
            battle.current_actor = Some(BattleActor::Enemy(0));
        }
        let hp1_before = state.party[1].hp;
        resolve_enemy_turn(&mut state, 0);
        assert_eq!(state.party[0].hp, 0);
        assert!(state.party[1].hp < hp1_before);
    }

    #[test]
    fn defend_reduces_damage() {
        let mut base = test_state();
        base.party.truncate(1);
        base.active_party_index = 0;
        sync_legacy_from_active(&mut base);
        let _ = start_battle(&mut base, vec![("zubat".to_string(), 10)], BattleKind::Wild);

        let mut guarded = base.clone();
        let mut plain = base.clone();
        guarded.rng_seed = 42;
        plain.rng_seed = 42;
        if let Some(slot) = guarded
            .battle
            .as_mut()
            .and_then(|battle| battle.party.iter_mut().find(|slot| slot.member_index == 0))
        {
            slot.guard_active = true;
            slot.guard_pct = 50;
        }
        resolve_enemy_turn(&mut guarded, 0);
        resolve_enemy_turn(&mut plain, 0);

        let guarded_loss = base.party[0].hp.saturating_sub(guarded.party[0].hp);
        let plain_loss = base.party[0].hp.saturating_sub(plain.party[0].hp);
        assert!(plain_loss > guarded_loss);
    }

    #[test]
    fn failed_run_increments_attempts() {
        let mut state = test_state();
        let _ = start_battle(
            &mut state,
            vec![("rattata".to_string(), 10)],
            BattleKind::Wild,
        );
        if let Some(battle) = state.battle.as_mut() {
            battle.stage = BattleStage::CommandMenu;
            battle.current_actor = Some(BattleActor::Party(0));
            battle.menu_index = 4; // RUN
            battle.run_attempts = 0;
        }
        let chance = run_chance_pct(&state);
        let mut attempts = 0;
        loop {
            let mut probe = state.clone();
            let roll = (next_rand(&mut probe) % 100) as i32 + 1;
            if roll > chance || attempts > 200 {
                break;
            }
            state.rng_seed = state.rng_seed.wrapping_add(1);
            attempts += 1;
        }
        let _ = battle_confirm(&mut state);
        let run_attempts = state
            .battle
            .as_ref()
            .map(|battle| battle.run_attempts)
            .unwrap_or(0);
        let stage = state
            .battle
            .as_ref()
            .map(|battle| battle.stage)
            .unwrap_or(BattleStage::Defeat);
        assert_eq!(run_attempts, 1);
        assert_eq!(stage, BattleStage::Waiting);
    }

    #[test]
    fn victory_splits_xp_and_records_species_defeats() {
        let mut state = test_state();
        let exp_before_0 = state.party[0].exp;
        let exp_before_1 = state.party[1].exp;
        let _ = start_battle(
            &mut state,
            vec![("pidgey".to_string(), 7), ("zubat".to_string(), 7)],
            BattleKind::Wild,
        );
        if let Some(battle) = state.battle.as_mut() {
            for enemy in &mut battle.enemies {
                enemy.base_experience = 70;
                enemy.hp = 0;
                enemy.defeated = true;
            }
            battle.stage = BattleStage::Victory;
            battle.message = "Victory!".to_string();
        }
        let _ = end_battle(&mut state);

        assert_eq!(state.party[0].exp, exp_before_0 + 70);
        assert_eq!(state.party[1].exp, exp_before_1 + 70);
        assert_eq!(state.defeat_counts.get("pidgey").copied().unwrap_or(0), 1);
        assert_eq!(state.defeat_counts.get("zubat").copied().unwrap_or(0), 1);
    }

    #[test]
    fn wild_victory_can_recruit_new_party_member() {
        let mut state = test_state();
        state.party.truncate(1);
        state.active_party_index = 0;
        let _ = start_battle(&mut state, vec![("zubat".to_string(), 8)], BattleKind::Wild);
        state
            .enemy_info_cache
            .insert("zubat".to_string(), info("zubat", 40, 45, 35, 55, 60));
        if let Some(battle) = state.battle.as_mut() {
            if let Some(enemy) = battle.enemies.first_mut() {
                enemy.hp = 0;
                enemy.defeated = true;
            }
            battle.stage = BattleStage::Victory;
            battle.message = "Victory!".to_string();
        }
        let _ = end_battle(&mut state);
        assert_eq!(state.party.len(), 2);
        assert_eq!(normalize_species(&state.party[1].info.name), "zubat");
    }
}
