use std::{cell::RefCell, io, path::PathBuf, time::Duration};

use clap::Parser;
use ratatui::{
    backend::{Backend, TestBackend},
    Terminal,
};
use tui_dispatch::{NoEffect, StoreWithMiddleware};
#[cfg(unix)]
use tui_dispatch_debug::{observe::DebugCollector, transport::DebugServer};
use tui_dispatch_debug::{DebugCliArgs, DebugSession};

use space_invaders::{
    action::Action,
    components::ComponentRegistry,
    input::{KeyboardMode, KeyboardSession},
    reducer::reducer,
    sprites::SpriteSizing,
    state::{AppState, MIN_HEIGHT},
    ui::{map_event_with_mode, GameUi},
    DEFAULT_SEED, TICK_MS,
};

#[derive(Parser, Debug)]
#[command(about = "A tiny, colorful Space Invaders arcade game")]
struct Args {
    #[arg(long, default_value_t = DEFAULT_SEED)]
    seed: u64,
    /// Automatically fit sprites to the terminal; compact forces 5×2 sprites.
    #[arg(long, value_enum, default_value_t = SpriteSizing::Auto)]
    sprite_size: SpriteSizing,
    /// Auto negotiates smooth held movement; legacy keeps stepped input.
    #[arg(long, value_enum, default_value_t = KeyboardMode::Auto)]
    keyboard_mode: KeyboardMode,
    /// Headless frame width (interactive play uses the terminal size).
    #[arg(long, default_value_t = 80, value_parser = clap::value_parser!(u16).range(1..=512))]
    width: u16,
    /// Headless frame height.
    #[arg(long, default_value_t = MIN_HEIGHT, value_parser = clap::value_parser!(u16).range(1..=256))]
    height: u16,
    /// Save the final gameplay state as a JSON snapshot.
    #[arg(long)]
    debug_state_out: Option<PathBuf>,
    /// Allow a standalone read-only debugger to attach through this Unix socket.
    #[arg(long, value_name = "PATH")]
    debug_socket: Option<PathBuf>,
    #[command(flatten)]
    debug: DebugCliArgs,
}

struct RestoreTerminal;

impl Drop for RestoreTerminal {
    fn drop(&mut self) {
        ratatui::restore();
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = Args::parse();
    #[cfg(not(unix))]
    if args.debug_socket.is_some() {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "debugger attachment requires Unix",
        )
        .into());
    }
    // The framework normally filters Tick out. Record it for deterministic games,
    // while preserving any explicit user filters.
    if args.debug.actions_out.is_some()
        && args.debug.actions_include.is_none()
        && args.debug.actions_exclude.is_none()
        && args.debug.actions_include_categories.is_none()
        && args.debug.actions_exclude_categories.is_none()
        && args.debug.actions_include_names.is_none()
        && args.debug.actions_exclude_names.is_none()
    {
        args.debug.actions_include = Some("*".into());
    }
    let headless = args.debug.render_once;
    let scripted = args.debug.actions_in.is_some();
    let session = DebugSession::new(args.debug.clone());
    session.save_state_schema::<AppState>()?;
    session.save_actions_schema::<Action>()?;

    if headless {
        let mut terminal = Terminal::new(TestBackend::new(args.width, args.height))?;
        run(
            &mut terminal,
            &session,
            &args,
            scripted,
            KeyboardMode::Legacy,
        )
        .await?;
    } else {
        let mut terminal = ratatui::try_init()?;
        let _restore = RestoreTerminal;
        let keyboard = KeyboardSession::start(args.keyboard_mode)?;
        run(&mut terminal, &session, &args, scripted, keyboard.mode()).await?;
    }
    Ok(())
}

