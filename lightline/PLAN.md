# Lightline

## Summary
Persist the agreed game design under a new project directory named `lightline`, with this plan stored as the source-of-truth markdown before implementation starts.

## Core Vision
`Lightline` is a no-combat, navigation-pressure game where the player stays centered on screen, spends light to move, leaves a reclaimable light trail, and tries to reach an exit to descend to the next level.

## Core Loop
1. Move through a procedurally generated level.
2. Spend light on each step.
3. Leave trail light on traversed tiles.
4. Reclaim trail light when stepping back over trail.
5. Locate and reach the exit.
6. Descend to the next level.

## Locked Mechanics

### Light Economy
- Movement consumes light (`burn_cost` baseline for balancing: `1` per step).
- Movement deposits trail light on traversed tiles.
- Backtracking reclaims available trail light.
- Trail can be cut off by danger events, reducing reclaim potential.

### Danger Modes (Seeded Per Floor)
Each floor deterministically selects one primary danger mode from run seed + floor index:
- `Sound Hunter`
- `Imminent Collapse`

Deterministic selection rule (default):
- `mode_index = hash(seed, floor_index) % 2`
- `0 => Sound Hunter`, `1 => Imminent Collapse`

### Sound Hunter
- The hunter tracks player noise (not emitted light).
- Player steps generate noise pressure; efficient routes reduce exposure.
- Labyrinth routing can juke pursuit but costs more light.

### Imminent Collapse
- Collapse event warns the player before impact.
- Collapse can cut trail segments and invalidate return routes.
- Player chooses between retreating to safer terrain or pushing forward with remaining light.

### No Combat Rule
- No attacks, weapon systems, or HP combat loops in v1.
- Tension comes from route planning, light budgeting, and danger response.

## Procgen Integration (`tui-map`)
Use the map crate as the generation backbone and inject gameplay entities through anchors.

### Generator Contract
- Implement floor generation using `tui-map` procgen contracts in `/Users/stviva/Work/fr/tui-stuff/tui-map/src/procgen.rs`:
  - `MapGenerator<P>`
  - `GenerateRequest<P>`
  - `GeneratedMap`
  - `SpawnAnchor`
  - `AnchorKind`

### Required Anchors
- `PlayerStart`
- `Custom("exit")`
- `Custom("beacon")`
- `Custom("relic")`
- `Custom("switch")`

### Procgen Expectations
- Deterministic outputs for same seed + params.
- Valid traversable route from player start to exit.
- Anchor placement compatible with core loop and danger systems.

## Architecture for New Crate
New crate name: `lightline`.

Expected module layout:
- `action`
- `effect`
- `state`
- `reducer`
- `procgen`
- `ui`
- `danger/hunter`
- `danger/collapse`

State flow baseline:
- `tui-dispatch` action/reducer/effect architecture.
- `state` stores map, player light/trail, floor index, active danger mode, and anchor runtime state.
- `reducer` applies deterministic game rules per move/tick.
- `effect` handles async or deferred operations (e.g., floor generation tasks).

## Milestones
1. Scaffold `lightline` crate and module skeleton.
2. Implement movement + light spend/reclaim economy.
3. Implement deterministic floor procgen + anchor injection.
4. Implement `Sound Hunter` mode.
5. Implement `Imminent Collapse` mode.
6. Implement map rendering polish (darkness, trail readability, danger telegraphing).
7. Add tests and balancing pass.

## Balancing Defaults
- Burn cost default: `1` light per move.
- Reclaim behavior default: reclaim light from stepped trail tiles.
- Floor scaling default:
  - Increasing map complexity/size by floor.
  - Increasing danger intensity by floor.
  - Tuning starting light and recovery opportunities per floor.

## Public API / Interface Additions
1. New crate: `lightline`.
2. New module interfaces for `action`, `effect`, `state`, `reducer`, `procgen`, `ui`, `danger/hunter`, and `danger/collapse`.
3. New procgen params/result contracts based on `/Users/stviva/Work/fr/tui-stuff/tui-map/src/procgen.rs`.

## Test Cases and Scenarios
1. Directory exists at `/Users/stviva/Work/fr/tui-stuff/lightline/`.
2. Plan file exists at `/Users/stviva/Work/fr/tui-stuff/lightline/PLAN.md`.
3. Plan includes locked mechanics: light spend/reclaim, seeded danger modes, no combat.
4. Plan names game as `Lightline` consistently.
5. Plan defines deterministic generation and seeded danger selection.

## Assumptions and Defaults
1. Persistence happens before gameplay code changes.
2. Plan Mode previously prevented file creation; now execution mode allows writing these files.
3. This document is the implementation baseline for upcoming `lightline` development.
