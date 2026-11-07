use ratatui::{buffer::Buffer, style::Color};
use space_invaders::{
    action::{Action, Direction},
    reducer::reducer,
    sprites::{SpriteAtlas, SpriteId, SpriteSize, SpriteSizing},
    state::{AppState, Explosion, Phase},
    ui::{map_event, BoardLayout, GameUi},
};
use tui_dispatch::{testing::RenderHarness, EventKind};

fn render(state: &AppState, width: u16, height: u16) -> (String, Buffer) {
    let mut harness = RenderHarness::new(width, height);
    let mut ui = GameUi::default();
    let text = harness.render_to_string_plain(|frame| ui.render(frame, frame.area(), state));
    let buffer = harness
        .render(|frame| ui.render(frame, frame.area(), state))
        .clone();
    (text, buffer)
}

#[test]
fn all_frames_have_matching_bounds_and_artbox_colors() {
    let atlas = SpriteAtlas::new();
    for size in SpriteSize::ALL {
        for id in SpriteId::ALL {
            let bounds = |sprite: &artbox::sprites::SpriteRendered| {
                (sprite.chars[0].len() as u16, sprite.chars.len() as u16)
            };
            for index in 0..id.frame_count() {
                let frame = atlas.get(id, size, index);
                assert_eq!(bounds(frame), id.bounds(size), "{id:?} {size:?} {index}");
                let colors: std::collections::HashSet<_> = frame
                    .chars
                    .iter()
                    .flatten()
                    .filter(|cell| cell.ch != ' ')
                    .map(|cell| {
                        let color = cell.fg.expect("Artbox must color each visible cell");
                        (color.r, color.g, color.b)
                    })
                    .collect();
                assert!(!colors.is_empty());
                if id != SpriteId::Explosion {
                    assert!(colors.len() > 1, "{id:?} {size:?} needs a visible gradient");
                    let mut strongest_shade = 0;
                    for component in 0..3 {
                        let values: Vec<_> = colors
                            .iter()
                            .map(|&(r, g, b)| [r, g, b][component])
                            .collect();
                        let range = values.iter().max().unwrap() - values.iter().min().unwrap();
                        strongest_shade = strongest_shade.max(range);
                        assert!(
                            range <= 190,
                            "{id:?} {size:?} gradient became too contrasty"
                        );
                    }
                    assert!(
                        strongest_shade > 32,
                        "{id:?} {size:?} shading became too flat"
                    );
                }
            }
        }
    }
}

#[test]
fn approved_small_sprites_are_preserved() {
    let atlas = SpriteAtlas::new();
    let squid = atlas
        .get(SpriteId::Squid, SpriteSize::Small, 0)
        .to_plain_string();
    assert_eq!(squid.trim_end(), " ▄█▄ \n▟▀ ▀▙");
    assert_eq!(
        atlas
            .get(SpriteId::Player, SpriteSize::Small, 0)
            .to_plain_string()
            .trim_end(),
        "  ▄  \n▄███▄"
    );
}

#[test]
fn compact_override_keeps_small_sprites_in_large_terminals() {
    let state = AppState {
        phase: Phase::Playing,
        ..AppState::default()
    };
    let mut harness = RenderHarness::new(176, 78);
    let mut ui = GameUi {
        sizing: SpriteSizing::Compact,
        ..GameUi::default()
    };
    let frame = harness.render_to_string_plain(|frame| ui.render(frame, frame.area(), &state));
    assert_eq!(ui.sizing, SpriteSizing::Compact);
    assert!(frame.contains("▟▀ ▀▙"));
    assert!(!frame.contains("▟█▀█▀█▀█▙"));
    assert_eq!(
        BoardLayout::with_sizing(
            ratatui::layout::Rect::new(0, 0, 176, 78),
            SpriteSizing::Compact
        )
        .unwrap()
        .size,
        SpriteSize::Small
    );
}

