use tui_dispatch::DispatchResult;

use crate::action::Action;
use crate::effect::Effect;
use crate::scenario::{AbilityEffect, AbilitySpec, ScenarioRuntime, ScenarioTrigger};
use crate::state::{
    calc_hp, calc_stat, exp_for_level, AppState, BattleKind, BattleStage, ComboHit, Direction,
    GameMode, ItemKind, MenuState, PartyMember, Pickup, PokemonSelectState, SpriteState,
    SpriteTarget, Tile, TurnActor, MAX_LEVEL,
};

const DEFAULT_STARTERS: [&str; 4] = ["pikachu", "charmander", "bulbasaur", "squirtle"];
const DEFAULT_WILD_POOL: [&str; 6] = ["pidgey", "rattata", "caterpie", "weedle", "oddish", "zubat"];
const BOSS_NAME: &str = "onix";
const RELIC_WINS: u16 = 3;
const BOSS_WINS: u16 = 5;
const MOVE_POWER: u32 = 40;

pub fn reducer(state: &mut AppState, action: Action) -> DispatchResult<Effect> {
    match action {
        Action::Init => {
            // Start at main menu and check if save exists
            state.mode = GameMode::MainMenu;
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
        Action::BattleMenuNext => battle_menu_change(state, 1),
        Action::BattleMenuPrev => battle_menu_change(state, -1),
        Action::BattleConfirm => battle_confirm(state),
        Action::MessageNext => message_next(state),
        Action::BattleItemCancel => {
            let is_item_menu = state
                .battle
                .as_ref()
                .map(|battle| battle.stage == BattleStage::ItemMenu)
                .unwrap_or(false);
            if is_item_menu {
                if let Some(battle) = state.battle.as_mut() {
                    battle.stage = BattleStage::Menu;
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
        .filter(|pool| !pool.is_empty());
    let enemy_name = if let Some(pool) = scenario_pool {
        let index = (next_rand(state) as usize) % pool.len();
        pool[index].clone()
    } else {
        let index = (next_rand(state) as usize) % DEFAULT_WILD_POOL.len();
        DEFAULT_WILD_POOL[index].to_string()
    };
    let enemy_level = roll_enemy_level(state);
    start_battle(state, enemy_name, enemy_level, BattleKind::Wild)
}

fn start_boss_battle(state: &mut AppState) -> DispatchResult<Effect> {
    let enemy_name = BOSS_NAME.to_string();
    let enemy_level = boss_level(state);
    start_battle(state, enemy_name, enemy_level, BattleKind::Boss)
}

fn start_battle(
    state: &mut AppState,
    enemy_name: String,
    enemy_level: u8,
    kind: BattleKind,
) -> DispatchResult<Effect> {
    state.mode = GameMode::Battle;
    state.enemy_info = None;
    state.enemy_sprite.reset();
    state.enemy_sprite.loading = true;
    let (player_hp_max, player_hp) = match state.active_member_mut() {
        Some(member) => {
            let max_hp = calc_hp(member.info.hp, member.level).max(1);
            if member.hp == 0 || member.hp > max_hp {
                member.hp = max_hp;
            }
            (max_hp, member.hp)
        }
        None => (state.player_max_hp(), state.player_max_hp()),
    };
    state.battle = Some(crate::state::BattleState::new(
        enemy_name.clone(),
        enemy_level,
        player_hp_max,
        player_hp,
        kind,
    ));
    if let Some(battle) = state.battle.as_mut() {
        battle.message = match battle.kind {
            BattleKind::Boss => format!("Boss {} appears!", format_name(&enemy_name)),
            BattleKind::Wild => format!("A wild {} appeared!", format_name(&enemy_name)),
        };
    }
    DispatchResult::changed_with(Effect::LoadPokemon {
        target: SpriteTarget::Enemy,
        name: enemy_name,
    })
}

fn battle_menu_change(state: &mut AppState, delta: i16) -> DispatchResult<Effect> {
    let Some(stage) = state.battle.as_ref().map(|battle| battle.stage) else {
        return DispatchResult::unchanged();
    };
    let menu_len = match stage {
        BattleStage::Menu => 5i16,
        BattleStage::ItemMenu => {
            let count = available_items(state).len() as i16;
            if count == 0 {
                return DispatchResult::unchanged();
            }
            count
        }
        _ => return DispatchResult::unchanged(),
    };
    let current_index = match stage {
        BattleStage::Menu => state
            .battle
            .as_ref()
            .map(|battle| battle.menu_index)
            .unwrap_or(0),
        BattleStage::ItemMenu => state
            .battle
            .as_ref()
            .map(|battle| battle.item_index)
            .unwrap_or(0),
        _ => 0,
    } as i16;
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
            BattleStage::Menu => battle.menu_index = next as usize,
            BattleStage::ItemMenu => battle.item_index = next as usize,
            _ => {}
        }
    }
    DispatchResult::changed()
}

fn set_battle_menu_prompt(state: &mut AppState) {
    let player_name = state.player_name();
    if let Some(battle) = state.battle.as_mut() {
        battle.message = format!("What will {} do?", format_name(&player_name));
    }
}

fn set_battle_item_prompt(state: &mut AppState) {
    if let Some(battle) = state.battle.as_mut() {
        battle.message = "Choose an item.".to_string();
    }
}

fn battle_confirm(state: &mut AppState) -> DispatchResult<Effect> {
    let Some(stage) = state.battle.as_ref().map(|battle| battle.stage) else {
        return DispatchResult::unchanged();
    };

    match stage {
        BattleStage::Intro => {
            if let Some(battle) = state.battle.as_mut() {
                battle.stage = BattleStage::Menu;
            }
            set_battle_menu_prompt(state);
            DispatchResult::changed()
        }
        BattleStage::Menu => {
            let menu_index = state
                .battle
                .as_ref()
                .map(|battle| battle.menu_index)
                .unwrap_or(0);
            let mut play_sound = false;
            let mut combo_effect: Option<Effect> = None;
            match menu_index {
                0 => {
                    play_sound = true;
                    combo_effect = start_combo_attack(state, None, None);
                }
                1 => {
                    let items = available_items(state);
                    if items.is_empty() {
                        push_message(state, "Your bag is empty.");
                        set_battle_menu_prompt(state);
                    } else {
                        if let Some(battle) = state.battle.as_mut() {
                            battle.stage = BattleStage::ItemMenu;
                            battle.item_index = 0;
                        }
                        set_battle_item_prompt(state);
                    }
                }
                2 => {
                    let kind = state
                        .battle
                        .as_ref()
                        .map(|battle| battle.kind)
                        .unwrap_or(BattleKind::Wild);
                    if kind == BattleKind::Boss {
                        push_message(state, "You can't catch this Pokemon!");
                        set_battle_menu_prompt(state);
                        return DispatchResult::changed();
                    }
                    if pokeball_count(state) == 0 {
                        push_message(state, "No Poke Balls left.");
                        set_battle_menu_prompt(state);
                        return DispatchResult::changed();
                    }
                    take_pokeball(state);
                    let (enemy_hp, enemy_hp_max, enemy_level, enemy_name) =
                        match state.battle.as_ref() {
                            Some(battle) => (
                                battle.enemy_hp,
                                battle.enemy_hp_max,
                                battle.enemy_level,
                                battle.enemy_name.clone(),
                            ),
                            None => return DispatchResult::unchanged(),
                        };
                    let hp_ratio = if enemy_hp_max == 0 {
                        1.0
                    } else {
                        enemy_hp as f32 / enemy_hp_max as f32
                    };
                    let mut chance = 0.2 + 0.6 * (1.0 - hp_ratio);
                    chance = chance.clamp(0.2, 0.8);
                    let roll = (next_rand(state) % 100) as f32 / 100.0;
                    if roll <= chance {
                        if state.party.len() >= 3 {
                            let pending_damage = calc_damage(
                                state,
                                enemy_level,
                                enemy_attack(state, enemy_level),
                                player_defense(state),
                            );
                            if let Some(battle) = state.battle.as_mut() {
                                battle.stage = BattleStage::EnemyTurn;
                                battle.message = "Party full!".to_string();
                                battle.pending_enemy_damage = Some(pending_damage);
                            }
                            return DispatchResult::changed();
                        }
                        if let Some(info) = state.enemy_info.clone() {
                            let level = enemy_level.max(1);
                            let max_hp = calc_hp(info.hp, level);
                            let ability_id = ability_id_for_species(state, &info.name);
                            state.party.push(PartyMember {
                                info,
                                level,
                                exp: exp_for_level(level),
                                hp: max_hp,
                                ability_id,
                                ability_cd: 0,
                            });
                            if let Some(battle) = state.battle.as_mut() {
                                battle.captured = true;
                                battle.stage = BattleStage::Victory;
                                battle.message = format!("Caught {}!", format_name(&enemy_name));
                            }
                        } else {
                            let pending_damage = calc_damage(
                                state,
                                enemy_level,
                                enemy_attack(state, enemy_level),
                                player_defense(state),
                            );
                            if let Some(battle) = state.battle.as_mut() {
                                battle.stage = BattleStage::EnemyTurn;
                                battle.message = "It slipped away!".to_string();
                                battle.pending_enemy_damage = Some(pending_damage);
                            }
                        }
                    } else {
                        let pending_damage = calc_damage(
                            state,
                            enemy_level,
                            enemy_attack(state, enemy_level),
                            player_defense(state),
                        );
                        if let Some(battle) = state.battle.as_mut() {
                            battle.stage = BattleStage::EnemyTurn;
                            battle.message = "It broke free!".to_string();
                            battle.pending_enemy_damage = Some(pending_damage);
                        }
                    }
                }
                3 => {
                    let ability = active_ability_spec(state);
                    let ability_name = ability.as_ref().map(|spec| spec.name.clone());
                    let ability_effect = ability.as_ref().map(|spec| spec.effect.clone());
                    let ability_cd = state
                        .active_member()
                        .map(|member| member.ability_cd)
                        .unwrap_or(0);
                    if ability.is_none() {
                        push_message(state, "No ability available.");
                        set_battle_menu_prompt(state);
                        return DispatchResult::changed();
                    }
                    if ability_cd > 0 {
                        push_message(
                            state,
                            format!("Ability recharging ({}).", ability_cd),
                        );
                        set_battle_menu_prompt(state);
                        return DispatchResult::changed();
                    }
                    if let Some(member) = state.active_member_mut() {
                        member.ability_cd = ability.as_ref().map(|spec| spec.cooldown).unwrap_or(0);
                    }
                    let mut ability_damage = None;
                    if let Some(effect) = ability_effect.clone() {
                        match effect {
                            AbilityEffect::Damage { power } => {
                                ability_damage = Some(power.max(1));
                                play_sound = true;
                            }
                            AbilityEffect::Heal { amount } => {
                                heal_active_member(state, amount);
                            }
                            AbilityEffect::Guard {
                                reduction_pct,
                                turns,
                            } => {
                                if let Some(battle) = state.battle.as_mut() {
                                    battle.guard_pct = reduction_pct.min(90);
                                    battle.guard_turns = turns.max(1);
                                }
                            }
                        }
                    }
                    combo_effect = start_combo_attack(state, ability_damage, ability_name);
                }
                _ => {
                    let kind = state
                        .battle
                        .as_ref()
                        .map(|battle| battle.kind)
                        .unwrap_or(BattleKind::Wild);
                    if kind == BattleKind::Boss {
                        push_message(state, "Can't run from a boss!");
                        set_battle_menu_prompt(state);
                        return DispatchResult::changed();
                    }
                    if let Some(battle) = state.battle.as_mut() {
                        battle.stage = BattleStage::Escape;
                        battle.message = "Got away safely!".to_string();
                    }
                }
            }
            let mut effects = Vec::new();
            if play_sound {
                effects.push(Effect::PlayAttackSound);
            }
            if let Some(effect) = combo_effect {
                effects.push(effect);
            }
            match effects.len() {
                0 => DispatchResult::changed(),
                1 => DispatchResult::changed_with(effects.remove(0)),
                _ => DispatchResult::changed_with_many(effects),
            }
        }
        BattleStage::ItemMenu => {
            let items = available_items(state);
            if items.is_empty() {
                if let Some(battle) = state.battle.as_mut() {
                    battle.stage = BattleStage::Menu;
                }
                push_message(state, "Your bag is empty.");
                set_battle_menu_prompt(state);
                return DispatchResult::changed();
            }

            let (item_index, player_hp, player_hp_max, enemy_level) = match state.battle.as_ref() {
                Some(battle) => (
                    battle.item_index,
                    battle.player_hp,
                    battle.player_hp_max,
                    battle.enemy_level,
                ),
                None => return DispatchResult::unchanged(),
            };

            let (kind, _) = items.get(item_index).copied().unwrap_or_else(|| items[0]);

            if player_hp >= player_hp_max {
                if let Some(battle) = state.battle.as_mut() {
                    battle.stage = BattleStage::Menu;
                }
                push_message(state, "HP is already full.");
                set_battle_menu_prompt(state);
                return DispatchResult::changed();
            }

            if !take_item(state, kind) {
                if let Some(battle) = state.battle.as_mut() {
                    battle.stage = BattleStage::Menu;
                }
                push_message(state, "No items left.");
                set_battle_menu_prompt(state);
                return DispatchResult::changed();
            }

            let heal = kind
                .heal_amount()
                .min(player_hp_max.saturating_sub(player_hp));
            let new_hp = player_hp.saturating_add(heal);
            let pending_damage = calc_damage(
                state,
                enemy_level,
                enemy_attack(state, enemy_level),
                player_defense(state),
            );

            if let Some(battle) = state.battle.as_mut() {
                battle.player_hp = new_hp;
                battle.stage = BattleStage::EnemyTurn;
                battle.message = format!("Used {}! Restored {} HP.", kind.label(), heal);
                battle.pending_enemy_damage = Some(pending_damage);
            }
            sync_active_hp_from_battle(state);

            DispatchResult::changed()
        }
        BattleStage::PlayerCombo => {
            let combo_empty = state
                .battle
                .as_ref()
                .map(|battle| battle.combo_hits.is_empty())
                .unwrap_or(true);
            if combo_empty {
                if let Some(battle) = state.battle.as_mut() {
                    battle.stage = BattleStage::Menu;
                }
                set_battle_menu_prompt(state);
                return DispatchResult::changed();
            }

            let hit = match state.battle.as_mut() {
                Some(battle) => battle.combo_hits.remove(0),
                None => return DispatchResult::unchanged(),
            };

            let (ended, effect) = apply_combo_hit(state, hit);
            if ended {
                if let Some(battle) = state.battle.as_mut() {
                    battle.combo_hits.clear();
                }
                if let Some(effect) = effect {
                    return DispatchResult::changed_with(effect);
                }
                return DispatchResult::changed();
            }
            if let Some(effect) = effect {
                return DispatchResult::changed_with(effect);
            }
            DispatchResult::changed()
        }
        BattleStage::EnemyTurn => {
            let pending_damage = state
                .battle
                .as_ref()
                .and_then(|battle| battle.pending_enemy_damage);
            let enemy_level = state
                .battle
                .as_ref()
                .map(|battle| battle.enemy_level)
                .unwrap_or(5);
            let mut damage = pending_damage.unwrap_or_else(|| {
                calc_damage(
                    state,
                    enemy_level,
                    enemy_attack(state, enemy_level),
                    player_defense(state),
                )
            });
            let mut fainted = false;
            if let Some(battle) = state.battle.as_mut() {
                if battle.guard_turns > 0 && battle.guard_pct > 0 {
                    let reduction = (damage as u32 * battle.guard_pct as u32) / 100;
                    damage = damage.saturating_sub(reduction as u16);
                    battle.guard_turns = battle.guard_turns.saturating_sub(1);
                    if battle.guard_turns == 0 {
                        battle.guard_pct = 0;
                    }
                }
                battle.pending_enemy_damage = None;
                battle.player_hp = battle.player_hp.saturating_sub(damage);
                fainted = battle.player_hp == 0;
            }
            sync_active_hp_from_battle(state);
            tick_ability_cooldowns(state);

            if fainted {
                if let Some(name) = switch_to_next_alive(state) {
                    if let Some(battle) = state.battle.as_mut() {
                        battle.stage = BattleStage::Menu;
                    }
                    push_message(state, format!("{} is sent out!", name));
                    set_battle_menu_prompt(state);
                    if let Some(effect) = load_player_sprite_for_active(state) {
                        return DispatchResult::changed_with(effect);
                    }
                } else if let Some(battle) = state.battle.as_mut() {
                    battle.stage = BattleStage::Defeat;
                    battle.message = "You fainted!".to_string();
                }
            } else if let Some(battle) = state.battle.as_mut() {
                battle.stage = BattleStage::Menu;
            }
            if !fainted {
                let enemy_name = state
                    .battle
                    .as_ref()
                    .map(|battle| battle.enemy_name.clone())
                    .unwrap_or_else(|| "Enemy".to_string());
                push_message(
                    state,
                    format!("Wild {} hit you for {}!", format_name(&enemy_name), damage),
                );
                set_battle_menu_prompt(state);
            }
            DispatchResult::changed()
        }
        BattleStage::Victory | BattleStage::Escape | BattleStage::Defeat => end_battle(state),
    }
}

fn end_battle(state: &mut AppState) -> DispatchResult<Effect> {
    if let Some(battle) = state.battle.take() {
        let mut message = match battle.stage {
            BattleStage::Victory => battle.message.clone(),
            BattleStage::Escape => "Back on the route.".to_string(),
            BattleStage::Defeat => "You recovered and returned to the path.".to_string(),
            _ => "Battle ended.".to_string(),
        };

        match battle.stage {
            BattleStage::Victory => {
                sync_active_hp_from_battle(state);
                let mut relic_triggered = false;
                if !battle.captured {
                    record_defeat(state, &battle.enemy_name);
                }
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
                } else if !battle.captured {
                    let old_max = state.player_max_hp();
                    let (gained, levels) = award_exp(state, battle.enemy_level);
                    let new_max = state.player_max_hp();
                    if let Some(member) = state.active_member_mut() {
                        let hp_bonus = new_max.saturating_sub(old_max);
                        member.hp = member.hp.saturating_add(hp_bonus).min(new_max);
                    }
                    sync_legacy_from_active(state);
                    if levels > 0 {
                        message = format!(
                            "{} won! Gained {} XP. Leveled up to {}!",
                            format_name(&state.player_name()),
                            gained,
                            state.active_level()
                        );
                    } else {
                        message = format!(
                            "{} won! Gained {} XP.",
                            format_name(&state.player_name()),
                            gained
                        );
                    }
                }

                if relic_triggered && battle.kind != BattleKind::Boss {
                    message = format!("{message} You found a relic!");
                }
            }
            BattleStage::Escape => {
                sync_active_hp_from_battle(state);
            }
            BattleStage::Defeat => {
                for member in &mut state.party {
                    let max_hp = calc_hp(member.info.hp, member.level).max(1);
                    member.hp = max_hp;
                }
                sync_legacy_from_active(state);
            }
            _ => {}
        }
        push_message(state, message);
    }
    state.mode = GameMode::Overworld;
    state.enemy_info = None;
    state.enemy_sprite.reset();
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
            state.enemy_info = Some(info.clone());
            if let Some(battle) = state.battle.as_mut() {
                let enemy_hp = calc_hp(info.hp, battle.enemy_level).max(1);
                battle.enemy_hp_max = enemy_hp;
                battle.enemy_hp = enemy_hp;
            }
            if let Some(sprite_url) = sprite_url_for(&info, target) {
                state.enemy_sprite.loading = true;
                return DispatchResult::changed_with(Effect::LoadSprite {
                    target,
                    url: sprite_url,
                });
            }
            state.enemy_sprite.loading = false;
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
                battle.stage = BattleStage::Escape;
                battle.message = format!("{name} fled.");
            }
            state.enemy_sprite.loading = false;
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
        }
        SpriteTarget::Enemy => {
            state.enemy_sprite.sprite = Some(sprite.clone());
            state.enemy_sprite.sprite_flipped = Some(sprite.flipped());
            state.enemy_sprite.frame_index = 0;
            state.enemy_sprite.frame_tick = 0;
            state.enemy_sprite.loading = false;
        }
    }
    DispatchResult::changed()
}

