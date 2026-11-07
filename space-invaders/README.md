# Space Invaders

A tiny Unicode arcade game built with **tui-dispatch 0.7.1**, **Artbox 0.2**,
and **Ratatui 0.30**. A classic 55-alien formation, destructible half-cell shields,
a mystery saucer, three lives, and increasingly fast waves.

The `tui-dispatch` crates use Cargo path dependencies from the sibling
`../tui-dispatch` checkout. Its debugger toggles on F12 presses only, so held-key
keyboard support does not close the overlay on key release.

```sh
cargo run --manifest-path space-invaders/Cargo.toml
```

Run commands from the repository root. A terminal of at least **80×40** is
required. **Sprite sizes adapt automatically**: 5×2, 7×3, or 11×4, choosing the
largest variant that fits the entire board. Use `--sprite-size compact` to
keep the original 5×2 silhouettes in a large terminal.
Resize during play without resetting the game. Below the minimum size,
simulation stops until the terminal is large enough again.

| Key | Action |
| --- | --- |
| Enter | Launch; play again after game over |
| ← / → or A / D | Hold to move; release to stop |
| Space | Fire; repeat is limited by the firing cooldown |
| P / Escape | Pause or resume |
| R | Restart with the original seed |
| Q / Ctrl-C | Quit |
| F12 (start with `--debug`) | Inspect state, actions, and frame cells |

Ghostty and other terminals supporting the Kitty keyboard protocol automatically
use **held movement at 50 columns/second**, driven by the 20 ms game tick.
Key repeat does not change speed. The ship crosses the field in about 1.5 seconds,
with immediate start/stop and independent A/D and arrow-key tracking.
Held controls clear on pause, resize, respawn, wave transitions, and debug focus.

Keyboard support is negotiated before the event loop starts; the previous
protocol is restored on exit. Use `--keyboard-mode held` to force the enhanced
protocol, or `--keyboard-mode legacy` for one-column press/repeat input on older
terminals. `--keyboard-mode auto` is the default. Headless runs never send
terminal protocol commands.

Magenta squid score 30, cyan crabs 20, green octopuses 10. Saucers award a
seeded 100–300 points. Shields block shots from either direction, including
your own. Defeated ranks expose the next shooter in each column. A player hit
costs one life, followed by a respawn delay and flashing invulnerability.
An invasion reaching the ship ends the game immediately.