#[test]
fn modal_dims_the_background_and_hud_but_keeps_its_content_bright() {
    let mut state = AppState {
        phase: Phase::Playing,
        ..AppState::default()
    };
    let (_, live) = render(&state, 80, 40);
    state.phase = Phase::Paused;
    let (text, modal) = render(&state, 80, 40);
    assert!(text.contains("PAUSED"));
    assert!(live.content.iter().all(|cell| cell.bg == Color::Reset));
    assert!(modal
        .content
        .iter()
        .any(|cell| cell.bg == Color::Rgb(35, 35, 45)));
    assert!(!text.contains('╭'));

    // Header score, top squid rank, and footer all sit outside the modal.
    for (x, y) in [(0, 0), (8, 4), (12, 39)] {
        assert_eq!(live[(x, y)].symbol(), modal[(x, y)].symbol());
        let Color::Rgb(r, g, b) = live[(x, y)].fg else {
            panic!("expected RGB foreground");
        };
        let Color::Rgb(dr, dg, db) = modal[(x, y)].fg else {
            panic!("expected dim RGB foreground");
        };
        assert!(dr < r && dg < g && db < b, "cell {x},{y} did not dim");
    }
    assert!(modal
        .content
        .iter()
        .any(|cell| cell.symbol() == "P" && cell.fg == Color::White));
}

#[test]
fn explosion_expands_then_fades_through_six_distinct_stages() {
    let atlas = SpriteAtlas::new();
    let mut previous = None;
    for age in [0, 2, 5, 8, 12, 17] {
        let explosion = Explosion { x: 20, y: 24, age };
        let frame = atlas.get(SpriteId::Explosion, SpriteSize::Small, explosion.frame());
        let text = frame.to_plain_string();
        assert_ne!(previous.as_ref(), Some(&text));
        previous = Some(text);
        let mut state = AppState {
            phase: Phase::Playing,
            ..AppState::default()
        };
        state.explosions.push(explosion);
        let (text, _) = render(&state, 80, 40);
        assert!(text.contains("SCORE"));
    }
    let brightness = |index| {
        atlas
            .get(SpriteId::Explosion, SpriteSize::Small, index)
            .chars
            .iter()
            .flatten()
            .filter(|cell| cell.ch != ' ')
            .map(|cell| {
                let c = cell.fg.unwrap();
                u32::from(c.r) + u32::from(c.g) + u32::from(c.b)
            })
            .max()
            .unwrap()
    };
    assert!(brightness(5) < brightness(2) / 2);
}

#[test]
fn layout_selects_largest_whole_board_that_fits() {
    assert_eq!(SpriteSizing::default(), SpriteSizing::Auto);
    assert_eq!(SpriteSize::fit(79, 40), None);
    assert_eq!(SpriteSize::fit(80, 39), None);
    assert_eq!(SpriteSize::fit(80, 40), Some(SpriteSize::Small));
    assert_eq!(SpriteSize::fit(112, 59), Some(SpriteSize::Medium));
    assert_eq!(SpriteSize::fit(176, 78), Some(SpriteSize::Large));
    assert_eq!(SpriteSize::fit(176, 77), Some(SpriteSize::Medium));
}

#[test]
fn all_sizes_show_the_same_game_and_colored_ranks() {
    let mut state = AppState::default();
    reducer(&mut state, Action::GameStart);
    for (width, height) in [(80, 40), (112, 59), (176, 78)] {
        let (text, buffer) = render(&state, width, height);
        assert!(text.contains("SCORE 000000"));
        assert!(text.contains("WAVE 01"));
        assert!(text.contains("SPACE fire"));
        let atlas = SpriteAtlas::new();
        let size = SpriteSize::fit(width, height).unwrap();
        for id in [
            SpriteId::Squid,
            SpriteId::Crab,
            SpriteId::Octopus,
            SpriteId::Player,
        ] {
            let sample = atlas
                .get(id, size, 0)
                .chars
                .iter()
                .flatten()
                .find(|cell| cell.ch != ' ')
                .unwrap()
                .fg
                .unwrap();
            assert!(buffer
                .content
                .iter()
                .any(|cell| cell.fg == Color::Rgb(sample.r, sample.g, sample.b)
                    && cell.symbol() != " "));
        }
        assert_eq!(text.lines().count(), usize::from(height));
    }
}

