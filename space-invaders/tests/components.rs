use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use space_invaders::{
    components::ComponentRegistry,
    state::{AppState, Bullet, Explosion, Owner, Phase, Saucer},
    ui::GameUi,
};
use tui_dispatch::{testing::RenderHarness, EventKind};
use tui_dispatch_debug::debug::DebugOverlay;

#[test]
fn f12_components_view_lists_each_entity_with_its_state() {
    let mut state = AppState {
        phase: Phase::Playing,
        ..AppState::default()
    };
    state.bullets.push(Bullet {
        x: 30,
        y: 50,
        dy: -2,
        owner: Owner::Player,
    });
    state.explosions.push(Explosion {
        x: 20,
        y: 24,
        age: 8,
    });
    state.saucer = Some(Saucer {
        x: -3,
        y: 1,
        direction: 1,
        score: 100,
    });
    let mut ui = GameUi {
        components: ComponentRegistry::enabled(),
        ..GameUi::default()
    };
    // The debug layer is wired before the first render, just as in main.
    let mut debug = ui.components.debug_layer();
    let mut harness = RenderHarness::new(80, 40);
    harness.render(|frame| ui.render(frame, frame.area(), &state));
    for code in [KeyCode::F(12), KeyCode::Char('c')] {
        assert!(
            debug
                .handle_event_with_state(
                    &EventKind::Key(KeyEvent::new(code, KeyModifiers::NONE)),
                    &state,
                )
                .consumed
        );
    }
    let Some(DebugOverlay::Components(overlay)) = &debug.freeze().overlay else {
        panic!("expected the Components view");
    };
    let components = &overlay.components;
    for (name, expected) in [
        ("GameUi", 1),
        ("Hud", 1),
        ("Controls", 1),
        ("Playfield", 1),
        ("PhaseOverlay", 1),
        ("AlienComponent", 55),
        ("PlayerComponent", 1),
        ("ShieldComponent", 4),
        ("BulletComponent", 1),
        ("SaucerComponent", 1),
        ("ExplosionComponent", 1),
    ] {
        assert_eq!(
            components.iter().filter(|c| c.type_name == name).count(),
            expected,
            "{name}"
        );
    }
    let ids: std::collections::HashSet<_> = components
        .iter()
        .map(|c| c.bound_id.as_deref().unwrap())
        .collect();
    assert_eq!(ids.len(), components.len());
    let explosion = components
        .iter()
        .find(|c| c.bound_id.as_deref() == Some("explosion[0]"))
        .unwrap();
    assert!(explosion
        .debug_entries
        .contains(&("age".into(), "8".into())));
    let alien = components
        .iter()
        .find(|c| c.bound_id.as_deref() == Some("alien[0]"))
        .unwrap();
    assert!(alien
        .type_name_full
        .ends_with("components::alien::AlienComponent"));
    assert!(alien
        .debug_entries
        .contains(&("kind".into(), "\"Squid\"".into())));
    assert!(components.iter().all(|c| c.last_area.is_some()));

    let text = harness.render_to_string_plain(|frame| {
        debug.render_state(frame, &state, |frame, area| ui.render(frame, area, &state));
    });
    assert!(text.contains("Components"));
    assert!(text.contains("AlienComponent"));
}

#[test]
fn snapshots_follow_entity_lifetimes_damage_and_resize() {
    let mut state = AppState {
        phase: Phase::Playing,
        ..AppState::default()
    };
    state.bullets.push(Bullet {
        x: 30,
        y: 50,
        dy: -2,
        owner: Owner::Player,
    });
    let mut ui = GameUi {
        components: ComponentRegistry::enabled(),
        ..GameUi::default()
    };
    let mut harness = RenderHarness::new(80, 40);
    harness.render(|frame| ui.render(frame, frame.area(), &state));
    state.aliens.clear();
    state.bullets.clear();
    state.shields[0].pixels.fill(false);
    state.player.x = 10;
    harness.render(|frame| ui.render(frame, frame.area(), &state));
    let snapshots = ui.components.snapshots();
    assert!(!snapshots
        .iter()
        .any(|c| matches!(c.type_name.as_str(), "AlienComponent" | "BulletComponent")));
    let player = snapshots
        .iter()
        .find(|c| c.type_name == "PlayerComponent")
        .unwrap();
    assert!(player.debug_entries.contains(&("x".into(), "10".into())));
    let shield = snapshots
        .iter()
        .find(|c| c.bound_id.as_deref() == Some("shield[0]"))
        .unwrap();
    let pixels = shield
        .debug_entries
        .iter()
        .find(|(key, _)| key == "pixels")
        .unwrap();
    assert_eq!(
        serde_json::from_str::<Vec<bool>>(&pixels.1).unwrap(),
        vec![false; 42]
    );
    assert_eq!(
        snapshots
            .iter()
            .filter(|c| c.type_name == "PlayerComponent")
            .count(),
        1
    );

    let mut tiny = RenderHarness::new(20, 5);
    tiny.render(|frame| ui.render(frame, frame.area(), &state));
    let snapshots = ui.components.snapshots();
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].type_name, "GameUi");
    assert_eq!(snapshots[0].last_area.unwrap().width, 20);
}