The formation has **five rows of eleven**: one squid row, two crab rows, and
two octopus rows, with four bunkers below. This restores the density of the
classic game while keeping the three original tiny silhouettes. Taito's
[history of Space Invaders](https://artsandculture.google.com/story/the-evolution-of-space-invaders/wAVxCu_0BaX0aw)
documents the 5×11 formation. Both black-and-white and color editions exist;
this game's palette is a terminal interpretation.

## Timing and architecture

One runtime interval subscription dispatches `Tick` every 20 ms. The reducer
advances all gameplay, cooldowns, animations, respawn delays, and wave changes
in fixed steps. It uses `NoEffect`: no entity tasks or sleep effects. Pause
freezes simulation time, including timers and randomness.

The full fleet takes 920 ms per step and accelerates in proportion to surviving
aliens. It drops four vertical units only at screen edges. Enemy shots advance
two units every three ticks, with at most three active shots. Later waves start
lower using the arcade ROM's start-height sequence.

Coordinates use a fixed 80×76 world (vertical units are half terminal rows).
Collisions walk projectile paths in travel order, so thin shields and nearer
targets intercept fast shots. All gameplay state and the random-generator
state are serializable; the sprite atlas is separate.

See [GAMEPLAY.md](GAMEPLAY.md) for sourced arcade specifications, terminal
adaptations, and measured before/after pacing. The first fleet now has room to
advance for over two minutes if left intact; that benchmark isolates invasion
from player deaths caused by shots.

Artbox renders hand-authored size variants and colors into cached grids. A
diagonal gradient blends each sprite's bright highlights through its base color
into deeper shadows. The richer palette has 15% softer contrast and blends across
columns as well as rows. Bunkers shade from lime toward teal. Six-stage radial-gradient explosions expand
from a white flash into a gold burst, orange sparks, and fading red embers over
480 ms. The final ship's explosion finishes behind the game-over modal.
Reusable components in `src/components/` render aliens, the player, shields,
bullets, the saucer, and explosions. `Playfield` composes them in drawing order;
each component receives its entity state plus the layout and timing it needs.
Entity state remains in `AppState`, with all updates driven by the reducer.
A shared sprite component blits non-space cells into Ratatui for entity views,
life icons, and title previews. The shield component draws damaged pixel masks
directly. The game uses Artbox's grid API without its older Ratatui
integration. See [SPRITES.md](SPRITES.md) for the original sketches.

Title, pause, game-over, and wave-clear popups use tui-dispatch's `Modal`
component. Its backdrop dims the entire scene, including the HUD and controls,
while the popup content stays bright. The game follows the terminal background;
borderless charcoal popups match the Weather and Minesweeper forms.

## Scripted debugging

Start with `--debug`, press **F12**, then **C** to inspect components. The view
lists each alien, shield, bullet, saucer, player, and explosion separately,
alongside the HUD, playfield, controls, and phase overlay. Expand an entry or
press Enter for its state, including shield pixels and explosion age. Snapshots
follow the latest rendered frame and discard removed entities; collection is
disabled without `--debug`.

For a standalone debugger on macOS or Linux, expose a Unix socket:

```sh
mkdir -p -m 700 /tmp/space-invaders-debug
cargo run --manifest-path space-invaders/Cargo.toml -- \
  --debug --debug-socket /tmp/space-invaders-debug/game.sock
```

Attach from another terminal, running from this repository root:

```sh
cargo run --manifest-path ../tui-dispatch/Cargo.toml \
  -p tui-dispatch-debug --bin tui-dispatch-debug -- \
  /tmp/space-invaders-debug/game.sock
```

`--debug-socket` works independently of `--debug`. The remote debugger reads the
current state and the last 256 dispatches, including ticks, while the game keeps
running. Disconnect and reconnect without losing that retained history. Add
`--once` to the debugger command for one JSON snapshot. The socket is removed
when the game exits; existing paths are never overwritten. Remote inspection
is read only; component/cell inspection and frame freezing use the in-app F12
overlay.

Replay files contain the same actions emitted by keyboard input, plus `Tick`
and `GameAdvanceTicks(n)`. Fifty ticks equal one second of game time without
sleeping. The live clock is disabled whenever actions are loaded or a single
frame is requested. An interactive replay therefore stays frozen afterward
unless more actions are supplied; normal play runs without `--debug-actions-in`.

Held movement records `PlayerKeyDown` / `PlayerKeyUp` with `Left`, `A`, `Right`,
or `D`, alongside the ticks. For example, `PlayerKeyDown("Right")`, ten ticks,
and `PlayerKeyUp("Right")` moves ten columns and stops. See
`scenarios/held-move.json`. `PlayerMove` remains available as a one-column nudge
for scripts and legacy input. Held-key state and fractional positions are part
of snapshots; older snapshots default those fields to zero.

```json
[
  "GameStart",
  {"PlayerMove": "Right"},
  "PlayerFire",
  {"GameAdvanceTicks": 50},
  "PauseToggle"
]
```

```sh
cargo run --manifest-path space-invaders/Cargo.toml -- \
  --debug-actions-in space-invaders/scenarios/fire.json \
  --debug-render-once

# Larger sprites, deterministic seed, and the resulting state snapshot:
cargo run --manifest-path space-invaders/Cargo.toml -- \
  --seed 123 --width 176 --height 78 --sprite-size auto \
  --debug-actions-in space-invaders/scenarios/formation.json \
  --debug-render-once --debug-state-out /tmp/invaders-state.json

# Inspect an alien explosion in mid-burst:
cargo run --manifest-path space-invaders/Cargo.toml -- \
  --debug-actions-in space-invaders/scenarios/explosion.json \
  --debug-render-once

# Record every action, including ticks, then replay to inspect the final frame:
cargo run --manifest-path space-invaders/Cargo.toml -- \
  --debug-actions-out /tmp/invaders-actions.json
cargo run --manifest-path space-invaders/Cargo.toml -- \
  --debug-actions-in /tmp/invaders-actions.json --debug-render-once

# Export the state and action/replay schemas:
cargo run --manifest-path space-invaders/Cargo.toml -- \
  --debug-render-once \
  --debug-state-schema-out /tmp/invaders-state-schema.json \
  --debug-actions-schema-out /tmp/invaders-actions-schema.json
```

Use `--debug-state-in PATH` to resume a saved snapshot. `--seed` defaults to
`1523456941`; a loaded snapshot supplies its own seed. Headless dimensions
default to 80×40; interactive play uses the real terminal dimensions.
Render-once prints a plain-text frame, so colors are verified separately using
Ratatui buffer styles. A normal terminal displays the Artbox colors.

The framework's `_await` and `_await_any` markers are retained for headless
replays; they wait for dispatched action names, not elapsed time. There is no
external asynchronous work in this version. `scenarios/await-start.json`
demonstrates an action wait; use `GameAdvanceTicks` for game-time waits.

Action recordings include `Tick` by default to preserve exact simulation
timing. Explicit debug include/exclude filters override this behavior;
filtering ticks out prevents exact replay. `ViewportResize { width, height }`
is also an action so a recording preserves periods of resize suspension.

## Checks

```sh
cargo fmt --manifest-path space-invaders/Cargo.toml --check
cargo test --manifest-path space-invaders/Cargo.toml
cargo clippy --manifest-path space-invaders/Cargo.toml --all-targets -- -D warnings
```

Tests exercise gameplay, virtual timer timing, deterministic state/replay,
headless CLI runs, all three sprite sizes, color styles, and small terminals.