async fn run<B: Backend>(
    terminal: &mut Terminal<B>,
    session: &DebugSession,
    args: &Args,
    scripted: bool,
    keyboard_mode: KeyboardMode,
) -> Result<(), Box<dyn std::error::Error>>
where
    B::Error: Send + Sync + 'static,
{
    let mut state = session.load_state_or_else(|| Ok::<_, io::Error>(AppState::new(args.seed)))?;
    if !session.render_once() && !scripted {
        state.player.clear_controls();
    }
    let size = terminal.size()?;
    reducer(
        &mut state,
        Action::ViewportResize {
            width: size.width,
            height: size.height,
        },
    );
    let replay = session.load_replay_items::<Action>()?;
    let (middleware, recorder) = session.middleware_with_recorder();
    #[allow(unused_mut)]
    let mut store = StoreWithMiddleware::new(state, reducer, middleware);
    #[cfg(unix)]
    let remote_debugger = if let Some(path) = &args.debug_socket {
        let collector = DebugCollector::new(serde_json::to_value(store.state())?, 256);
        store.set_observer(collector.observer());
        Some((DebugServer::bind(path, collector.clone()).await?, collector))
    } else {
        None
    };
    let mut ui = GameUi {
        sizing: args.sprite_size,
        components: if args.debug.enabled || args.debug_socket.is_some() {
            ComponentRegistry::enabled()
        } else {
            ComponentRegistry::default()
        },
        ..GameUi::default()
    };
    #[allow(unused_mut)]
    let mut debug_layer = ui.components.debug_layer();
    #[cfg(unix)]
    if let Some((_, collector)) = &remote_debugger {
        debug_layer = debug_layer.with_remote_debug(collector.clone());
    }
    let action_sender = RefCell::new(None::<tokio::sync::mpsc::UnboundedSender<Action>>);
    let output = session
        .run_effect_app(
            terminal,
            store,
            debug_layer,
            replay,
            None,
            Some(Action::Quit),
            |runtime| {
                *action_sender.borrow_mut() = Some(runtime.action_tx());
                if !session.render_once() && !scripted {
                    runtime.subscriptions().interval(
                        "game-clock",
                        Duration::from_millis(TICK_MS),
                        || Action::Tick,
                    );
                }
            },
            |frame, area, state, ctx| {
                // DebugSession intercepts F12 before the input mapper. Queue a
                // store action when it takes focus so swallowed releases cannot
                // leave the ship moving after the overlay closes.
                if !ctx.is_focused() && state.player.held_keys != 0 {
                    if let Some(sender) = action_sender.borrow().as_ref() {
                        let _ = sender.send(Action::PlayerControlsClear);
                    }
                }
                ui.render(frame, area, state);
                #[cfg(unix)]
                if let Some((_, collector)) = &remote_debugger {
                    collector.set_components(ui.components.snapshots());
                }
            },
            |event, state| map_event_with_mode(event, state, keyboard_mode),
            |action| matches!(action, Action::Quit),
            |effect: NoEffect, _ctx| match effect {},
        )
        .await;
    #[cfg(unix)]
    if let Some((server, _)) = remote_debugger {
        server.shutdown().await?;
    }
    let output = output?;
    output.write_render_output()?;
    session.save_actions(recorder.as_ref())?;
    if let Some(path) = &args.debug_state_out {
        std::fs::write(path, serde_json::to_string_pretty(output.state())?)?;
    }
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::{os::unix::fs::DirBuilderExt, time::SystemTime};
    use tui_dispatch_debug::transport::DebugClient;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let unique = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir()
                .join(format!("invaders-remote-{}-{unique}", std::process::id()));
            std::fs::DirBuilder::new()
                .mode(0o700)
                .create(&path)
                .unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[tokio::test]
    async fn remote_debugger_attaches_and_reconnects_while_game_keeps_ticking() {
        let temp = TempDir::new();
        let socket = temp.0.join("game.sock");
        let state_path = temp.0.join("state.json");
        let state = AppState {
            phase: space_invaders::state::Phase::Playing,
            ..AppState::default()
        };
        std::fs::write(&state_path, serde_json::to_vec(&state).unwrap()).unwrap();
        let args = Args::parse_from([
            "space-invaders",
            "--debug-socket",
            socket.to_str().unwrap(),
            "--debug-state-in",
            state_path.to_str().unwrap(),
        ]);
        assert!(
            !args.debug.enabled,
            "remote attachment does not need the overlay"
        );
        let session = DebugSession::new(args.debug.clone());
        let mut terminal = Terminal::new(TestBackend::new(80, MIN_HEIGHT)).unwrap();
        let inspect = async {
            let mut client = loop {
                if let Ok(client) = DebugClient::connect(&socket).await {
                    break client;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            };
            let first = client.snapshot().await.unwrap();
            assert_eq!(first.state["phase"], "Playing");
            let first_tick = first.state["tick"].as_u64().unwrap();
            drop(client);
            let mut reconnected = DebugClient::connect(&socket).await.unwrap();
            loop {
                let next = reconnected.snapshot().await.unwrap();
                if next.state["tick"].as_u64().unwrap() > first_tick {
                    assert!(next
                        .history
                        .iter()
                        .any(|record| record.name == "Tick" && record.changed));
                    assert!(next
                        .history
                        .iter()
                        .all(|record| record.serialization_errors.is_empty()));
                    assert_eq!(next.state["seed"], first.state["seed"]);
                    assert!(
                        next.control.is_some(),
                        "remote app controls must be attached"
                    );
                    let components = next
                        .components
                        .as_ref()
                        .expect("socket mode must publish components without --debug");
                    assert!(components
                        .iter()
                        .any(|component| component.type_name == "GameUi"));
                    assert!(components
                        .iter()
                        .any(|component| component.type_name == "PlayerComponent"));
                    assert!(components
                        .iter()
                        .all(|component| component.last_area.is_some()));
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            use tui_dispatch_debug::control::{ComponentTarget, DebugCommand};
            reconnected
                .command(DebugCommand::SetPaused(true))
                .await
                .unwrap();
            let paused = loop {
                let snapshot = reconnected.snapshot().await.unwrap();
                if snapshot.control.as_ref().unwrap().paused {
                    break snapshot;
                }
                tokio::task::yield_now().await;
            };
            tokio::time::sleep(Duration::from_millis(TICK_MS * 3)).await;
            assert_eq!(
                reconnected.snapshot().await.unwrap().state["tick"],
                paused.state["tick"]
            );
            let player = paused
                .components
                .as_ref()
                .unwrap()
                .iter()
                .find(|component| component.bound_id.as_deref() == Some("player"))
                .unwrap();
            reconnected
                .command(DebugCommand::Highlight(Some(ComponentTarget {
                    raw_id: player.raw_id,
                    bound_id: player.bound_id.clone(),
                })))
                .await
                .unwrap();
            reconnected
                .command(DebugCommand::SetPaused(false))
                .await
                .unwrap();
            loop {
                let snapshot = reconnected.snapshot().await.unwrap();
                if snapshot.state["tick"].as_u64().unwrap() > paused.state["tick"].as_u64().unwrap()
                {
                    break;
                }
                tokio::task::yield_now().await;
            }
        };
        // Exercise the real app runtime without taking over the test terminal.
        // Cancelling it also checks that the socket is cleaned up on early exit.
        tokio::select! {
            result = run(&mut terminal, &session, &args, false, KeyboardMode::Legacy) => {
                panic!("game exited before inspection: {result:?}");
            }
            result = tokio::time::timeout(Duration::from_secs(3), inspect) => {
                result.expect("remote debugger should attach while the game is running");
            }
        }
        assert!(
            !socket.exists(),
            "cancelled app left its debug socket behind"
        );
    }
}