#[test]
fn component_inspection_preserves_game_state_and_rendered_buffers() {
    let state = AppState {
        phase: Phase::Playing,
        ..AppState::default()
    };
    let before = state.clone();
    let mut ui = GameUi::default();
    for (w, h) in [(80, 40), (112, 59), (176, 78)] {
        let mut harness = RenderHarness::new(w, h);
        ui.components = ComponentRegistry::default();
        let plain = harness
            .render(|frame| ui.render(frame, frame.area(), &state))
            .clone();
        assert!(ui.components.snapshots().is_empty());
        ui.components = ComponentRegistry::enabled();
        let inspected = harness.render(|frame| ui.render(frame, frame.area(), &state));
        assert_eq!(&plain, inspected);
        assert!(!ui.components.snapshots().is_empty());
    }
    assert_eq!(state, before);
}

#[test]
fn entity_bounds_follow_the_sprite_instead_of_the_playfield() {
    use ratatui::layout::Rect;
    use space_invaders::{sprites::SpriteId, ui::BoardLayout};

    let mut state = AppState {
        phase: Phase::Playing,
        ..AppState::default()
    };
    state.saucer = Some(Saucer {
        x: -3,
        y: 1,
        direction: 1,
        score: 100,
    });
    state.bullets.push(Bullet {
        x: 30,
        y: 50,
        dy: -2,
        owner: Owner::Player,
    });
    state.explosions.push(Explosion {
        x: 20,
        y: 24,
        age: 8,
    });
    let mut ui = GameUi {
        components: ComponentRegistry::enabled(),
        ..GameUi::default()
    };
    for (width, height) in [(80, 40), (112, 59), (176, 78)] {
        let layout = BoardLayout::fit(Rect::new(0, 0, width, height)).unwrap();
        let mut harness = RenderHarness::new(width, height);
        harness.render(|frame| ui.render(frame, frame.area(), &state));
        let snapshots = ui.components.snapshots();
        let area = |id: &str| {
            snapshots
                .iter()
                .find(|c| c.bound_id.as_deref() == Some(id))
                .unwrap()
                .last_area
                .unwrap()
        };
        let (w, h) = layout.size.alien_bounds();
        for (index, alien) in state.aliens.iter().enumerate() {
            let (x, y) = layout.point(alien.x, alien.y);
            assert_eq!(
                area(&format!("alien[{index}]")),
                Rect::new(x as u16, y as u16, w, h)
            );
        }
        assert_ne!(area("alien[0]"), area("alien[1]"));
        for id in [
            "alien[0]",
            "player",
            "shield[0]",
            "bullet[0]",
            "explosion[0]",
            "saucer",
        ] {
            let bounds = area(id);
            assert!(layout.board.contains((bounds.x, bounds.y).into()), "{id}");
            assert_eq!(bounds.intersection(layout.board), bounds, "{id}");
            assert!(
                bounds.width < layout.board.width && bounds.height < layout.board.height,
                "{id}"
            );
        }
        // The saucer is entering from the left; only its visible part belongs
        // to the component. Negative coordinates must not wrap around.
        assert_eq!(area("saucer").x, layout.board.x);
        assert!(area("saucer").width < SpriteId::Saucer.bounds(layout.size).0);
        assert_eq!((area("bullet[0]").width, area("bullet[0]").height), (1, 1));
        let previous = area("alien[0]");
        state.aliens[0].x += 1;
        harness.render(|frame| ui.render(frame, frame.area(), &state));
        let moved = ui
            .components
            .snapshots()
            .into_iter()
            .find(|c| c.bound_id.as_deref() == Some("alien[0]"))
            .unwrap()
            .last_area
            .unwrap();
        assert!(moved.x > previous.x);
    }
}
