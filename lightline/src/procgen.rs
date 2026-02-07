use std::collections::VecDeque;

use tui_map::core::{MapGrid, MapSize, TileKind};
use tui_map::procgen::{
    AnchorKind, GenError, GenerateRequest, GeneratedMap, MapGenerator, SpawnAnchor,
};

use crate::state::{DangerMode, GeneratedFloor, MapState, RuntimeAnchor, RuntimeAnchorKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FloorGenParams {
    pub floor_index: u32,
    pub danger_mode: DangerMode,
}

// ---------------------------------------------------------------------------
// Seeded PRNG
// ---------------------------------------------------------------------------

struct SeededRng {
    state: u64,
}

impl SeededRng {
    fn new(seed: u64) -> Self {
        Self { state: mix64(seed) }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = mix64(self.state.wrapping_add(0x9e37_79b9_7f4a_7c15));
        self.state
    }

    fn next_bounded(&mut self, bound: u64) -> u64 {
        if bound == 0 {
            return 0;
        }
        self.next_u64() % bound
    }
}

// ---------------------------------------------------------------------------
// Maze grid
// ---------------------------------------------------------------------------

struct MazeGrid {
    cell_w: u16,
    cell_h: u16,
    right_open: Vec<bool>,
    down_open: Vec<bool>,
    visited: Vec<bool>,
}

impl MazeGrid {
    fn new(cell_w: u16, cell_h: u16) -> Self {
        let size = cell_w as usize * cell_h as usize;
        Self {
            cell_w,
            cell_h,
            right_open: vec![false; size],
            down_open: vec![false; size],
            visited: vec![false; size],
        }
    }

    fn index(&self, cx: u16, cy: u16) -> usize {
        cy as usize * self.cell_w as usize + cx as usize
    }

    fn open_wall(&mut self, cx1: u16, cy1: u16, cx2: u16, cy2: u16) {
        if cx2 == cx1 + 1 && cy2 == cy1 {
            let idx = self.index(cx1, cy1);
            self.right_open[idx] = true;
        } else if cx1 == cx2 + 1 && cy1 == cy2 {
            let idx = self.index(cx2, cy2);
            self.right_open[idx] = true;
        } else if cy2 == cy1 + 1 && cx2 == cx1 {
            let idx = self.index(cx1, cy1);
            self.down_open[idx] = true;
        } else if cy1 == cy2 + 1 && cx1 == cx2 {
            let idx = self.index(cx2, cy2);
            self.down_open[idx] = true;
        }
    }

    fn open_wall_count(&self, cx: u16, cy: u16) -> u8 {
        let mut count = 0u8;
        let idx = self.index(cx, cy);
        if cx > 0 && self.right_open[self.index(cx - 1, cy)] {
            count += 1;
        }
        if cx + 1 < self.cell_w && self.right_open[idx] {
            count += 1;
        }
        if cy > 0 && self.down_open[self.index(cx, cy - 1)] {
            count += 1;
        }
        if cy + 1 < self.cell_h && self.down_open[idx] {
            count += 1;
        }
        count
    }
}

fn maze_cell_to_tile(cx: u16, cy: u16) -> (u16, u16) {
    (1 + cx * 2, 1 + cy * 2)
}

fn wall_between_tiles(cx1: u16, cy1: u16, cx2: u16, cy2: u16) -> (u16, u16) {
    let (tx1, ty1) = maze_cell_to_tile(cx1, cy1);
    let (tx2, ty2) = maze_cell_to_tile(cx2, cy2);
    ((tx1 + tx2) / 2, (ty1 + ty2) / 2)
}

// ---------------------------------------------------------------------------
// Maze generation (recursive backtracker via explicit stack)
// ---------------------------------------------------------------------------

fn carve_maze(grid: &mut MazeGrid, rng: &mut SeededRng) {
    let mut stack: Vec<(u16, u16)> = Vec::new();
    grid.visited[0] = true;
    stack.push((0, 0));

    while let Some(&(cx, cy)) = stack.last() {
        let mut neighbors = Vec::with_capacity(4);
        if cx > 0 && !grid.visited[grid.index(cx - 1, cy)] {
            neighbors.push((cx - 1, cy));
        }
        if cx + 1 < grid.cell_w && !grid.visited[grid.index(cx + 1, cy)] {
            neighbors.push((cx + 1, cy));
        }
        if cy > 0 && !grid.visited[grid.index(cx, cy - 1)] {
            neighbors.push((cx, cy - 1));
        }
        if cy + 1 < grid.cell_h && !grid.visited[grid.index(cx, cy + 1)] {
            neighbors.push((cx, cy + 1));
        }

        if neighbors.is_empty() {
            stack.pop();
            continue;
        }

        let pick = rng.next_bounded(neighbors.len() as u64) as usize;
        let (nx, ny) = neighbors[pick];
        grid.open_wall(cx, cy, nx, ny);
        let ni = grid.index(nx, ny);
        grid.visited[ni] = true;
        stack.push((nx, ny));
    }
}

// ---------------------------------------------------------------------------
// Convert maze to tile grid
// ---------------------------------------------------------------------------

fn maze_to_tiles(grid: &MazeGrid, tiles: &mut [TileKind], width: u16) {
    for cy in 0..grid.cell_h {
        for cx in 0..grid.cell_w {
            let (tx, ty) = maze_cell_to_tile(cx, cy);
            tiles[ty as usize * width as usize + tx as usize] = TileKind::Floor;

            let idx = grid.index(cx, cy);
            if cx + 1 < grid.cell_w && grid.right_open[idx] {
                let (wx, wy) = wall_between_tiles(cx, cy, cx + 1, cy);
                tiles[wy as usize * width as usize + wx as usize] = TileKind::Floor;
            }
            if cy + 1 < grid.cell_h && grid.down_open[idx] {
                let (wx, wy) = wall_between_tiles(cx, cy, cx, cy + 1);
                tiles[wy as usize * width as usize + wx as usize] = TileKind::Floor;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Post-processing: rooms, corridor widening, water
// ---------------------------------------------------------------------------

struct Room {
    x: u16,
    y: u16,
    w: u16,
    h: u16,
    center: (u16, u16),
}

fn carve_rooms(
    tiles: &mut [TileKind],
    width: u16,
    height: u16,
    rng: &mut SeededRng,
    floor_index: u32,
) -> Vec<Room> {
    let room_count = 2 + rng.next_bounded(3 + (floor_index as u64 / 3).min(2)) as u16;
    let mut rooms = Vec::new();

    for _ in 0..room_count {
        let rw = 3 + rng.next_bounded(3) as u16;
        let rh = 3 + rng.next_bounded(2) as u16;

        let x_space = width.saturating_sub(rw + 3);
        let y_space = height.saturating_sub(rh + 3);
        if x_space == 0 || y_space == 0 {
            continue;
        }

        for _ in 0..20 {
            let rx = 2 + rng.next_bounded(x_space as u64) as u16;
            let ry = 2 + rng.next_bounded(y_space as u64) as u16;

            if rx + rw >= width - 1 || ry + rh >= height - 1 {
                continue;
            }

            for dy in 0..rh {
                for dx in 0..rw {
                    let idx = (ry + dy) as usize * width as usize + (rx + dx) as usize;
                    tiles[idx] = TileKind::Floor;
                }
            }

            rooms.push(Room {
                x: rx,
                y: ry,
                w: rw,
                h: rh,
                center: (rx + rw / 2, ry + rh / 2),
            });
            break;
        }
    }

    rooms
}

fn widen_corridors(
    tiles: &mut [TileKind],
    width: u16,
    height: u16,
    rng: &mut SeededRng,
    floor_index: u32,
) {
    let widen_chance = 20 + (floor_index as u64).min(15);
    let directions: [(i16, i16); 4] = [(0, -1), (0, 1), (-1, 0), (1, 0)];

    for y in 2..height.saturating_sub(2) {
        for x in 2..width.saturating_sub(2) {
            let idx = y as usize * width as usize + x as usize;
            if tiles[idx] != TileKind::Floor {
                continue;
            }
            if rng.next_bounded(100) >= widen_chance {
                continue;
            }

            let dir_idx = rng.next_bounded(4) as usize;
            let (dx, dy) = directions[dir_idx];
            let nx = (x as i16 + dx) as u16;
            let ny = (y as i16 + dy) as u16;

            if nx == 0 || ny == 0 || nx >= width - 1 || ny >= height - 1 {
                continue;
            }

            let nidx = ny as usize * width as usize + nx as usize;
            if tiles[nidx] == TileKind::Wall {
                tiles[nidx] = TileKind::Floor;
            }
        }
    }
}

fn add_water_patches(tiles: &mut [TileKind], width: u16, rooms: &[Room], rng: &mut SeededRng) {
    for room in rooms {
        if rng.next_bounded(3) == 0 {
            continue;
        }
        let water_count = 1 + rng.next_bounded(3) as u16;
        for _ in 0..water_count {
            let w_space = room.w.saturating_sub(2) as u64;
            let h_space = room.h.saturating_sub(2) as u64;
            if w_space == 0 || h_space == 0 {
                continue;
            }
            let wx = room.x + 1 + rng.next_bounded(w_space) as u16;
            let wy = room.y + 1 + rng.next_bounded(h_space) as u16;
            let idx = wy as usize * width as usize + wx as usize;
            tiles[idx] = TileKind::Water;
        }
    }
}

// ---------------------------------------------------------------------------
// Anchor placement
// ---------------------------------------------------------------------------

fn find_dead_end_in_region(
    grid: &MazeGrid,
    min_cx: u16,
    min_cy: u16,
    max_cx: u16,
    max_cy: u16,
    rng: &mut SeededRng,
) -> Option<(u16, u16)> {
    let mut dead_ends = Vec::new();
    for cy in min_cy..max_cy {
        for cx in min_cx..max_cx {
            if grid.open_wall_count(cx, cy) == 1 {
                dead_ends.push((cx, cy));
            }
        }
    }
    if dead_ends.is_empty() {
        return None;
    }
    let pick = rng.next_bounded(dead_ends.len() as u64) as usize;
    Some(dead_ends[pick])
}

fn find_any_cell_in_region(
    grid: &MazeGrid,
    min_cx: u16,
    min_cy: u16,
    max_cx: u16,
    max_cy: u16,
    rng: &mut SeededRng,
) -> (u16, u16) {
    let w = max_cx.saturating_sub(min_cx).max(1);
    let h = max_cy.saturating_sub(min_cy).max(1);
    let cx = min_cx + rng.next_bounded(w as u64) as u16;
    let cy = min_cy + rng.next_bounded(h as u64) as u16;
    (cx.min(grid.cell_w - 1), cy.min(grid.cell_h - 1))
}

#[allow(clippy::type_complexity)]
fn place_anchors(
    grid: &MazeGrid,
    rooms: &[Room],
    rng: &mut SeededRng,
) -> ((u16, u16), (u16, u16), (u16, u16), (u16, u16), (u16, u16)) {
    let half_w = grid.cell_w / 2;
    let half_h = grid.cell_h / 2;

    // Player start: dead end in top-left quadrant
    let start_cell = find_dead_end_in_region(grid, 0, 0, half_w, half_h, rng)
        .unwrap_or_else(|| find_any_cell_in_region(grid, 0, 0, half_w, half_h, rng));
    let player_start = maze_cell_to_tile(start_cell.0, start_cell.1);

    // Exit: dead end in bottom-right quadrant
    let exit_cell = find_dead_end_in_region(grid, half_w, half_h, grid.cell_w, grid.cell_h, rng)
        .unwrap_or_else(|| {
            find_any_cell_in_region(grid, half_w, half_h, grid.cell_w, grid.cell_h, rng)
        });
    let exit = maze_cell_to_tile(exit_cell.0, exit_cell.1);

    // Beacon: first room center or mid-map
    let beacon = rooms
        .first()
        .map(|r| r.center)
        .unwrap_or_else(|| maze_cell_to_tile(half_w, half_h));

    // Relic: second room or dead end in top-right
    let relic = rooms.get(1).map(|r| r.center).unwrap_or_else(|| {
        find_dead_end_in_region(grid, half_w, 0, grid.cell_w, half_h, rng)
            .map(|(cx, cy)| maze_cell_to_tile(cx, cy))
            .unwrap_or_else(|| {
                let c = find_any_cell_in_region(grid, half_w, 0, grid.cell_w, half_h, rng);
                maze_cell_to_tile(c.0, c.1)
            })
    });

    // Switch: third room or dead end in bottom-left
    let switch = rooms.get(2).map(|r| r.center).unwrap_or_else(|| {
        find_dead_end_in_region(grid, 0, half_h, half_w, grid.cell_h, rng)
            .map(|(cx, cy)| maze_cell_to_tile(cx, cy))
            .unwrap_or_else(|| {
                let c = find_any_cell_in_region(grid, 0, half_h, half_w, grid.cell_h, rng);
                maze_cell_to_tile(c.0, c.1)
            })
    });

    (player_start, exit, beacon, relic, switch)
}

// ---------------------------------------------------------------------------
// Connectivity validation (BFS)
// ---------------------------------------------------------------------------

fn validate_connectivity(
    tiles: &[TileKind],
    width: u16,
    height: u16,
    start: (u16, u16),
    end: (u16, u16),
) -> bool {
    let idx = |x: u16, y: u16| y as usize * width as usize + x as usize;
    let is_passable = |x: u16, y: u16| {
        matches!(
            tiles[idx(x, y)],
            TileKind::Floor | TileKind::Trail | TileKind::Water
        )
    };

    let mut visited = vec![false; tiles.len()];
    let mut queue = VecDeque::new();

    visited[idx(start.0, start.1)] = true;
    queue.push_back(start);

    while let Some((x, y)) = queue.pop_front() {
        if (x, y) == end {
            return true;
        }
        for (dx, dy) in [(0i16, -1), (0, 1), (-1, 0), (1, 0)] {
            let nx = x as i16 + dx;
            let ny = y as i16 + dy;
            if nx < 0 || ny < 0 || nx >= width as i16 || ny >= height as i16 {
                continue;
            }
            let (nx, ny) = (nx as u16, ny as u16);
            let ni = idx(nx, ny);
            if !visited[ni] && is_passable(nx, ny) {
                visited[ni] = true;
                queue.push_back((nx, ny));
            }
        }
    }

    false
}

// ---------------------------------------------------------------------------
// Generator
// ---------------------------------------------------------------------------

pub struct LightlineGenerator;

impl MapGenerator<FloorGenParams> for LightlineGenerator {
    fn id(&self) -> &'static str {
        "lightline-floor"
    }

    fn version(&self) -> u32 {
        2
    }

    fn generate(&self, req: &GenerateRequest<FloorGenParams>) -> Result<GeneratedMap, GenError> {
        if req.width < 20 || req.height < 12 {
            return Err(GenError::InvalidSize);
        }

        let width = req.width;
        let height = req.height;
        let mut rng = SeededRng::new(req.seed ^ ((req.params.floor_index as u64) << 32));

        // Maze dimensions: each cell is 1 tile with 1-tile walls between
        let cell_w = (width - 1) / 2;
        let cell_h = (height - 1) / 2;
        let mut maze = MazeGrid::new(cell_w, cell_h);

        // Carve corridors
        carve_maze(&mut maze, &mut rng);

        // Convert to tile grid (starts all-wall)
        let mut tiles = vec![TileKind::Wall; width as usize * height as usize];
        maze_to_tiles(&maze, &mut tiles, width);

        // Carve rooms
        let rooms = carve_rooms(&mut tiles, width, height, &mut rng, req.params.floor_index);

        // Widen some corridors
        widen_corridors(&mut tiles, width, height, &mut rng, req.params.floor_index);

        // Water patches in rooms
        add_water_patches(&mut tiles, width, &rooms, &mut rng);

        // Enforce border walls
        for x in 0..width {
            tiles[x as usize] = TileKind::Wall;
            tiles[(height - 1) as usize * width as usize + x as usize] = TileKind::Wall;
        }
        for y in 0..height {
            tiles[y as usize * width as usize] = TileKind::Wall;
            tiles[y as usize * width as usize + (width - 1) as usize] = TileKind::Wall;
        }

        // Place anchors
        let (player_start, exit, beacon, relic, switch) = place_anchors(&maze, &rooms, &mut rng);

        // Ensure anchor tiles are passable
        for (x, y) in [player_start, exit, beacon, relic, switch] {
            let idx = y as usize * width as usize + x as usize;
            tiles[idx] = TileKind::Trail;
        }

        // Validate connectivity
        if !validate_connectivity(&tiles, width, height, player_start, exit) {
            return Err(GenError::Internal("no path from start to exit".to_string()));
        }

        let map = MapGrid::new(
            format!("Lightline Floor {}", req.params.floor_index + 1),
            MapSize::new(width, height),
            tiles,
        )
        .map_err(|err| GenError::Internal(err.to_string()))?;

        let anchors = vec![
            SpawnAnchor {
                kind: AnchorKind::PlayerStart,
                x: player_start.0,
                y: player_start.1,
                tag: None,
            },
            SpawnAnchor {
                kind: AnchorKind::Custom("exit".to_string()),
                x: exit.0,
                y: exit.1,
                tag: None,
            },
            SpawnAnchor {
                kind: AnchorKind::Custom("beacon".to_string()),
                x: beacon.0,
                y: beacon.1,
                tag: None,
            },
            SpawnAnchor {
                kind: AnchorKind::Custom("relic".to_string()),
                x: relic.0,
                y: relic.1,
                tag: None,
            },
            SpawnAnchor {
                kind: AnchorKind::Custom("switch".to_string()),
                x: switch.0,
                y: switch.1,
                tag: None,
            },
        ];

        Ok(GeneratedMap::with_computed_fingerprint(
            self.id(),
            self.version(),
            req.seed,
            map,
            anchors,
        ))
    }
}

// ---------------------------------------------------------------------------
// Shared utilities
// ---------------------------------------------------------------------------

pub fn choose_danger_mode(seed: u64, floor_index: u32) -> DangerMode {
    if mix64(seed ^ (floor_index as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15)) & 1 == 0 {
        DangerMode::SoundHunter
    } else {
        DangerMode::ImminentCollapse
    }
}

pub fn generate_floor(
    run_seed: u64,
    floor_index: u32,
    width: u16,
    height: u16,
) -> Result<GeneratedFloor, GenError> {
    let generator = LightlineGenerator;
    let danger_mode = choose_danger_mode(run_seed, floor_index);
    let seed = mix64(run_seed ^ ((floor_index as u64) << 1));

    let req = GenerateRequest {
        generator_id: generator.id().to_string(),
        generator_version: generator.version(),
        seed,
        width,
        height,
        params: FloorGenParams {
            floor_index,
            danger_mode,
        },
    };

    let generated = generator.generate(&req)?;
    Ok(into_runtime_floor(generated, danger_mode))
}

fn into_runtime_floor(generated: GeneratedMap, danger_mode: DangerMode) -> GeneratedFloor {
    let map = MapState::from_grid(generated.map);
    let anchors = generated
        .anchors
        .into_iter()
        .filter_map(runtime_anchor)
        .collect();

    GeneratedFloor {
        map,
        anchors,
        danger_mode,
        generator_id: generated.fingerprint.generator_id,
        generator_version: generated.fingerprint.generator_version,
        seed: generated.fingerprint.seed,
        fingerprint: generated.fingerprint.output_hash_hex,
    }
}

fn runtime_anchor(anchor: SpawnAnchor) -> Option<RuntimeAnchor> {
    let SpawnAnchor { kind, x, y, tag } = anchor;
    let kind = match kind {
        AnchorKind::PlayerStart => RuntimeAnchorKind::PlayerStart,
        AnchorKind::Custom(name) => match name.as_str() {
            "exit" => RuntimeAnchorKind::Exit,
            "beacon" => RuntimeAnchorKind::Beacon,
            "relic" => RuntimeAnchorKind::Relic,
            "switch" => RuntimeAnchorKind::Switch,
            _ => return None,
        },
        _ => return None,
    };

    Some(RuntimeAnchor { kind, x, y, tag })
}

fn mix64(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::Tile;
    use std::collections::HashSet;

    #[test]
    fn danger_mode_is_deterministic() {
        let a = choose_danger_mode(42, 7);
        let b = choose_danger_mode(42, 7);
        assert_eq!(a, b);
    }

    #[test]
    fn generated_floor_has_required_anchors() {
        let floor = generate_floor(77, 0, 36, 24).expect("floor");
        let kinds: HashSet<RuntimeAnchorKind> = floor.anchors.iter().map(|a| a.kind).collect();
        assert!(kinds.contains(&RuntimeAnchorKind::PlayerStart));
        assert!(kinds.contains(&RuntimeAnchorKind::Exit));
        assert!(kinds.contains(&RuntimeAnchorKind::Beacon));
        assert!(kinds.contains(&RuntimeAnchorKind::Relic));
        assert!(kinds.contains(&RuntimeAnchorKind::Switch));
    }

    #[test]
    fn fingerprint_stays_stable_for_same_inputs() {
        let a = generate_floor(99, 2, 40, 26).expect("floor a");
        let b = generate_floor(99, 2, 40, 26).expect("floor b");
        assert_eq!(a.fingerprint, b.fingerprint);
    }

    #[test]
    fn labyrinth_has_path_from_start_to_exit() {
        for seed in [42u64, 123, 999, 7777] {
            let floor = generate_floor(seed, 0, 36, 24).expect("floor");
            let start = floor
                .anchors
                .iter()
                .find(|a| a.kind == RuntimeAnchorKind::PlayerStart)
                .unwrap();
            let exit = floor
                .anchors
                .iter()
                .find(|a| a.kind == RuntimeAnchorKind::Exit)
                .unwrap();
            assert!(
                has_path(&floor.map, (start.x, start.y), (exit.x, exit.y)),
                "seed {seed}: no path from start to exit"
            );
        }
    }

    #[test]
    fn labyrinth_has_walls_on_border() {
        let floor = generate_floor(42, 0, 36, 24).expect("floor");
        for x in 0..floor.map.width {
            assert_eq!(floor.map.tile(x, 0), Tile::Wall);
            assert_eq!(floor.map.tile(x, floor.map.height - 1), Tile::Wall);
        }
        for y in 0..floor.map.height {
            assert_eq!(floor.map.tile(0, y), Tile::Wall);
            assert_eq!(floor.map.tile(floor.map.width - 1, y), Tile::Wall);
        }
    }

    #[test]
    fn labyrinth_generation_is_deterministic() {
        let a = generate_floor(555, 3, 40, 28).expect("a");
        let b = generate_floor(555, 3, 40, 28).expect("b");
        assert_eq!(a.map.tiles, b.map.tiles);
        assert_eq!(a.fingerprint, b.fingerprint);
    }

    #[test]
    fn different_seeds_produce_different_mazes() {
        let a = generate_floor(111, 0, 36, 24).expect("a");
        let b = generate_floor(222, 0, 36, 24).expect("b");
        assert_ne!(a.map.tiles, b.map.tiles);
    }

    fn has_path(map: &MapState, start: (u16, u16), end: (u16, u16)) -> bool {
        let mut visited = vec![false; map.width as usize * map.height as usize];
        let mut queue = VecDeque::new();
        let idx = |x: u16, y: u16| y as usize * map.width as usize + x as usize;

        visited[idx(start.0, start.1)] = true;
        queue.push_back(start);

        while let Some((x, y)) = queue.pop_front() {
            if (x, y) == end {
                return true;
            }
            for (dx, dy) in [(0i16, -1), (0, 1), (-1, 0), (1, 0)] {
                let nx = x as i16 + dx;
                let ny = y as i16 + dy;
                if nx < 0 || ny < 0 || nx >= map.width as i16 || ny >= map.height as i16 {
                    continue;
                }
                let (nx, ny) = (nx as u16, ny as u16);
                let ni = idx(nx, ny);
                if !visited[ni] && map.is_walkable(nx, ny) {
                    visited[ni] = true;
                    queue.push_back((nx, ny));
                }
            }
        }

        false
    }
}
