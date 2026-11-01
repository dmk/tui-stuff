use space_invaders::{
    action::{Action, Direction, MovementKey},
    reducer::{advance_bullets, fire_alien_bullet, formation_interval, reducer},
    state::*,
};
use tui_dispatch::testing::StoreTestHarness;

fn playing() -> AppState {
    let mut state = AppState::default();
    reducer(&mut state, Action::GameStart);
    state
}

#[test]
fn classic_formation_has_five_rows_eleven_columns_and_four_bunkers() {
    let mut state = playing();
    assert_eq!(state.aliens.len(), 55);
    for (kind, expected) in [
        (AlienKind::Squid, 11),
        (AlienKind::Crab, 22),
        (AlienKind::Octopus, 22),
    ] {
        assert_eq!(
            state
                .aliens
                .iter()
                .filter(|alien| alien.kind == kind)
                .count(),
            expected
        );
    }
    for row in 0..5 {
        assert_eq!(
            state
                .aliens
                .iter()
                .filter(|alien| alien.y == ALIEN_START_Y + row * ALIEN_ROW_SPACING)
                .count(),
            11
        );
    }
    for alien in &state.aliens {
        assert!(alien.x >= 0 && alien.x + 5 <= WORLD_WIDTH);
        assert!(alien.y + 4 < state.shields[0].y);
    }
    assert_eq!(state.shields.len(), 4);
    // The additional rightmost columns must participate in enemy shooting too.
    state.aliens.retain(|alien| alien.column == 10);
    fire_alien_bullet(&mut state);
    assert_eq!((state.bullets[0].x, state.bullets[0].y), (69, 35));
}

fn shot(x: i32, y: i32, dy: i32, owner: Owner) -> Bullet {
    Bullet { x, y, dy, owner }
}

#[test]
fn replay_and_snapshot_round_trip_are_deterministic() {
    let actions = [
        Action::GameStart,
        Action::PlayerMove(Direction::Right),
        Action::PlayerFire,
        Action::GameAdvanceTicks(140),
        Action::PauseToggle,
        Action::GameAdvanceTicks(50),
        Action::PauseToggle,
        Action::GameAdvanceTicks(120),
    ];
    let mut harness = StoreTestHarness::new(AppState::new(0), reducer);
    harness.dispatch_all(actions.clone());
    let mut second = AppState::new(0);
    for action in actions {
        let action = serde_json::from_str(&serde_json::to_string(&action).unwrap()).unwrap();
        reducer(&mut second, action);
    }
    assert_eq!(harness.state(), &second);
    let mut restored: AppState =
        serde_json::from_str(&serde_json::to_string(&second).unwrap()).unwrap();
    reducer(&mut restored, Action::GameAdvanceTicks(90));
    reducer(&mut second, Action::GameAdvanceTicks(90));
    assert_eq!(restored, second);
}

#[test]
fn advance_ticks_matches_individual_ticks_including_transitions() {
    let mut batched = playing();
    let mut individual = batched.clone();
    reducer(&mut batched, Action::GameAdvanceTicks(2000));
    for _ in 0..2000 {
        reducer(&mut individual, Action::Tick);
    }
    assert_eq!(batched, individual);
}

#[test]
fn opening_fleet_step_takes_about_a_second_and_accelerates_with_survivors() {
    let mut state = playing();
    assert_eq!(formation_interval(&state), 46);
    let initial = state.aliens.clone();
    reducer(&mut state, Action::GameAdvanceTicks(45));
    assert_eq!(state.aliens, initial);
    reducer(&mut state, Action::Tick);
    assert_eq!(state.aliens[0].x, initial[0].x + 1);
    assert_eq!(state.aliens[0].y, initial[0].y);
    state.aliens.truncate(11);
    assert_eq!(formation_interval(&state), 10);
    state.aliens.truncate(1);
    assert_eq!(formation_interval(&state), 1);
}

