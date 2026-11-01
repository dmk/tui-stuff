use tui_dispatch::ReducerResult;

use crate::action::{Action, Direction};
use crate::state::*;

pub fn reducer(state: &mut AppState, action: Action) -> ReducerResult {
    match action {
        Action::GameStart if state.phase == Phase::Title => state.phase = Phase::Playing,
        Action::GameRestart => {
            let (width, height) = (state.viewport_width, state.viewport_height);
            *state = AppState::new(state.seed);
            state.viewport_width = width;
            state.viewport_height = height;
            state.phase = Phase::Playing;
        }
        Action::PlayerMove(direction)
            if state.phase == Phase::Playing && state.viewport_ready() =>
        {
            let dx = match direction {
                Direction::Left => -PLAYER_MOVE_STEP,
                Direction::Right => PLAYER_MOVE_STEP,
            };
            let x = (state.player.x + dx).clamp(0, WORLD_WIDTH - 5);
            if state.player.x == x {
                return ReducerResult::unchanged();
            }
            state.player.x = x;
            state.player.motion_fraction = 0;
        }
        Action::PlayerKeyDown(key) if state.phase == Phase::Playing && state.viewport_ready() => {
            if state.player.held_keys & key.mask() != 0 {
                return ReducerResult::unchanged();
            }
            state.player.held_keys |= key.mask();
        }
        Action::PlayerKeyUp(key) => {
            if state.player.held_keys & key.mask() == 0 {
                return ReducerResult::unchanged();
            }
            state.player.held_keys &= !key.mask();
        }
        Action::PlayerControlsClear => {
            if state.player.held_keys == 0 {
                return ReducerResult::unchanged();
            }
            state.player.clear_controls();
        }
        Action::PlayerFire
            if state.phase == Phase::Playing
                && state.viewport_ready()
                && state.player.cooldown == 0 =>
        {
            state.bullets.push(Bullet {
                x: state.player.x + 2,
                y: PLAYER_Y - 1,
                dy: -2,
                owner: Owner::Player,
            });
            state.player.cooldown = FIRE_COOLDOWN;
        }
        Action::PauseToggle => match state.phase {
            Phase::Playing | Phase::Respawning | Phase::WaveClear => {
                state.player.clear_controls();
                state.resume_phase = Some(state.phase);
                state.phase = Phase::Paused;
            }
            Phase::Paused => state.phase = state.resume_phase.take().unwrap_or(Phase::Playing),
            _ => return ReducerResult::unchanged(),
        },
        Action::Tick => {
            if !step(state) {
                return ReducerResult::unchanged();
            }
        }
        Action::GameAdvanceTicks(count) => {
            let mut changed = false;
            for _ in 0..count {
                if !step(state) {
                    break;
                }
                changed = true;
            }
            if !changed {
                return ReducerResult::unchanged();
            }
        }
        Action::ViewportResize { width, height } => {
            let controls_changed = state.player.held_keys != 0;
            state.player.clear_controls();
            if !controls_changed && (width, height) == (state.viewport_width, state.viewport_height)
            {
                return ReducerResult::unchanged();
            }
            state.viewport_width = width;
            state.viewport_height = height;
        }
        _ => return ReducerResult::unchanged(),
    }
    ReducerResult::changed()
}

pub fn formation_interval(state: &AppState) -> u32 {
    // Original fleet cadence: one alien is processed per ~60 Hz refresh.
    // Quantize the whole-rack cycle to our deterministic 20 ms simulation clock.
    (state.aliens.len() as u32 * 1000)
        .div_ceil(60 * crate::TICK_MS as u32)
        .max(1)
}