fn sprite_error(state: &mut AppState, target: SpriteTarget, error: &str) -> DispatchResult<Effect> {
    match target {
        SpriteTarget::Player => {
            push_message(state, format!("Player sprite error: {error}"));
            state.player_sprite.loading = false;
        }
        SpriteTarget::Enemy => {
            if let Some(battle) = state.battle.as_mut() {
                battle.message = format!("Sprite error: {error}");
            }
            state.enemy_sprite.loading = false;
        }
    }
    DispatchResult::changed()
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

    // Advance starter preview sprite if in pokemon select screen
    if let Some(select) = state.pokemon_select.as_mut() {
        sprite_changed = advance_sprite(&mut select.preview_sprite) || sprite_changed;
    }

    if sprite_changed {
        changed = true;
    }

    if let Some(effect) = maybe_request_party_sprite(state) {
        return DispatchResult::changed_with(effect);
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

fn active_ability_spec(state: &AppState) -> Option<AbilitySpec> {
    let member = state.active_member()?;
    let ability_id = member.ability_id.as_ref()?;
    let scenario = state.scenario.as_ref()?;
    scenario
        .manifest
        .abilities
        .iter()
        .find(|ability| ability.id == *ability_id)
        .cloned()
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

fn player_defense(state: &AppState) -> u16 {
    let (base, level) = state
        .active_member()
        .map(|member| (member.info.defense, member.level))
        .or_else(|| {
            state
                .player_info
                .as_ref()
                .map(|info| (info.defense, state.player_level))
        })
        .unwrap_or((10, state.player_level));
    calc_stat(base, level)
}

fn enemy_attack(state: &AppState, enemy_level: u8) -> u16 {
    let base = state
        .enemy_info
        .as_ref()
        .map(|info| info.attack)
        .unwrap_or(10);
    calc_stat(base, enemy_level)
}

fn enemy_defense(state: &AppState, enemy_level: u8) -> u16 {
    let base = state
        .enemy_info
        .as_ref()
        .map(|info| info.defense)
        .unwrap_or(10);
    calc_stat(base, enemy_level)
}

fn start_combo_attack(
    state: &mut AppState,
    ability_damage: Option<u16>,
    ability_name: Option<String>,
) -> Option<Effect> {
    let mut hits = build_combo_hits(state, ability_damage, ability_name.as_deref());
    if hits.is_empty() {
        if let Some(battle) = state.battle.as_mut() {
            battle.stage = BattleStage::Defeat;
            battle.message = "No one can fight!".to_string();
        }
        return None;
    }
    let first = hits.remove(0);
    let (ended, effect) = apply_combo_hit(state, first);
    if let Some(battle) = state.battle.as_mut() {
        battle.combo_hits.clear();
        if !ended {
            battle.stage = BattleStage::PlayerCombo;
            battle.combo_hits = hits;
        }
    }
    if let Some(battle) = state.battle.as_mut() {
        if matches!(battle.stage, BattleStage::Victory | BattleStage::Defeat) {
            battle.combo_hits.clear();
        }
    }
    effect
}

fn build_combo_hits(
    state: &mut AppState,
    ability_damage: Option<u16>,
    ability_name: Option<&str>,
) -> Vec<ComboHit> {
    let enemy_level = match state.battle.as_ref() {
        Some(battle) => battle.enemy_level,
        None => return Vec::new(),
    };
    let enemy_name = state
        .battle
        .as_ref()
        .map(|battle| battle.enemy_name.clone())
        .unwrap_or_else(|| "Enemy".to_string());
    let enemy_def = enemy_defense(state, enemy_level);

    let mut actors: Vec<(TurnActor, u16)> = Vec::new();
    if state.party.is_empty() {
        let base = state
            .player_info
            .as_ref()
            .map(|info| info.speed)
            .unwrap_or(10);
        let speed = calc_stat(base, state.player_level.max(1));
        actors.push((TurnActor::Player { member_index: 0 }, speed));
    } else {
        for (idx, member) in state.party.iter().enumerate() {
            if member.hp == 0 {
                continue;
            }
            let speed = calc_stat(member.info.speed, member.level.max(1));
            actors.push((TurnActor::Player { member_index: idx }, speed));
        }
    }

    if actors.is_empty() {
        return Vec::new();
    }

    let enemy_speed_base = state
        .enemy_info
        .as_ref()
        .map(|info| info.speed)
        .unwrap_or(10);
    let enemy_speed = calc_stat(enemy_speed_base, enemy_level.max(1));
    actors.push((TurnActor::Enemy, enemy_speed));

    actors.sort_by(|a, b| b.1.cmp(&a.1));
    let start = (next_rand(state) as usize) % actors.len();
    let first = actors.remove(start);
    let mut ordered = Vec::with_capacity(actors.len() + 1);
    ordered.push(first);
    ordered.extend(actors);

    let active_idx = state.active_party_index;
    let mut hits = Vec::new();
    for (actor, _) in ordered {
        match actor {
            TurnActor::Player { member_index } => {
                let (level, attack, name) = match state.party.get(member_index) {
                    Some(member) => (
                        member.level.max(1),
                        member.info.attack,
                        member.info.name.clone(),
                    ),
                    None => {
                        let info = state.player_info.as_ref();
                        let level = state.player_level.max(1);
                        let attack = info.map(|info| info.attack).unwrap_or(10);
                        let name = info
                            .map(|info| info.name.clone())
                            .unwrap_or_else(|| state.player_name());
                        (level, attack, name)
                    }
                };
                let is_active = member_index == active_idx;
                let ability_label = if is_active {
                    ability_name.map(|name| name.to_string())
                } else {
                    None
                };
                let ability_damage_used = is_active && ability_damage.is_some();
                let damage = if ability_damage_used {
                    ability_damage.unwrap_or(1).max(1)
                } else {
                    calc_damage(state, level, calc_stat(attack, level), enemy_def)
                };
                hits.push(ComboHit {
                    actor: TurnActor::Player { member_index },
                    name: format_name(&name),
                    damage,
                    ability_name: ability_label,
                    ability_damage: ability_damage_used,
                });
            }
            TurnActor::Enemy => {
                let damage = calc_damage(
                    state,
                    enemy_level,
                    enemy_attack(state, enemy_level),
                    player_defense(state),
                );
                hits.push(ComboHit {
                    actor: TurnActor::Enemy,
                    name: format_name(&enemy_name),
                    damage,
                    ability_name: None,
                    ability_damage: false,
                });
            }
        }
    }
    hits
}

fn apply_combo_hit(state: &mut AppState, hit: ComboHit) -> (bool, Option<Effect>) {
    let (battle_kind, enemy_name) = match state.battle.as_ref() {
        Some(battle) => (battle.kind, battle.enemy_name.clone()),
        None => return (false, None),
    };
    match hit.actor {
        TurnActor::Enemy => {
            let mut damage = hit.damage;
            let mut fainted = false;
            if let Some(battle) = state.battle.as_mut() {
                if battle.guard_turns > 0 && battle.guard_pct > 0 {
                    let reduction = (damage as u32 * battle.guard_pct as u32) / 100;
                    damage = damage.saturating_sub(reduction as u16);
                    battle.guard_turns = battle.guard_turns.saturating_sub(1);
                    if battle.guard_turns == 0 {
                        battle.guard_pct = 0;
                    }
                }
                battle.player_hp = battle.player_hp.saturating_sub(damage);
                fainted = battle.player_hp == 0;
            }
            sync_active_hp_from_battle(state);
            tick_ability_cooldowns(state);

            let prefix = match battle_kind {
                BattleKind::Boss => "Boss",
                BattleKind::Wild => "Wild",
            };
            let mut message = format!(
                "{} {} hit you for {}!",
                prefix,
                format_name(&enemy_name),
                damage
            );

            if fainted {
                let dead_index = state.active_party_index;
                if let Some(name) = switch_to_next_alive(state) {
                    if let Some(battle) = state.battle.as_mut() {
                        battle.combo_hits.retain(|combo| {
                            !matches!(combo.actor, TurnActor::Player { member_index } if member_index == dead_index)
                        });
                    }
                    message = format!("{} {} is sent out!", message, name);
                    if let Some(battle) = state.battle.as_mut() {
                        battle.message = message;
                    }
                    let effect = load_player_sprite_for_active(state);
                    return (false, effect);
                }
                if let Some(battle) = state.battle.as_mut() {
                    battle.stage = BattleStage::Defeat;
                    battle.message = format!("{} You fainted!", message);
                }
                return (true, None);
            }

            if let Some(battle) = state.battle.as_mut() {
                battle.message = message;
            }
            (false, None)
        }
        TurnActor::Player { member_index } => {
            let alive = if state.party.is_empty() {
                state
                    .battle
                    .as_ref()
                    .map(|battle| battle.player_hp > 0)
                    .unwrap_or(false)
            } else {
                state
                    .party
                    .get(member_index)
                    .map(|member| member.hp > 0)
                    .unwrap_or(false)
            };
            if !alive {
                return (false, None);
            }
            let mut enemy_hp = match state.battle.as_ref() {
                Some(battle) => battle.enemy_hp,
                None => return (false, None),
            };
            enemy_hp = enemy_hp.saturating_sub(hit.damage);
            let enemy_fainted = enemy_hp == 0;
            if let Some(battle) = state.battle.as_mut() {
                battle.enemy_hp = enemy_hp;
            }

            let mut message = if let Some(ability_name) = hit.ability_name.as_deref() {
                if hit.ability_damage {
                    format!("{} used {} for {}!", hit.name, ability_name, hit.damage)
                } else {
                    format!(
                        "{} used {}! {} hit for {}!",
                        hit.name, ability_name, hit.name, hit.damage
                    )
                }
            } else {
                format!("{} hit for {}!", hit.name, hit.damage)
            };

            if enemy_fainted {
                message = format!("{} {} fainted!", message, format_name(&enemy_name));
                if let Some(battle) = state.battle.as_mut() {
                    battle.stage = BattleStage::Victory;
                    battle.message = message;
                }
                return (true, None);
            }

            if let Some(battle) = state.battle.as_mut() {
                battle.message = message;
            }
            (false, None)
        }
    }
}

fn heal_active_member(state: &mut AppState, amount: u16) {
    if let Some(member) = state.active_member_mut() {
        let max_hp = calc_hp(member.info.hp, member.level).max(1);
        member.hp = member.hp.saturating_add(amount).min(max_hp);
    }
    sync_legacy_from_active(state);
    sync_battle_from_active(state);
}

fn tick_ability_cooldowns(state: &mut AppState) {
    for member in &mut state.party {
        if member.ability_cd > 0 {
            member.ability_cd = member.ability_cd.saturating_sub(1);
        }
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

fn pokeball_count(state: &AppState) -> u16 {
    state
        .inventory
        .iter()
        .find(|stack| stack.kind == ItemKind::PokeBall)
        .map(|stack| stack.qty)
        .unwrap_or(0)
}

fn take_pokeball(state: &mut AppState) -> bool {
    take_item(state, ItemKind::PokeBall)
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

fn sync_active_hp_from_battle(state: &mut AppState) {
    let (hp, level, base) = match (state.battle.as_ref(), state.active_member()) {
        (Some(battle), Some(member)) => (battle.player_hp, member.level, member.info.hp),
        _ => return,
    };
    if let Some(member) = state.active_member_mut() {
        let max_hp = calc_hp(base, level).max(1);
        member.hp = hp.min(max_hp);
    }
    sync_legacy_from_active(state);
}

fn sync_battle_from_active(state: &mut AppState) {
    let (level, base, hp) = match state.active_member() {
        Some(member) => (member.level, member.info.hp, member.hp),
        None => return,
    };
    if let Some(battle) = state.battle.as_mut() {
        let max_hp = calc_hp(base, level).max(1);
        battle.player_hp_max = max_hp;
        battle.player_hp = hp.min(max_hp);
    }
}

fn switch_to_next_alive(state: &mut AppState) -> Option<String> {
    let len = state.party.len();
    if len == 0 {
        return None;
    }
    let start = state.active_party_index;
    let mut next_index = None;
    let mut next_name = None;
    for offset in 1..=len {
        let idx = (start + offset) % len;
        if let Some(member) = state.party.get(idx) {
            if member.hp > 0 {
                next_index = Some(idx);
                next_name = Some(format_name(&member.info.name));
                break;
            }
        }
    }
    let (idx, name) = match (next_index, next_name) {
        (Some(idx), Some(name)) => (idx, name),
        _ => return None,
    };
    state.active_party_index = idx;
    sync_legacy_from_active(state);
    sync_battle_from_active(state);
    Some(name)
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

fn award_exp(state: &mut AppState, enemy_level: u8) -> (u32, u8) {
    let base_exp = state
        .enemy_info
        .as_ref()
        .map(|info| info.base_experience as u32)
        .unwrap_or(60);
    let gained = ((base_exp * enemy_level as u32) / 7).max(1);
    if let Some(member) = state.active_member_mut() {
        let starting_level = member.level;
        member.exp = member.exp.saturating_add(gained);
        while member.level < MAX_LEVEL
            && member.exp >= exp_for_level(member.level.saturating_add(1))
        {
            member.level = member.level.saturating_add(1);
        }
        let levels = member.level.saturating_sub(starting_level);
        sync_legacy_from_active(state);
        return (gained, levels);
    }
    let starting_level = state.player_level;
    state.player_exp = state.player_exp.saturating_add(gained);
    while state.player_level < MAX_LEVEL
        && state.player_exp >= exp_for_level(state.player_level.saturating_add(1))
    {
        state.player_level = state.player_level.saturating_add(1);
    }
    let levels = state.player_level.saturating_sub(starting_level);
    (gained, levels)
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
        crate::state::ItemStack {
            kind: ItemKind::PokeBall,
            qty: 5,
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
    } else if !state
        .inventory
        .iter()
        .any(|stack| stack.kind == ItemKind::PokeBall)
    {
        state.inventory.push(crate::state::ItemStack {
            kind: ItemKind::PokeBall,
            qty: 5,
        });
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