#[test]
fn untouched_first_fleet_allows_minutes_before_invasion() {
    let mut state = playing();
    state.player.invulnerable = 100_000;
    let lowest = state.aliens.iter().map(|alien| alien.y + 4).max().unwrap();
    assert!(
        PLAYER_Y - lowest >= 36,
        "the opening fleet needs vertical headroom"
    );
    reducer(&mut state, Action::GameAdvanceTicks(6000));
    assert_eq!(
        state.phase,
        Phase::Playing,
        "an untouched fleet must not invade in under two minutes"
    );
    assert_eq!(state.lives, 3);
    reducer(&mut state, Action::GameAdvanceTicks(3000));
    assert_eq!(
        state.phase,
        Phase::GameOver,
        "the fleet must still eventually invade"
    );
    assert!((120_000..=180_000).contains(&(state.tick * space_invaders::TICK_MS)));
}

#[test]
fn opening_enemy_shot_gives_over_a_second_to_react() {
    let mut state = playing();
    let lowest = state.aliens.iter().map(|alien| alien.y + 4).max().unwrap();
    state
        .bullets
        .push(shot(PLAYER_START_X + 2, lowest, 2, Owner::Alien));
    reducer(&mut state, Action::GameAdvanceTicks(50));
    assert_eq!(state.lives, 3);
    assert_eq!(state.phase, Phase::Playing);
    reducer(&mut state, Action::GameAdvanceTicks(10));
    assert_eq!(state.lives, 2);
    assert_eq!(state.phase, Phase::Respawning);
}

#[test]
fn later_racks_start_lower_using_the_arcade_sequence() {
    let mut state = playing();
    for (wave, first_y) in [7, 19, 27, 31, 31, 31, 35, 35, 35, 19]
        .into_iter()
        .enumerate()
    {
        state.wave = wave as u32 + 1;
        state.populate_wave();
        assert_eq!(state.aliens[0].y, first_y);
        assert!(state.aliens.iter().all(|alien| alien.y + 4 < SHIELD_Y));
        assert_eq!(
            formation_interval(&state),
            46,
            "wave difficulty comes from height, not arbitrary clock changes"
        );
    }
}

#[test]
fn enemy_fire_has_three_projectile_slots() {
    let mut state = playing();
    for _ in 0..10 {
        fire_alien_bullet(&mut state);
    }
    assert_eq!(state.bullets.len(), 3);
    state.bullets.remove(0);
    fire_alien_bullet(&mut state);
    assert_eq!(state.bullets.len(), 3);
}

#[test]
fn movement_clamps_and_does_not_work_on_title_or_pause() {
    let mut state = AppState::default();
    reducer(&mut state, Action::PlayerMove(Direction::Left));
    assert_eq!(state.player.x, PLAYER_START_X);
    reducer(&mut state, Action::GameStart);
    for _ in 0..100 {
        reducer(&mut state, Action::PlayerMove(Direction::Left));
    }
    assert_eq!(state.player.x, 0);
    for _ in 0..100 {
        reducer(&mut state, Action::PlayerMove(Direction::Right));
    }
    assert_eq!(state.player.x, WORLD_WIDTH - 5);
    reducer(&mut state, Action::PauseToggle);
    reducer(&mut state, Action::PlayerMove(Direction::Left));
    assert_eq!(state.player.x, WORLD_WIDTH - 5);
}

#[test]
fn held_movement_advances_every_tick_without_repeat_and_stops_on_release() {
    let mut state = playing();
    reducer(&mut state, Action::PlayerKeyDown(MovementKey::Right));
    assert_eq!(
        state.player.x, PLAYER_START_X,
        "input must not jump the ship"
    );
    reducer(&mut state, Action::Tick);
    assert_eq!(state.player.x, PLAYER_START_X + 1);
    reducer(&mut state, Action::GameAdvanceTicks(9));
    assert_eq!(state.player.x, PLAYER_START_X + 10);
    assert!(!reducer(&mut state, Action::PlayerKeyDown(MovementKey::Right)).changed);
    reducer(&mut state, Action::Tick);
    assert_eq!(state.player.x, PLAYER_START_X + 11);
    reducer(&mut state, Action::PlayerKeyUp(MovementKey::Right));
    reducer(&mut state, Action::GameAdvanceTicks(10));
    assert_eq!(state.player.x, PLAYER_START_X + 11);
}