#[test]
fn phases_and_tiny_terminals_render_without_mutating_state() {
    let mut state = AppState::default();
    for phase in [
        Phase::Title,
        Phase::Paused,
        Phase::GameOver,
        Phase::WaveClear,
    ] {
        state.phase = phase;
        let before = state.clone();
        let (text, _) = render(&state, 80, 40);
        assert!(text.contains(match phase {
            Phase::Title => "SPACE INVADERS",
            Phase::Paused => "PAUSED",
            Phase::GameOver => "GAME OVER",
            _ => "SECTOR CLEAR",
        }));
        for (w, h) in [(1, 1), (20, 5), (63, 24), (80, 23)] {
            let (small, _) = render(&state, w, h);
            assert!(!small.is_empty());
            if w >= 30 {
                assert!(small.contains("Resize to at least"));
            }
        }
        assert_eq!(state, before);
    }
}

#[test]
fn shield_damage_is_visible_in_each_size() {
    let state = AppState {
        phase: Phase::Playing,
        ..AppState::default()
    };
    for (w, h) in [(80, 40), (112, 59), (176, 78)] {
        let (_, intact) = render(&state, w, h);
        let mut damaged = state.clone();
        damaged.shields[0].pixels.fill(false);
        let (_, damaged_buffer) = render(&damaged, w, h);
        assert_ne!(intact, damaged_buffer);
    }
}

#[test]
fn saucers_clip_at_edges_and_resize_keeps_the_simulation() {
    let mut state = AppState::default();
    state.phase = Phase::Playing;
    state.saucer = Some(space_invaders::state::Saucer {
        x: -6,
        y: 1,
        direction: 1,
        score: 100,
    });
    let before = state.clone();
    for (w, h) in [(80, 40), (112, 59), (176, 78)] {
        render(&state, w, h);
    }
    assert_eq!(state, before);
    let layout = BoardLayout::fit(ratatui::layout::Rect::new(0, 0, 80, 40)).unwrap();
    assert_eq!(layout.board.x, 0);
}

#[test]
fn controls_accept_presses_repeats_and_ignore_release() {
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
    let state = AppState::default();
    for kind in [KeyEventKind::Press, KeyEventKind::Repeat] {
        let event = EventKind::Key(KeyEvent::new_with_kind(
            KeyCode::Left,
            KeyModifiers::NONE,
            kind,
        ));
        assert_eq!(
            map_event(&event, &state).actions,
            vec![Action::PlayerMove(Direction::Left)]
        );
    }
    let event = EventKind::Key(KeyEvent::new_with_kind(
        KeyCode::Left,
        KeyModifiers::NONE,
        KeyEventKind::Release,
    ));
    assert!(map_event(&event, &state).actions.is_empty());
    assert_eq!(
        map_event(&EventKind::Resize(50, 20), &state).actions,
        vec![Action::ViewportResize {
            width: 50,
            height: 20
        }]
    );
}

#[test]
fn held_controls_map_down_and_up_but_ignore_repeat() {
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
    use space_invaders::{action::MovementKey, input::KeyboardMode, ui::map_event_with_mode};
    let state = AppState::default();
    for (code, movement) in [
        (KeyCode::Left, MovementKey::Left),
        (KeyCode::Char('a'), MovementKey::A),
        (KeyCode::Right, MovementKey::Right),
        (KeyCode::Char('d'), MovementKey::D),
    ] {
        for (kind, expected) in [
            (KeyEventKind::Press, vec![Action::PlayerKeyDown(movement)]),
            (KeyEventKind::Release, vec![Action::PlayerKeyUp(movement)]),
            (KeyEventKind::Repeat, vec![]),
        ] {
            let event = EventKind::Key(KeyEvent::new_with_kind(code, KeyModifiers::NONE, kind));
            assert_eq!(
                map_event_with_mode(&event, &state, KeyboardMode::Held).actions,
                expected
            );
        }
    }
}