fn step(state: &mut AppState) -> bool {
    if !state.viewport_ready()
        || !matches!(
            state.phase,
            Phase::Playing | Phase::Respawning | Phase::WaveClear | Phase::GameOver
        )
    {
        return false;
    }
    if state.phase == Phase::GameOver && state.explosions.is_empty() {
        return false;
    }
    state.tick += 1;
    for explosion in &mut state.explosions {
        explosion.age += 1;
    }
    state
        .explosions
        .retain(|explosion| explosion.age < EXPLOSION_TICKS);
    // The last ship's burst can finish under the modal without moving enemies
    // or advancing gameplay cooldowns after game over.
    if state.phase == Phase::GameOver {
        return true;
    }
    state.player.cooldown = state.player.cooldown.saturating_sub(1);
    state.player.invulnerable = state.player.invulnerable.saturating_sub(1);

    if state.phase == Phase::Respawning || state.phase == Phase::WaveClear {
        state.phase_timer = state.phase_timer.saturating_sub(1);
        if state.phase_timer == 0 {
            if state.phase == Phase::WaveClear {
                state.wave += 1;
                state.populate_wave();
            }
            state.player.x = PLAYER_START_X;
            state.player.motion_fraction = 0;
            state.player.clear_controls();
            state.player.invulnerable = INVULNERABLE_TICKS;
            state.phase = Phase::Playing;
        }
        return true;
    }

    state.player.advance_movement();

    state.formation_timer += 1;
    if state.formation_timer >= formation_interval(state) {
        state.formation_timer = 0;
        state.formation_frame ^= 1;
        let touches_edge = state.aliens.iter().any(|alien| {
            let next = alien.x + state.formation_direction;
            next < 0 || next + 5 > WORLD_WIDTH
        });
        if touches_edge {
            state.formation_direction *= -1;
            for alien in &mut state.aliens {
                alien.y += ALIEN_DROP;
            }
        } else {
            for alien in &mut state.aliens {
                alien.x += state.formation_direction;
            }
        }
        crush_shields(state);
    }

    if state.aliens.iter().any(|alien| alien.y + 4 >= PLAYER_Y) {
        state.player.clear_controls();
        state.phase = Phase::GameOver;
        state.bullets.clear();
        return true;
    }

    state.alien_fire_timer += 1;
    let fire_interval = (50_i64 - i64::from(state.wave.saturating_sub(1)) * 5).max(15) as u32;
    if state.alien_fire_timer >= fire_interval {
        state.alien_fire_timer = 0;
        fire_alien_bullet(state);
    }

    advance_saucer(state);
    advance_bullets_inner(state, state.tick.is_multiple_of(3));

    // A final kill takes precedence over the respawn delay, but never game over.
    if state.aliens.is_empty() && state.phase != Phase::GameOver {
        state.player.clear_controls();
        state.phase = Phase::WaveClear;
        state.phase_timer = WAVE_CLEAR_TICKS;
        state.bullets.clear();
        state.saucer = None;
    }
    true
}

fn crush_shields(state: &mut AppState) {
    for shield in &mut state.shields {
        for row in 0..6 {
            for col in 0..7 {
                if state
                    .aliens
                    .iter()
                    .any(|alien| alien.bounds().contains(shield.x + col, shield.y + row))
                {
                    shield.pixels[(row * 7 + col) as usize] = false;
                }
            }
        }
    }
}

pub fn fire_alien_bullet(state: &mut AppState) {
    if state
        .bullets
        .iter()
        .filter(|bullet| bullet.owner == Owner::Alien)
        .count()
        >= MAX_ALIEN_SHOTS
    {
        return;
    }
    let mut shooters = Vec::new();
    for column in 0..ALIEN_COLUMNS {
        if let Some(alien) = state
            .aliens
            .iter()
            .filter(|alien| alien.column == column)
            .max_by_key(|alien| alien.y)
        {
            shooters.push((alien.x + 2, alien.y + 4));
        }
    }
    if !shooters.is_empty() {
        let (x, y) = shooters[state.random() as usize % shooters.len()];
        state.bullets.push(Bullet {
            x,
            y,
            dy: 2,
            owner: Owner::Alien,
        });
    }
}