#[test]
fn held_keys_handle_aliases_and_opposite_directions_independently() {
    let mut state = playing();
    reducer(&mut state, Action::PlayerKeyDown(MovementKey::Left));
    reducer(&mut state, Action::PlayerKeyDown(MovementKey::A));
    reducer(&mut state, Action::PlayerKeyUp(MovementKey::A));
    reducer(&mut state, Action::Tick);
    assert_eq!(state.player.x, PLAYER_START_X - 1);
    reducer(&mut state, Action::PlayerKeyDown(MovementKey::D));
    reducer(&mut state, Action::GameAdvanceTicks(5));
    assert_eq!(
        state.player.x,
        PLAYER_START_X - 1,
        "opposite directions cancel"
    );
    reducer(&mut state, Action::PlayerKeyUp(MovementKey::Left));
    reducer(&mut state, Action::Tick);
    assert_eq!(state.player.x, PLAYER_START_X);
}

#[test]
fn held_ship_crosses_the_field_in_one_and_a_half_seconds_and_clamps() {
    let mut state = playing();
    state.player.x = 0;
    state.player.invulnerable = 1000;
    reducer(&mut state, Action::PlayerKeyDown(MovementKey::D));
    reducer(&mut state, Action::GameAdvanceTicks(75));
    assert_eq!(state.player.x, WORLD_WIDTH - 5);
    reducer(&mut state, Action::GameAdvanceTicks(20));
    assert_eq!(state.player.x, WORLD_WIDTH - 5);
    reducer(&mut state, Action::PlayerKeyUp(MovementKey::D));
    reducer(&mut state, Action::PlayerKeyDown(MovementKey::A));
    reducer(&mut state, Action::GameAdvanceTicks(80));
    assert_eq!(state.player.x, 0);
    assert_eq!(state.player.motion_fraction, 0);
}

#[test]
fn pause_resize_and_debug_clear_do_not_resume_stale_held_input() {
    for cancellation in [
        Action::PauseToggle,
        Action::ViewportResize {
            width: 80,
            height: 40,
        },
        Action::PlayerControlsClear,
    ] {
        let mut state = playing();
        reducer(&mut state, Action::PlayerKeyDown(MovementKey::Right));
        assert!(reducer(&mut state, cancellation.clone()).changed);
        assert_eq!(state.player.held_keys, 0);
        if matches!(cancellation, Action::PauseToggle) {
            reducer(&mut state, Action::PauseToggle);
        }
        let x = state.player.x;
        reducer(&mut state, Action::GameAdvanceTicks(5));
        assert_eq!(state.player.x, x);
    }
}

#[test]
fn held_input_and_fractional_positions_round_trip_with_snapshots() {
    let mut state = playing();
    state.player.motion_fraction = 500;
    reducer(&mut state, Action::PlayerKeyDown(MovementKey::Right));
    let mut loaded: AppState =
        serde_json::from_str(&serde_json::to_string(&state).unwrap()).unwrap();
    reducer(&mut state, Action::GameAdvanceTicks(5));
    for _ in 0..5 {
        reducer(&mut loaded, Action::Tick);
    }
    assert_eq!(loaded, state);
    assert_eq!(state.player.motion_fraction, 500);
    let mut old_player = serde_json::to_value(&state.player).unwrap();
    old_player.as_object_mut().unwrap().remove("held_keys");
    old_player
        .as_object_mut()
        .unwrap()
        .remove("motion_fraction");
    let restored: Player = serde_json::from_value(old_player).unwrap();
    assert_eq!((restored.held_keys, restored.motion_fraction), (0, 0));
}

