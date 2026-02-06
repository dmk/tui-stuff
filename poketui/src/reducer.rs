use tui_dispatch::DispatchResult;

use crate::action::Action;
use crate::effect::Effect;
use crate::state::{
    AppState, BattleStage, Direction, GameMode, SpriteTarget,
};

const PLAYER_NAME: &str = "pikachu";
const WILD_POOL: [&str; 6] = ["pidgey", "rattata", "caterpie", "weedle", "oddish", "zubat"];

pub fn reducer(state: &mut AppState, action: Action) -> DispatchResult<Effect> {
    match action {
        Action::Init => {
            state.message = Some("Heading into the grass...".to_string());
            state.player_sprite.reset();
            state.enemy_sprite.reset();
            state.enemy_info = None;
            state.battle = None;
            state.mode = GameMode::Overworld;
            state.steps_since_encounter = 0;
            state.player_sprite.loading = true;
            DispatchResult::changed_with(Effect::LoadPokemon {
                target: SpriteTarget::Player,
                name: PLAYER_NAME.to_string(),
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
        Action::PokemonDidLoad { target, info } => pokemon_loaded(state, target, info),
        Action::PokemonDidError {
            target,
            name,
            error,
        } => pokemon_error(state, target, &name, &error),
        Action::SpriteDidLoad { target, sprite } => sprite_loaded(state, target, sprite),
        Action::SpriteDidError { target, error } => sprite_error(state, target, &error),
        Action::Quit => DispatchResult::unchanged(),
    }
}

fn move_player(state: &mut AppState, direction: Direction) -> DispatchResult<Effect> {
    if state.mode != GameMode::Overworld {
        return DispatchResult::unchanged();
    }
    let (mut next_x, mut next_y) = (state.player.x, state.player.y);
    match direction {
        Direction::Up => next_y = next_y.saturating_sub(1),
        Direction::Down => next_y = next_y.saturating_add(1),
        Direction::Left => next_x = next_x.saturating_sub(1),
        Direction::Right => next_x = next_x.saturating_add(1),
    }
    if next_x == state.player.x && next_y == state.player.y {
        return DispatchResult::unchanged();
    }
    if next_x >= state.map.width || next_y >= state.map.height {
        return DispatchResult::unchanged();
    }
    if !state.map.is_walkable(next_x, next_y) {
        return DispatchResult::unchanged();
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
    state.mode = GameMode::Battle;
    state.enemy_info = None;
    state.enemy_sprite.reset();
    state.enemy_sprite.loading = true;
    state.battle = Some(crate::state::BattleState::new(
        enemy_name.clone(),
        state.player_max_hp(),
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
    let Some(battle) = state.battle.as_mut() else {
        return DispatchResult::unchanged();
    };
    if battle.stage != BattleStage::Menu {
        return DispatchResult::unchanged();
    }
    let menu_len = 2i16;
    let mut next = battle.menu_index as i16 + delta;
    if next < 0 {
        next = menu_len - 1;
    }
    if next >= menu_len {
        next = 0;
    }
    if next as usize == battle.menu_index {
        return DispatchResult::unchanged();
    }
    battle.menu_index = next as usize;
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
            if menu_index == 0 {
                let damage = roll_damage(state, 6, 12);
                let enemy_damage = roll_damage(state, 4, 9);
                play_sound = true;
                if let Some(battle) = state.battle.as_mut() {
                    battle.enemy_hp = battle.enemy_hp.saturating_sub(damage);
                    if battle.enemy_hp == 0 {
                        battle.stage = BattleStage::Victory;
                        battle.message = format!(
                            "{} fainted!",
                            format_name(&battle.enemy_name)
                        );
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
            } else if let Some(battle) = state.battle.as_mut() {
                battle.stage = BattleStage::Escape;
                battle.message = "Got away safely!".to_string();
            }
            if play_sound {
                DispatchResult::changed_with(Effect::PlayAttackSound)
            } else {
                DispatchResult::changed()
            }
        }
        BattleStage::EnemyTurn => {
            let pending_damage = state
                .battle
                .as_ref()
                .and_then(|battle| battle.pending_enemy_damage);
            let damage = pending_damage.unwrap_or_else(|| roll_damage(state, 4, 9));
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
            BattleStage::Victory => format!("{} won the battle!", format_name(&state.player_name())),
            BattleStage::Escape => "Back on the route.".to_string(),
            BattleStage::Defeat => "You recovered and returned to the path.".to_string(),
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
                battle.enemy_hp_max = info.hp.max(1);
                battle.enemy_hp = battle.enemy_hp_max;
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

fn pokemon_error(state: &mut AppState, target: SpriteTarget, name: &str, error: &str) -> DispatchResult<Effect> {
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
            state.player_sprite.sprite = Some(sprite);
            state.player_sprite.frame_index = 0;
            state.player_sprite.frame_tick = 0;
            state.player_sprite.loading = false;
        }
        SpriteTarget::Enemy => {
            state.enemy_sprite.sprite = Some(sprite);
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
    let changed = advance_sprite(&mut state.player_sprite)
        || advance_sprite(&mut state.enemy_sprite);
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

fn roll_damage(state: &mut AppState, min: u16, max: u16) -> u16 {
    let span = (max - min + 1) as u32;
    let roll = next_rand(state) % span;
    min + roll as u16
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
