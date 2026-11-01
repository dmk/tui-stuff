use game_of_life::{action::Action, patterns::Pattern, reducer::reducer, state::*};

fn blank() -> AppState {
    let mut state = AppState::default();
    reducer(&mut state, Action::Clear);
    state
}

fn living(state: &AppState) -> Vec<(i32, i32)> {
    state
        .cells
        .iter()
        .enumerate()
        .filter_map(|(i, &age)| {
            (age > 0).then_some((i as i32 % WORLD_WIDTH, i as i32 / WORLD_WIDTH))
        })
        .collect()
}

fn load(state: &mut AppState, pattern: Pattern) {
    reducer(state, Action::PatternSelect(pattern));
    reducer(state, Action::PatternLoad);
}

#[test]
fn block_is_stable_blinker_alternates_and_lonely_cell_dies() {
    let mut state = blank();
    for (x, y) in [(10, 10), (11, 10), (10, 11), (11, 11)] {
        reducer(&mut state, Action::CellSet { x, y, alive: true });
    }
    let before = living(&state);
    reducer(&mut state, Action::Advance(20));
    assert_eq!(living(&state), before);
    assert_eq!(state.population, 4);
    assert_eq!((state.births, state.deaths), (0, 0));
    assert_eq!(state.age(10, 10), 21);

    reducer(&mut state, Action::Clear);
    for x in 9..=11 {
        reducer(
            &mut state,
            Action::CellSet {
                x,
                y: 10,
                alive: true,
            },
        );
    }
    reducer(&mut state, Action::Step);
    assert_eq!(living(&state), [(10, 9), (10, 10), (10, 11)]);
    assert_eq!((state.births, state.deaths), (2, 2));
    reducer(&mut state, Action::Step);
    assert_eq!(living(&state), [(9, 10), (10, 10), (11, 10)]);

    reducer(&mut state, Action::Clear);
    reducer(
        &mut state,
        Action::CellSet {
            x: 0,
            y: 0,
            alive: true,
        },
    );
    reducer(&mut state, Action::Step);
    assert_eq!(state.population, 0);
    assert_eq!(state.trails[0], 12);
    reducer(&mut state, Action::Advance(12));
    assert_eq!(state.trails[0], 0);
}

#[test]
fn glider_translates_and_pulsar_returns_after_three_generations() {
    let mut state = blank();
    load(&mut state, Pattern::Glider);
    let shifted: Vec<_> = living(&state)
        .into_iter()
        .map(|(x, y)| (x + 1, y + 1))
        .collect();
    reducer(&mut state, Action::Advance(4));
    assert_eq!(living(&state), shifted);
    load(&mut state, Pattern::Pulsar);
    let initial = living(&state);
    assert_eq!(initial.len(), 48);
    reducer(&mut state, Action::Advance(3));
    assert_eq!(living(&state), initial);
}

#[test]
fn gun_emits_gliders_while_retaining_its_core() {
    let mut state = blank();
    load(&mut state, Pattern::GosperGun);
    let initial = living(&state);
    assert_eq!(initial.len(), 36);
    reducer(&mut state, Action::Advance(30));
    assert!(initial.iter().all(|&(x, y)| state.age(x, y) > 0));
    assert_eq!(state.population, 41);
}

#[test]
fn wrapping_changes_neighbors_at_both_world_edges() {
    let mut wrapped = blank();
    for x in [WORLD_WIDTH - 1, 0, 1] {
        reducer(
            &mut wrapped,
            Action::CellSet {
                x,
                y: 0,
                alive: true,
            },
        );
    }
    let mut fixed = wrapped.clone();
    reducer(&mut fixed, Action::BoundaryToggle);
    reducer(&mut wrapped, Action::Step);
    reducer(&mut fixed, Action::Step);
    assert_eq!(living(&wrapped), [(0, 0), (0, 1), (0, WORLD_HEIGHT - 1)]);
    assert_eq!(fixed.population, 0);
}

#[test]
fn clock_obeys_speed_pause_help_and_resize_suspension() {
    let mut state = AppState::default();
    for _ in 0..50 {
        reducer(&mut state, Action::Tick);
    }
    assert_eq!(state.generation, 8);
    reducer(&mut state, Action::PauseToggle);
    let before = state.clone();
    for _ in 0..100 {
        reducer(&mut state, Action::Tick);
    }
    assert_eq!(state, before);
    reducer(&mut state, Action::PauseToggle);
    reducer(&mut state, Action::HelpToggle);
    let before = state.clone();
    reducer(&mut state, Action::Tick);
    assert_eq!(state, before);
    reducer(&mut state, Action::HelpToggle);
    reducer(
        &mut state,
        Action::ViewportResize {
            width: 40,
            height: 10,
        },
    );
    reducer(&mut state, Action::Tick);
    assert_eq!(state.generation, 8);
    reducer(
        &mut state,
        Action::ViewportResize {
            width: 120,
            height: 42,
        },
    );
    reducer(&mut state, Action::SpeedChange(100));
    for _ in 0..50 {
        reducer(&mut state, Action::Tick);
    }
    assert_eq!(state.generation, 68);
}

#[test]
fn seeded_soup_snapshots_and_action_batches_are_deterministic() {
    let mut first = AppState::new(0);
    reducer(&mut first, Action::Randomize);
    let mut second = AppState::new(0);
    reducer(&mut second, Action::Randomize);
    assert_eq!(first, second);
    reducer(&mut first, Action::Advance(90));
    for _ in 0..90 {
        reducer(&mut second, Action::Step);
    }
    assert_eq!(first, second);
    assert_eq!(first.history.len(), 80);
    let json = serde_json::to_string(&first).unwrap();
    let mut restored: AppState = serde_json::from_str(&json).unwrap();
    restored.validate().unwrap();
    reducer(&mut restored, Action::Advance(2));
    reducer(&mut first, Action::Advance(2));
    assert_eq!(restored, first);
}

#[test]
fn editing_clips_patterns_and_rejects_outside_cells() {
    let mut state = blank();
    let before = state.clone();
    reducer(
        &mut state,
        Action::CellSet {
            x: -1,
            y: 0,
            alive: true,
        },
    );
    assert_eq!(state, before);
    reducer(
        &mut state,
        Action::CursorMove {
            dx: i32::MIN,
            dy: i32::MAX,
        },
    );
    assert_eq!((state.cursor_x, state.cursor_y), (0, WORLD_HEIGHT - 1));
    reducer(&mut state, Action::PatternSelect(Pattern::Pulsar));
    reducer(&mut state, Action::PatternStamp);
    assert!(state.population > 0 && state.population < 48);
    assert!(!state.running);
    state.validate().unwrap();
}