#[test]
fn fire_cooldown_uses_simulation_time() {
    let mut state = playing();
    reducer(&mut state, Action::PlayerFire);
    reducer(&mut state, Action::PlayerFire);
    assert_eq!(state.bullets.len(), 1);
    assert_eq!(state.player.cooldown, FIRE_COOLDOWN);
    reducer(&mut state, Action::GameAdvanceTicks(FIRE_COOLDOWN - 1));
    let before = state.bullets.len();
    reducer(&mut state, Action::PlayerFire);
    assert_eq!(state.bullets.len(), before);
    reducer(&mut state, Action::Tick);
    reducer(&mut state, Action::PlayerFire);
    assert_eq!(state.bullets.len(), before + 1);
}

#[test]
fn shield_intercepts_the_nearest_half_pixel_in_both_directions() {
    let mut state = playing();
    state.shields = vec![Shield::new(8)];
    let before = state.shields[0]
        .pixels
        .iter()
        .filter(|pixel| **pixel)
        .count();
    state.bullets = vec![shot(8, SHIELD_Y + 7, -12, Owner::Player)];
    advance_bullets(&mut state);
    assert!(!state.shields[0].occupied(8, SHIELD_Y + 5));
    assert!(state.shields[0].occupied(8, SHIELD_Y + 4));
    assert_eq!(
        before
            - state.shields[0]
                .pixels
                .iter()
                .filter(|pixel| **pixel)
                .count(),
        2
    );
    assert!(state.bullets.is_empty());
    state.bullets = vec![shot(11, SHIELD_Y - 2, 12, Owner::Alien)];
    advance_bullets(&mut state);
    assert!(!state.shields[0].occupied(11, SHIELD_Y));
    assert!(state.shields[0].occupied(11, SHIELD_Y + 1));
}

#[test]
fn shields_protect_targets_behind_them() {
    let mut state = playing();
    state.player.x = 7;
    state.shields = vec![Shield::new(8)];
    state.bullets = vec![shot(9, SHIELD_Y - 3, 20, Owner::Alien)];
    advance_bullets(&mut state);
    assert_eq!(state.lives, 3);
    assert!(state.bullets.is_empty());
}

#[test]
fn swept_shot_hits_nearest_alien_not_vector_order() {
    let mut state = playing();
    state.shields.clear();
    state.aliens = vec![
        Alien {
            kind: AlienKind::Squid,
            column: 0,
            x: 5,
            y: 5,
        },
        Alien {
            kind: AlienKind::Octopus,
            column: 0,
            x: 5,
            y: 15,
        },
    ];
    state.bullets = vec![shot(7, 25, -30, Owner::Player)];
    advance_bullets(&mut state);
    assert_eq!(state.aliens.len(), 1);
    assert_eq!(state.aliens[0].kind, AlienKind::Squid);
    assert_eq!(state.score, 10);
    assert_eq!(state.explosions.len(), 1);
}

#[test]
fn one_projectile_scores_once_and_saucer_uses_seeded_reward() {
    let mut state = playing();
    state.shields.clear();
    state.aliens.clear();
    state.saucer = Some(Saucer {
        x: 20,
        y: 1,
        direction: 1,
        score: 200,
    });
    state.bullets = vec![shot(22, 10, -20, Owner::Player)];
    advance_bullets(&mut state);
    assert_eq!(state.score, 200);
    assert!(state.saucer.is_none());
    advance_bullets(&mut state);
    assert_eq!(state.score, 200);
}

#[test]
fn saucer_spawns_moves_and_leaves_on_simulation_ticks() {
    let mut state = playing();
    state.saucer_timer = 1;
    reducer(&mut state, Action::Tick);
    let saucer = state.saucer.clone().unwrap();
    assert!([100, 150, 200, 300].contains(&saucer.score));
    reducer(&mut state, Action::GameAdvanceTicks(3));
    assert_ne!(state.saucer.as_ref().unwrap().x, saucer.x);
    state.saucer = Some(Saucer {
        x: WORLD_WIDTH - 1,
        y: 1,
        direction: 1,
        score: 100,
    });
    reducer(&mut state, Action::GameAdvanceTicks(3));
    assert!(state.saucer.is_none());
}

