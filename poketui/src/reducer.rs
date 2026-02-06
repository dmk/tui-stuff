use tui_dispatch::DispatchResult;

use crate::action::Action;
use crate::effect::Effect;
use crate::state::{
    calc_hp, calc_stat, exp_for_level, AppState, BattleStage, Direction, GameMode, ItemKind,
    MenuState, PokemonSelectState, SpriteState, SpriteTarget, MAX_LEVEL,
};

const STARTERS: [&str; 4] = ["pikachu", "charmander", "bulbasaur", "squirtle"];
const WILD_POOL: [&str; 6] = ["pidgey", "rattata", "caterpie", "weedle", "oddish", "zubat"];
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
            DispatchResult::changed_with(Effect::CheckSaveExists)
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
        Action::BattleItemCancel => {
            let player_name = format_name(&state.player_name());
            if let Some(battle) = state.battle.as_mut() {
                if battle.stage == BattleStage::ItemMenu {
                    battle.stage = BattleStage::Menu;
                    battle.message = format!("What will {} do?", player_name);
                    return DispatchResult::changed();
                }
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
            state.message = Some(format!("Preview error: {}", error));
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
            state.message = Some("Game saved!".to_string());
            state.pause_menu.is_open = false;
            DispatchResult::changed()
        }
        Action::SaveError(error) => {
            state.message = Some(format!("Save failed: {}", error));
            DispatchResult::changed()
        }
        Action::LoadGame => DispatchResult::changed_with(Effect::LoadGame),
        Action::LoadComplete(loaded_state) => {
            // Replace entire state with loaded state
            *state = *loaded_state;
            normalize_loaded_state(state);
            state.message = Some("Game loaded!".to_string());
            DispatchResult::changed()
        }
        Action::LoadError(error) => {
            state.message = Some(format!("Load failed: {}", error));
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

    if state.map.is_grass(next_x, next_y) && state.steps_since_encounter >= 3 {
        let roll = next_rand(state) % 100;
        if roll < 18 {
            state.steps_since_encounter = 0;
            return start_battle(state);
        }
    }

    DispatchResult::changed()
}

fn start_battle(state: &mut AppState) -> DispatchResult<Effect> {
    let index = (next_rand(state) as usize) % WILD_POOL.len();
    let enemy_name = WILD_POOL[index].to_string();
    let enemy_level = roll_enemy_level(state);
    state.mode = GameMode::Battle;
    state.enemy_info = None;
    state.enemy_sprite.reset();
    state.enemy_sprite.loading = true;
    let player_hp_max = state.player_max_hp();
    if state.player_hp == 0 {
        state.player_hp = player_hp_max;
    }
    state.battle = Some(crate::state::BattleState::new(
        enemy_name.clone(),
        enemy_level,
        player_hp_max,
        state.player_hp,
    ));
    if let Some(battle) = state.battle.as_mut() {
        battle.message = format!("A wild {} appeared!", format_name(&enemy_name));
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
        BattleStage::Menu => 3i16,
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

fn battle_confirm(state: &mut AppState) -> DispatchResult<Effect> {
    let Some(stage) = state.battle.as_ref().map(|battle| battle.stage) else {
        return DispatchResult::unchanged();
    };

    match stage {
        BattleStage::Intro => {
            let player_name = state.player_name();
            if let Some(battle) = state.battle.as_mut() {
                battle.stage = BattleStage::Menu;
                battle.message = format!("What will {} do?", format_name(&player_name));
            }
            DispatchResult::changed()
        }
        BattleStage::Menu => {
            let menu_index = state
                .battle
                .as_ref()
                .map(|battle| battle.menu_index)
                .unwrap_or(0);
            let mut play_sound = false;
            match menu_index {
                0 => {
                    let enemy_level = state
                        .battle
                        .as_ref()
                        .map(|battle| battle.enemy_level)
                        .unwrap_or(5);
                    let damage = calc_damage(
                        state,
                        state.player_level,
                        player_attack(state),
                        enemy_defense(state, enemy_level),
                    );
                    let enemy_damage = calc_damage(
                        state,
                        enemy_level,
                        enemy_attack(state, enemy_level),
                        player_defense(state),
                    );
                    play_sound = true;
                    if let Some(battle) = state.battle.as_mut() {
                        battle.enemy_hp = battle.enemy_hp.saturating_sub(damage);
                        if battle.enemy_hp == 0 {
                            battle.stage = BattleStage::Victory;
                            battle.message =
                                format!("{} fainted!", format_name(&battle.enemy_name));
                        } else {
                            battle.stage = BattleStage::EnemyTurn;
                            battle.message = format!(
                                "You hit {} for {}!",
                                format_name(&battle.enemy_name),
                                damage
                            );
                            battle.pending_enemy_damage = Some(enemy_damage);
                        }
                    }
                }
                1 => {
                    let items = available_items(state);
                    if items.is_empty() {
                        if let Some(battle) = state.battle.as_mut() {
                            battle.message = "Your bag is empty.".to_string();
                        }
                    } else if let Some(battle) = state.battle.as_mut() {
                        battle.stage = BattleStage::ItemMenu;
                        battle.item_index = 0;
                        battle.message = "Choose an item.".to_string();
                    }
                }
                _ => {
                    if let Some(battle) = state.battle.as_mut() {
                        battle.stage = BattleStage::Escape;
                        battle.message = "Got away safely!".to_string();
                    }
                }
            }
            if play_sound {
                DispatchResult::changed_with(Effect::PlayAttackSound)
            } else {
                DispatchResult::changed()
            }
        }
        BattleStage::ItemMenu => {
            let items = available_items(state);
            if items.is_empty() {
                if let Some(battle) = state.battle.as_mut() {
                    battle.stage = BattleStage::Menu;
                    battle.message = "Your bag is empty.".to_string();
                }
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
                    battle.message = "HP is already full.".to_string();
                }
                return DispatchResult::changed();
            }

            if !take_item(state, kind) {
                if let Some(battle) = state.battle.as_mut() {
                    battle.stage = BattleStage::Menu;
                    battle.message = "No items left.".to_string();
                }
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
            let damage = pending_damage.unwrap_or_else(|| {
                calc_damage(
                    state,
                    enemy_level,
                    enemy_attack(state, enemy_level),
                    player_defense(state),
                )
            });
            if let Some(battle) = state.battle.as_mut() {
                battle.pending_enemy_damage = None;
                battle.player_hp = battle.player_hp.saturating_sub(damage);
                if battle.player_hp == 0 {
                    battle.stage = BattleStage::Defeat;
                    battle.message = "You fainted!".to_string();
                } else {
                    battle.stage = BattleStage::Menu;
                    battle.message = format!(
                        "Wild {} hit you for {}!",
                        format_name(&battle.enemy_name),
                        damage
                    );
                }
            }
            DispatchResult::changed()
        }
        BattleStage::Victory | BattleStage::Escape | BattleStage::Defeat => end_battle(state),
    }
}

fn end_battle(state: &mut AppState) -> DispatchResult<Effect> {
    if let Some(battle) = state.battle.take() {
        let message = match battle.stage {
            BattleStage::Victory => {
                let old_max = state.player_max_hp();
                let (gained, levels) = award_exp(state, battle.enemy_level);
                let new_max = state.player_max_hp();
                let hp_bonus = new_max.saturating_sub(old_max);
                state.player_hp = battle.player_hp.saturating_add(hp_bonus).min(new_max);
                if levels > 0 {
                    format!(
                        "{} won! Gained {} XP. Leveled up to {}!",
                        format_name(&state.player_name()),
                        gained,
                        state.player_level
                    )
                } else {
                    format!(
                        "{} won! Gained {} XP.",
                        format_name(&state.player_name()),
                        gained
                    )
                }
            }
            BattleStage::Escape => {
                state.player_hp = battle.player_hp.min(state.player_max_hp());
                "Back on the route.".to_string()
            }
            BattleStage::Defeat => {
                state.player_hp = state.player_max_hp();
                "You recovered and returned to the path.".to_string()
            }
            _ => "Battle ended.".to_string(),
        };
        state.message = Some(message);
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
            let max_hp = state.player_max_hp();
            if state.player_hp == 0 || state.player_hp > max_hp {
                state.player_hp = max_hp;
            }
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
            state.message = Some(format!("Player load error: {error}"));
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
            state.message = Some(format!("Player sprite error: {error}"));
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
    let mut changed = advance_sprite(&mut state.enemy_sprite);

    if state.mode != GameMode::Overworld {
        changed = advance_sprite(&mut state.player_sprite) || changed;
    } else if state.player_sprite.frame_index != 0 || state.player_sprite.frame_tick != 0 {
        state.player_sprite.frame_index = 0;
        state.player_sprite.frame_tick = 0;
        changed = true;
    }

    // Advance starter preview sprite if in pokemon select screen
    if let Some(select) = state.pokemon_select.as_mut() {
        changed = advance_sprite(&mut select.preview_sprite) || changed;
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

fn roll_enemy_level(state: &mut AppState) -> u8 {
    let base = state.player_level.max(2) as i16;
    let offset = (next_rand(state) % 5) as i16 - 2;
    let level = (base + offset).clamp(2, MAX_LEVEL as i16);
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

fn player_attack(state: &AppState) -> u16 {
    let base = state
        .player_info
        .as_ref()
        .map(|info| info.attack)
        .unwrap_or(10);
    calc_stat(base, state.player_level)
}

fn player_defense(state: &AppState) -> u16 {
    let base = state
        .player_info
        .as_ref()
        .map(|info| info.defense)
        .unwrap_or(10);
    calc_stat(base, state.player_level)
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

fn available_items(state: &AppState) -> Vec<(ItemKind, u16)> {
    state
        .inventory
        .iter()
        .filter(|stack| stack.qty > 0)
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

fn award_exp(state: &mut AppState, enemy_level: u8) -> (u32, u8) {
    let base_exp = state
        .enemy_info
        .as_ref()
        .map(|info| info.base_experience as u32)
        .unwrap_or(60);
    let gained = ((base_exp * enemy_level as u32) / 7).max(1);
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
    let max_hp = state.player_max_hp();
    if state.player_hp == 0 || state.player_hp > max_hp {
        state.player_hp = max_hp;
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
            state.pokemon_select = Some(PokemonSelectState {
                starters: STARTERS.iter().map(|s| s.to_string()).collect(),
                selected: 0,
                preview_info: None,
                preview_sprite: SpriteState::default(),
            });
            // Load preview for first starter
            DispatchResult::changed_with(Effect::LoadStarterPreview {
                name: STARTERS[0].to_string(),
            })
        }
        1 if menu.has_save => {
            // Continue -> Load Game
            state.menu = None;
            DispatchResult::changed_with(Effect::LoadGame)
        }
        1 if !menu.has_save => {
            // No save, this shouldn't be selectable but handle gracefully
            state.message = Some("No save file found.".to_string());
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
    state.player_level = 5;
    state.player_exp = exp_for_level(state.player_level);
    state.player_hp = state.player_max_hp();
    state.inventory = starting_inventory();
    state.message = Some(format!("You chose {}! Let's go!", format_name(&name)));

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