fn advance_saucer(state: &mut AppState) {
    if let Some(saucer) = &mut state.saucer {
        if state.tick.is_multiple_of(3) {
            saucer.x += saucer.direction;
        }
        if saucer.x >= WORLD_WIDTH || saucer.x + 7 <= 0 {
            state.saucer = None;
        }
    } else {
        state.saucer_timer = state.saucer_timer.saturating_sub(1);
        if state.saucer_timer == 0 {
            let direction = if state.random().is_multiple_of(2) {
                1
            } else {
                -1
            };
            let score = [100, 150, 200, 300][state.random() as usize % 4];
            state.saucer = Some(Saucer {
                x: if direction == 1 { -6 } else { WORLD_WIDTH - 1 },
                y: 1,
                direction,
                score,
            });
            state.saucer_timer = 1000 + (state.random() % 500) as u32;
        }
    }
}

#[derive(Clone, Copy)]
enum Target {
    Shield(usize, i32),
    Alien(usize),
    Saucer,
    Player,
}

// Walk the swept segment in travel order. This handles thin shields and ensures
// a shield or nearer alien intercepts a shot before a more distant target.
fn first_hit(state: &AppState, bullet: &Bullet) -> Option<Target> {
    for distance in 0..=bullet.dy.abs() {
        let y = bullet.y + distance * bullet.dy.signum();
        if let Some(index) = state
            .shields
            .iter()
            .position(|shield| shield.occupied(bullet.x, y))
        {
            return Some(Target::Shield(index, y));
        }
        match bullet.owner {
            Owner::Player => {
                if let Some(index) = state
                    .aliens
                    .iter()
                    .position(|alien| alien.bounds().contains(bullet.x, y))
                {
                    return Some(Target::Alien(index));
                }
                if state
                    .saucer
                    .as_ref()
                    .is_some_and(|saucer| saucer.bounds().contains(bullet.x, y))
                {
                    return Some(Target::Saucer);
                }
            }
            Owner::Alien if state.player.bounds().contains(bullet.x, y) => {
                return Some(Target::Player)
            }
            _ => {}
        }
    }
    None
}

pub fn advance_bullets(state: &mut AppState) {
    advance_bullets_inner(state, true);
}

fn advance_bullets_inner(state: &mut AppState, move_aliens: bool) {
    let mut survivors = Vec::new();
    for mut bullet in std::mem::take(&mut state.bullets) {
        // Alien shots advance every third simulation tick. At two world units
        // per step this approximates 4 arcade pixels every three video frames.
        if bullet.owner == Owner::Alien && !move_aliens {
            survivors.push(bullet);
            continue;
        }
        match first_hit(state, &bullet) {
            Some(Target::Shield(index, y)) => state.shields[index].chip(bullet.x, y),
            Some(Target::Alien(index)) => {
                let alien = state.aliens.remove(index);
                state.score += alien.kind.score();
                state.explosions.push(Explosion {
                    x: alien.x,
                    y: alien.y,
                    age: 0,
                });
            }
            Some(Target::Saucer) => {
                if let Some(saucer) = state.saucer.take() {
                    state.score += saucer.score;
                    state.explosions.push(Explosion {
                        x: saucer.x + 1,
                        y: saucer.y,
                        age: 0,
                    });
                }
            }
            Some(Target::Player) => {
                if state.player.invulnerable == 0 && state.phase == Phase::Playing {
                    state.player.clear_controls();
                    state.lives = state.lives.saturating_sub(1);
                    state.explosions.push(Explosion {
                        x: state.player.x,
                        y: PLAYER_Y,
                        age: 0,
                    });
                    state.phase = if state.lives == 0 {
                        Phase::GameOver
                    } else {
                        Phase::Respawning
                    };
                    state.phase_timer = RESPAWN_TICKS;
                }
            }
            None => {
                bullet.y += bullet.dy;
                if (0..WORLD_HEIGHT).contains(&bullet.y) {
                    survivors.push(bullet);
                }
            }
        }
    }
    state.bullets = if state.phase == Phase::Playing {
        survivors
    } else {
        Vec::new()
    };
}