#[test]
fn only_the_lowest_survivor_in_a_column_fires() {
    let mut state = playing();
    state.aliens.retain(|alien| alien.column == 2);
    fire_alien_bullet(&mut state);
    assert_eq!(state.bullets[0].y, 35);
    state
        .aliens
        .retain(|alien| alien.kind != AlienKind::Octopus);
    state.bullets.clear();
    fire_alien_bullet(&mut state);
    assert_eq!(state.bullets[0].y, 23);
}

#[test]
fn death_respawn_invulnerability_and_game_over() {
    let mut state = playing();
    state.shields.clear();
    let x = PLAYER_START_X + 2;
    state.bullets = vec![
        shot(x, PLAYER_Y - 2, 8, Owner::Alien),
        shot(x, PLAYER_Y - 2, 8, Owner::Alien),
    ];
    advance_bullets(&mut state);
    assert_eq!(state.lives, 2, "multiple hits in one tick cost one life");
    assert_eq!(state.phase, Phase::Respawning);
    reducer(&mut state, Action::GameAdvanceTicks(RESPAWN_TICKS));
    assert_eq!(state.phase, Phase::Playing);
    assert_eq!(state.player.invulnerable, INVULNERABLE_TICKS);
    state.bullets = vec![shot(x, PLAYER_Y - 2, 8, Owner::Alien)];
    advance_bullets(&mut state);
    assert_eq!(state.lives, 2);
    state.player.invulnerable = 0;
    state.lives = 1;
    state.bullets = vec![shot(x, PLAYER_Y - 2, 8, Owner::Alien)];
    advance_bullets(&mut state);
    assert_eq!(state.phase, Phase::GameOver);
    assert_eq!(state.lives, 0);
    let aliens = state.aliens.clone();
    reducer(&mut state, Action::GameAdvanceTicks(200));
    assert!(state.explosions.is_empty());
    assert_eq!(state.aliens, aliens);
    assert_eq!(state.lives, 0);
    assert_eq!(state.phase, Phase::GameOver);
    let final_state = state.clone();
    reducer(&mut state, Action::GameAdvanceTicks(200));
    assert_eq!(state, final_state);
}

#[test]
fn final_kill_advances_wave_preserving_score_and_lives() {
    let mut state = playing();
    state.lives = 2;
    state.shields.clear();
    state.aliens = vec![Alien {
        kind: AlienKind::Squid,
        column: 0,
        x: 5,
        y: 5,
    }];
    state.bullets = vec![shot(7, 10, -6, Owner::Player)];
    reducer(&mut state, Action::Tick);
    assert_eq!(state.phase, Phase::WaveClear);
    assert_eq!(state.score, 30);
    reducer(&mut state, Action::GameAdvanceTicks(WAVE_CLEAR_TICKS));
    assert_eq!((state.wave, state.score, state.lives), (2, 30, 2));
    assert_eq!(state.aliens.len(), INITIAL_ALIEN_COUNT);
    assert_eq!(state.shields.len(), 4);
    assert_eq!(state.phase, Phase::Playing);
    assert!(state.bullets.is_empty());
}

#[test]
fn explosion_lifetime_uses_game_time_and_freezes_when_paused() {
    let mut state = playing();
    state.explosions.push(Explosion {
        x: 20,
        y: 24,
        age: 0,
    });
    reducer(&mut state, Action::GameAdvanceTicks(EXPLOSION_TICKS - 1));
    assert_eq!(state.explosions.len(), 1);
    assert_eq!(state.explosions[0].age, EXPLOSION_TICKS - 1);
    reducer(&mut state, Action::PauseToggle);
    reducer(&mut state, Action::GameAdvanceTicks(100));
    assert_eq!(state.explosions[0].age, EXPLOSION_TICKS - 1);
    reducer(&mut state, Action::PauseToggle);
    reducer(&mut state, Action::Tick);
    assert!(state.explosions.is_empty());
}

#[test]
fn formation_reverses_descends_and_accelerates() {
    let mut state = playing();
    state.aliens = vec![Alien {
        kind: AlienKind::Squid,
        column: 0,
        x: WORLD_WIDTH - 5,
        y: 7,
    }];
    let interval = formation_interval(&state);
    assert!(interval < formation_interval(&playing()));
    state.formation_timer = interval - 1;
    reducer(&mut state, Action::Tick);
    assert_eq!(state.formation_direction, -1);
    assert_eq!(
        (state.aliens[0].x, state.aliens[0].y),
        (WORLD_WIDTH - 5, 7 + ALIEN_DROP)
    );
    state.wave = 100;
    assert_eq!(formation_interval(&state), 1);
    state.aliens[0].y = PLAYER_Y - 4;
    reducer(&mut state, Action::Tick);
    assert_eq!(state.phase, Phase::GameOver);
}

#[test]
fn pause_and_small_viewports_freeze_all_gameplay_timers() {
    for phase in [Phase::Playing, Phase::Respawning, Phase::WaveClear] {
        let mut state = playing();
        state.phase = phase;
        state.phase_timer = 20;
        reducer(&mut state, Action::PauseToggle);
        let paused = state.clone();
        reducer(&mut state, Action::GameAdvanceTicks(100));
        assert_eq!(state, paused);
        reducer(&mut state, Action::PauseToggle);
        assert_eq!(state.phase, phase);
        reducer(
            &mut state,
            Action::ViewportResize {
                width: 63,
                height: 24,
            },
        );
        let suspended = state.clone();
        reducer(&mut state, Action::GameAdvanceTicks(100));
        reducer(&mut state, Action::PlayerMove(Direction::Right));
        reducer(&mut state, Action::PlayerFire);
        assert_eq!(state, suspended);
        reducer(
            &mut state,
            Action::ViewportResize {
                width: 120,
                height: 50,
            },
        );
        reducer(&mut state, Action::Tick);
        assert_eq!(state.tick, suspended.tick + 1);
    }
}

#[test]
fn restart_restores_seed_and_keeps_current_viewport() {
    let mut state = AppState::new(123);
    reducer(&mut state, Action::GameStart);
    reducer(&mut state, Action::GameAdvanceTicks(150));
    reducer(
        &mut state,
        Action::ViewportResize {
            width: 160,
            height: 50,
        },
    );
    reducer(&mut state, Action::GameRestart);
    let mut expected = AppState::new(123);
    expected.phase = Phase::Playing;
    expected.viewport_width = 160;
    expected.viewport_height = 50;
    assert_eq!(state, expected);
}

#[tokio::test(start_paused = true)]
async fn subscription_has_no_immediate_tick_and_stops_when_cancelled() {
    use std::time::Duration;
    use tui_dispatch::subscriptions::Subscriptions;
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let mut subscriptions = Subscriptions::new(tx);
    subscriptions.interval("game-clock", Duration::from_millis(20), || Action::Tick);
    tokio::task::yield_now().await;
    assert!(rx.try_recv().is_err());
    tokio::time::advance(Duration::from_millis(19)).await;
    tokio::task::yield_now().await;
    assert!(rx.try_recv().is_err());
    tokio::time::advance(Duration::from_millis(1)).await;
    tokio::task::yield_now().await;
    assert_eq!(rx.try_recv().unwrap(), Action::Tick);
    subscriptions.cancel_all();
    tokio::time::advance(Duration::from_secs(1)).await;
    tokio::task::yield_now().await;
    assert!(rx.try_recv().is_err());
}
