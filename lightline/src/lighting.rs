use std::{cmp::Ordering, collections::BinaryHeap};

use ratatui::{buffer::Buffer, style::Color};
use tui_map::render::MapRenderResult;

use crate::state::MapState;

// Lighting tuneables (gameplay/visual knobs):
// - FALL_OFF_EXPONENT: direct light drop-off steepness from each source.
// - RAY_ANGLE_EPS: anti-gap micro-jitter for each sampled ray.
// - WALL_BOUNCE_FACTOR: amount of 1-step reflected light from wall faces.
// - GAMMA/MIN_VISIBLE: transfer curve from tile brightness to rendered darkness.
// - DDA_TIE_EPS: corner-tie tolerance in ray traversal.
// - FLOOR_PROPAGATE_*: post-pass diffusion strength and cutoff.
const FALL_OFF_EXPONENT: f32 = 1.35;
const RAY_ANGLE_EPS: f32 = 0.0008;
const WALL_BOUNCE_FACTOR: f32 = 0.05;
const GAMMA: f32 = 0.65;
const MIN_VISIBLE: f32 = 0.0;
const DDA_TIE_EPS: f32 = 0.001;
const FLOOR_PROPAGATE_CARDINAL_DECAY: f32 = 0.64;
const FLOOR_PROPAGATE_DIAGONAL_DECAY: f32 = 0.46;
const FLOOR_PROPAGATE_CUTOFF: f32 = 0.01;
const FLOOR_PROPAGATE_EPS: f32 = 0.0001;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LightSource {
    pub x: u16,
    pub y: u16,
    pub intensity: f32,
    pub range: u16,
    pub core_radius: u16,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LightField {
    pub start_x: u16,
    pub start_y: u16,
    pub width: u16,
    pub height: u16,
    pub brightness: Vec<f32>,
}

impl LightField {
    pub fn new(start_x: u16, start_y: u16, width: u16, height: u16) -> Self {
        Self {
            start_x,
            start_y,
            width,
            height,
            brightness: vec![0.0; width as usize * height as usize],
        }
    }

    pub fn brightness_at(&self, map_x: u16, map_y: u16) -> f32 {
        let Some(idx) = self.local_index(map_x, map_y) else {
            return 0.0;
        };
        self.brightness[idx]
    }

    fn local_index(&self, map_x: u16, map_y: u16) -> Option<usize> {
        local_index_in_view(
            self.start_x,
            self.start_y,
            self.width,
            self.height,
            map_x as i32,
            map_y as i32,
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WallFace {
    North,
    South,
    East,
    West,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct FaceLight {
    north: f32,
    south: f32,
    east: f32,
    west: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct LightNode {
    brightness: f32,
    idx: usize,
}

impl Eq for LightNode {}

impl Ord for LightNode {
    fn cmp(&self, other: &Self) -> Ordering {
        self.brightness
            .total_cmp(&other.brightness)
            .then_with(|| self.idx.cmp(&other.idx))
    }
}

impl PartialOrd for LightNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl FaceLight {
    fn set_max(&mut self, face: WallFace, value: f32) {
        match face {
            WallFace::North => self.north = self.north.max(value),
            WallFace::South => self.south = self.south.max(value),
            WallFace::East => self.east = self.east.max(value),
            WallFace::West => self.west = self.west.max(value),
        }
    }

    fn add_clamped(&mut self, face: WallFace, value: f32) {
        match face {
            WallFace::North => add_clamped(&mut self.north, value),
            WallFace::South => add_clamped(&mut self.south, value),
            WallFace::East => add_clamped(&mut self.east, value),
            WallFace::West => add_clamped(&mut self.west, value),
        }
    }

    fn value(self, face: WallFace) -> f32 {
        match face {
            WallFace::North => self.north,
            WallFace::South => self.south,
            WallFace::East => self.east,
            WallFace::West => self.west,
        }
    }
}

pub fn compute_light_field(
    map: &MapState,
    start_x: u16,
    start_y: u16,
    width: u16,
    height: u16,
    sources: &[LightSource],
) -> LightField {
    let mut field = LightField::new(start_x, start_y, width, height);
    if width == 0 || height == 0 || map.width == 0 || map.height == 0 {
        return field;
    }

    let len = width as usize * height as usize;
    let mut floor_light = vec![0.0_f32; len];
    let mut wall_light = vec![0.0_f32; len];
    let mut face_lights = vec![FaceLight::default(); len];

    for source in sources {
        if source.intensity <= 0.0 || source.range == 0 {
            continue;
        }
        if source.x >= map.width || source.y >= map.height {
            continue;
        }

        accumulate_direct_from_source(
            map,
            source,
            start_x,
            start_y,
            width,
            height,
            &mut floor_light,
            &mut wall_light,
            &mut face_lights,
        );
    }

    apply_directional_wall_bounce(
        map,
        start_x,
        start_y,
        width,
        height,
        &face_lights,
        &mut floor_light,
    );
    propagate_floor_light(map, start_x, start_y, width, height, &mut floor_light);

    for local_y in 0..height {
        for local_x in 0..width {
            let idx = local_y as usize * width as usize + local_x as usize;
            let map_x = start_x + local_x;
            let map_y = start_y + local_y;
            field.brightness[idx] = if map.is_light_blocker(map_x, map_y) {
                wall_light[idx]
            } else {
                floor_light[idx]
            };
        }
    }

    field
}

fn propagate_floor_light(
    map: &MapState,
    start_x: u16,
    start_y: u16,
    width: u16,
    height: u16,
    floor_light: &mut [f32],
) {
    let mut heap = BinaryHeap::new();
    for (idx, &brightness) in floor_light.iter().enumerate() {
        if brightness > FLOOR_PROPAGATE_CUTOFF {
            heap.push(LightNode { brightness, idx });
        }
    }

    while let Some(node) = heap.pop() {
        if node.brightness + FLOOR_PROPAGATE_EPS < floor_light[node.idx] {
            continue;
        }

        let local_x = (node.idx % width as usize) as i32;
        let local_y = (node.idx / width as usize) as i32;
        let map_x = start_x as i32 + local_x;
        let map_y = start_y as i32 + local_y;

        for (dx, dy) in [
            (-1, 0),
            (1, 0),
            (0, -1),
            (0, 1),
            (-1, -1),
            (1, -1),
            (-1, 1),
            (1, 1),
        ] {
            let tx = map_x + dx;
            let ty = map_y + dy;
            if tx < 0 || ty < 0 || tx >= map.width as i32 || ty >= map.height as i32 {
                continue;
            }
            if map.is_light_blocker(tx as u16, ty as u16) {
                continue;
            }

            // Prevent squeezing diffuse light through a fully blocked corner pinch.
            if dx != 0 && dy != 0 {
                let side_a_blocked = map.is_light_blocker((map_x + dx) as u16, map_y as u16);
                let side_b_blocked = map.is_light_blocker(map_x as u16, (map_y + dy) as u16);
                if side_a_blocked && side_b_blocked {
                    continue;
                }
            }

            let Some(tidx) = local_index_in_view(start_x, start_y, width, height, tx, ty) else {
                continue;
            };

            let decay = if dx != 0 && dy != 0 {
                FLOOR_PROPAGATE_DIAGONAL_DECAY
            } else {
                FLOOR_PROPAGATE_CARDINAL_DECAY
            };
            let candidate = node.brightness * decay;
            if candidate <= FLOOR_PROPAGATE_CUTOFF {
                continue;
            }

            if candidate > floor_light[tidx] + FLOOR_PROPAGATE_EPS {
                floor_light[tidx] = candidate.min(1.0);
                heap.push(LightNode {
                    brightness: floor_light[tidx],
                    idx: tidx,
                });
            }
        }
    }
}

fn accumulate_direct_from_source(
    map: &MapState,
    source: &LightSource,
    start_x: u16,
    start_y: u16,
    width: u16,
    height: u16,
    floor_light: &mut [f32],
    wall_light: &mut [f32],
    face_lights: &mut [FaceLight],
) {
    let len = width as usize * height as usize;
    let mut source_floor = vec![0.0_f32; len];
    let mut source_wall = vec![0.0_f32; len];
    let mut source_faces = vec![FaceLight::default(); len];

    if !map.is_light_blocker(source.x, source.y) {
        if let Some(idx) = local_index_in_view(
            start_x,
            start_y,
            width,
            height,
            source.x as i32,
            source.y as i32,
        ) {
            source_floor[idx] = source_floor[idx].max(source.intensity.min(1.0));
        }
    }

    for angle in sampled_angles(map, source) {
        for angle_offset in [-RAY_ANGLE_EPS, 0.0, RAY_ANGLE_EPS] {
            cast_ray_dda(
                map,
                source,
                angle + angle_offset,
                start_x,
                start_y,
                width,
                height,
                &mut source_floor,
                &mut source_wall,
                &mut source_faces,
            );
        }
    }

    for idx in 0..len {
        add_clamped(&mut floor_light[idx], source_floor[idx]);
        add_clamped(&mut wall_light[idx], source_wall[idx]);

        let source_face = source_faces[idx];
        if source_face.north > 0.0 {
            face_lights[idx].add_clamped(WallFace::North, source_face.north);
        }
        if source_face.south > 0.0 {
            face_lights[idx].add_clamped(WallFace::South, source_face.south);
        }
        if source_face.east > 0.0 {
            face_lights[idx].add_clamped(WallFace::East, source_face.east);
        }
        if source_face.west > 0.0 {
            face_lights[idx].add_clamped(WallFace::West, source_face.west);
        }
    }
}

fn cast_ray_dda(
    map: &MapState,
    source: &LightSource,
    angle: f32,
    start_x: u16,
    start_y: u16,
    width: u16,
    height: u16,
    source_floor: &mut [f32],
    source_wall: &mut [f32],
    source_faces: &mut [FaceLight],
) {
    let origin_x = source.x as f32 + 0.5;
    let origin_y = source.y as f32 + 0.5;
    let dir_x = angle.cos();
    let dir_y = angle.sin();

    let step_x = if dir_x > 0.0 {
        1
    } else if dir_x < 0.0 {
        -1
    } else {
        0
    };
    let step_y = if dir_y > 0.0 {
        1
    } else if dir_y < 0.0 {
        -1
    } else {
        0
    };

    if step_x == 0 && step_y == 0 {
        return;
    }

    let delta_dist_x = if step_x == 0 {
        f32::INFINITY
    } else {
        1.0 / dir_x.abs()
    };
    let delta_dist_y = if step_y == 0 {
        f32::INFINITY
    } else {
        1.0 / dir_y.abs()
    };

    let mut map_x = source.x as i32;
    let mut map_y = source.y as i32;

    let mut side_dist_x = if step_x > 0 {
        ((map_x + 1) as f32 - origin_x) * delta_dist_x
    } else if step_x < 0 {
        (origin_x - map_x as f32) * delta_dist_x
    } else {
        f32::INFINITY
    };

    let mut side_dist_y = if step_y > 0 {
        ((map_y + 1) as f32 - origin_y) * delta_dist_y
    } else if step_y < 0 {
        (origin_y - map_y as f32) * delta_dist_y
    } else {
        f32::INFINITY
    };

    let max_range = source.range as f32;

    let x_face = if step_x > 0 {
        WallFace::West
    } else {
        WallFace::East
    };
    let y_face = if step_y > 0 {
        WallFace::North
    } else {
        WallFace::South
    };

    loop {
        let cmp = side_dist_x - side_dist_y;
        if cmp.abs() <= DDA_TIE_EPS {
            // Corner crossing: inspect both side-adjacent cells so we don't miss
            // visibility along exact grid-corner rays.
            let traveled = side_dist_x;
            if traveled > max_range + 0.0001 {
                break;
            }

            let side_x = (map_x + step_x, map_y);
            let side_y = (map_x, map_y + step_y);

            let x_blocked = process_side_cell(
                map,
                side_x.0,
                side_x.1,
                x_face,
                source,
                origin_x,
                origin_y,
                max_range,
                start_x,
                start_y,
                width,
                height,
                source_floor,
                source_wall,
                source_faces,
            );
            let y_blocked = process_side_cell(
                map,
                side_y.0,
                side_y.1,
                y_face,
                source,
                origin_x,
                origin_y,
                max_range,
                start_x,
                start_y,
                width,
                height,
                source_floor,
                source_wall,
                source_faces,
            );

            // True corner pinch blocks the ray.
            if x_blocked && y_blocked {
                break;
            }

            map_x += step_x;
            map_y += step_y;
            side_dist_x += delta_dist_x;
            side_dist_y += delta_dist_y;

            if map_x < 0 || map_y < 0 || map_x >= map.width as i32 || map_y >= map.height as i32 {
                break;
            }

            let Some(contribution) =
                contribution_for_cell(source, origin_x, origin_y, map_x, map_y, max_range)
            else {
                continue;
            };
            if map.is_light_blocker(map_x as u16, map_y as u16) {
                if let Some(idx) =
                    local_index_in_view(start_x, start_y, width, height, map_x, map_y)
                {
                    source_wall[idx] = source_wall[idx].max(contribution);
                    // Diagonal/corner entry can hit two faces at once.
                    source_faces[idx].set_max(x_face, contribution);
                    source_faces[idx].set_max(y_face, contribution);
                }
                break;
            }
            if let Some(idx) = local_index_in_view(start_x, start_y, width, height, map_x, map_y) {
                source_floor[idx] = source_floor[idx].max(contribution);
            }
            continue;
        }

        let (traveled, entered_face) = if cmp < 0.0 {
            map_x += step_x;
            let traveled = side_dist_x;
            side_dist_x += delta_dist_x;
            (traveled, x_face)
        } else {
            map_y += step_y;
            let traveled = side_dist_y;
            side_dist_y += delta_dist_y;
            (traveled, y_face)
        };

        if traveled > max_range + 0.0001 {
            break;
        }

        if map_x < 0 || map_y < 0 || map_x >= map.width as i32 || map_y >= map.height as i32 {
            break;
        }

        let Some(contribution) =
            contribution_for_cell(source, origin_x, origin_y, map_x, map_y, max_range)
        else {
            continue;
        };

        if map.is_light_blocker(map_x as u16, map_y as u16) {
            if let Some(idx) = local_index_in_view(start_x, start_y, width, height, map_x, map_y) {
                source_wall[idx] = source_wall[idx].max(contribution);
                source_faces[idx].set_max(entered_face, contribution);
            }
            break;
        }

        if let Some(idx) = local_index_in_view(start_x, start_y, width, height, map_x, map_y) {
            source_floor[idx] = source_floor[idx].max(contribution);
        }
    }
}

fn process_side_cell(
    map: &MapState,
    tile_x: i32,
    tile_y: i32,
    entered_face: WallFace,
    source: &LightSource,
    origin_x: f32,
    origin_y: f32,
    max_range: f32,
    start_x: u16,
    start_y: u16,
    width: u16,
    height: u16,
    source_floor: &mut [f32],
    source_wall: &mut [f32],
    source_faces: &mut [FaceLight],
) -> bool {
    if tile_x < 0 || tile_y < 0 || tile_x >= map.width as i32 || tile_y >= map.height as i32 {
        return true;
    }

    let Some(contribution) =
        contribution_for_cell(source, origin_x, origin_y, tile_x, tile_y, max_range)
    else {
        return false;
    };

    if map.is_light_blocker(tile_x as u16, tile_y as u16) {
        if let Some(idx) = local_index_in_view(start_x, start_y, width, height, tile_x, tile_y) {
            source_wall[idx] = source_wall[idx].max(contribution);
            source_faces[idx].set_max(entered_face, contribution);
        }
        return true;
    }

    if let Some(idx) = local_index_in_view(start_x, start_y, width, height, tile_x, tile_y) {
        source_floor[idx] = source_floor[idx].max(contribution);
    }
    false
}

fn contribution_for_cell(
    source: &LightSource,
    origin_x: f32,
    origin_y: f32,
    tile_x: i32,
    tile_y: i32,
    max_range: f32,
) -> Option<f32> {
    let center_x = tile_x as f32 + 0.5;
    let center_y = tile_y as f32 + 0.5;
    let distance = ((center_x - origin_x).powi(2) + (center_y - origin_y).powi(2)).sqrt();
    if distance > max_range + 0.0001 {
        return None;
    }

    let contribution = source.intensity * falloff(distance, max_range, source.core_radius as f32);
    if contribution <= 0.0 {
        None
    } else {
        Some(contribution)
    }
}

fn apply_directional_wall_bounce(
    map: &MapState,
    start_x: u16,
    start_y: u16,
    width: u16,
    height: u16,
    face_lights: &[FaceLight],
    floor_light: &mut [f32],
) {
    for local_y in 0..height as i32 {
        for local_x in 0..width as i32 {
            let map_x = start_x as i32 + local_x;
            let map_y = start_y as i32 + local_y;
            if map_x < 0 || map_y < 0 || map_x >= map.width as i32 || map_y >= map.height as i32 {
                continue;
            }
            if !map.is_light_blocker(map_x as u16, map_y as u16) {
                continue;
            }

            let Some(wall_idx) = local_index_in_view(start_x, start_y, width, height, map_x, map_y)
            else {
                continue;
            };

            let faces = face_lights[wall_idx];
            for face in [
                WallFace::North,
                WallFace::South,
                WallFace::East,
                WallFace::West,
            ] {
                let face_energy = faces.value(face);
                if face_energy <= 0.0 {
                    continue;
                }

                let (tx, ty) = match face {
                    WallFace::North => (map_x, map_y - 1),
                    WallFace::South => (map_x, map_y + 1),
                    WallFace::East => (map_x + 1, map_y),
                    WallFace::West => (map_x - 1, map_y),
                };

                if tx < 0 || ty < 0 || tx >= map.width as i32 || ty >= map.height as i32 {
                    continue;
                }
                if map.is_light_blocker(tx as u16, ty as u16) {
                    continue;
                }

                if let Some(target_idx) =
                    local_index_in_view(start_x, start_y, width, height, tx, ty)
                {
                    add_clamped(
                        &mut floor_light[target_idx],
                        face_energy * WALL_BOUNCE_FACTOR,
                    );
                }
            }
        }
    }
}

fn sampled_angles(map: &MapState, source: &LightSource) -> Vec<f32> {
    let sx = source.x as i32;
    let sy = source.y as i32;
    let range = source.range as i32;

    let min_x = (sx - range).max(0);
    let max_x = (sx + range).min(map.width as i32 - 1);
    let min_y = (sy - range).max(0);
    let max_y = (sy + range).min(map.height as i32 - 1);

    let origin_x = sx as f32 + 0.5;
    let origin_y = sy as f32 + 0.5;
    let mut angles = Vec::new();

    let mut push_angle = |target_x: f32, target_y: f32| {
        let dx = target_x - origin_x;
        let dy = target_y - origin_y;
        if dx.abs() < f32::EPSILON && dy.abs() < f32::EPSILON {
            return;
        }
        angles.push(dy.atan2(dx));
    };

    let range_f = source.range as f32 + 1.0;
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            // Ignore tiles clearly outside influence to cap ray count.
            let cx = x as f32 + 0.5;
            let cy = y as f32 + 0.5;
            let dx = cx - origin_x;
            let dy = cy - origin_y;
            if (dx * dx + dy * dy).sqrt() > range_f {
                continue;
            }

            // Center and 4 corners: dense enough to eliminate missing-lit holes.
            push_angle(cx, cy);
            push_angle(x as f32, y as f32);
            push_angle((x + 1) as f32, y as f32);
            push_angle(x as f32, (y + 1) as f32);
            push_angle((x + 1) as f32, (y + 1) as f32);
        }
    }

    angles.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
    angles.dedup_by(|a, b| (*a - *b).abs() <= 0.000001);
    angles
}

fn local_index_in_view(
    start_x: u16,
    start_y: u16,
    width: u16,
    height: u16,
    map_x: i32,
    map_y: i32,
) -> Option<usize> {
    if map_x < start_x as i32 || map_y < start_y as i32 {
        return None;
    }

    let local_x = map_x - start_x as i32;
    let local_y = map_y - start_y as i32;
    if local_x < 0 || local_y < 0 || local_x >= width as i32 || local_y >= height as i32 {
        return None;
    }

    Some(local_y as usize * width as usize + local_x as usize)
}

fn add_clamped(value: &mut f32, delta: f32) {
    if delta <= 0.0 {
        return;
    }
    *value = (*value + delta).min(1.0);
}

pub fn apply_light_field_to_buffer(
    buf: &mut Buffer,
    render: MapRenderResult,
    _map: &MapState,
    field: &LightField,
) {
    let black = Color::Rgb(0, 0, 0);
    for row in 0..render.view_tiles_v {
        for col in 0..render.view_tiles_h {
            let map_x = render.start_x + col;
            let map_y = render.start_y + row;
            let brightness = field.brightness_at(map_x, map_y);
            let scale = brightness_to_scale(brightness);
            if scale >= 1.0 {
                continue;
            }

            if let Some((tile_x, tile_y)) = render.tile_cell_origin(map_x, map_y) {
                for dy in 0..render.rows_per_tile {
                    for dx in 0..render.cols_per_tile {
                        if let Some(cell) = buf.cell_mut((tile_x + dx, tile_y + dy)) {
                            if scale <= 0.0 {
                                cell.set_char(' ').set_fg(black).set_bg(black);
                            } else {
                                cell.set_fg(scale_and_floor_color(cell.fg, scale));
                                cell.set_bg(scale_and_floor_color(cell.bg, scale));
                            }
                        }
                    }
                }
            }
        }
    }
}

fn brightness_to_scale(brightness: f32) -> f32 {
    let b = brightness.clamp(0.0, 1.0);
    if b <= 0.0 {
        0.0
    } else {
        (MIN_VISIBLE + (1.0 - MIN_VISIBLE) * b.powf(GAMMA)).clamp(0.0, 1.0)
    }
}

fn falloff(distance: f32, range: f32, core_radius: f32) -> f32 {
    if range <= 0.0 {
        return 0.0;
    }
    if distance <= core_radius {
        return 1.0;
    }

    let span = (range - core_radius).max(f32::EPSILON);
    let t = ((distance - core_radius) / span).clamp(0.0, 1.0);
    (1.0 - t.powf(FALL_OFF_EXPONENT)).max(0.0)
}

fn scale_and_floor_color(color: Color, scale: f32) -> Color {
    match color {
        Color::Rgb(r, g, b) => {
            let s = scale.clamp(0.0, 1.0);
            Color::Rgb(
                (r as f32 * s).round() as u8,
                (g as f32 * s).round() as u8,
                (b as f32 * s).round() as u8,
            )
        }
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{MapState, Tile};
    use tui_map::core::{MapSize, TileKind};

    fn open_map(width: u16, height: u16) -> MapState {
        MapState::filled("test", MapSize::new(width, height), TileKind::Floor)
    }

    fn set_tile(map: &mut MapState, x: u16, y: u16, tile: Tile) {
        let idx = y as usize * map.width as usize + x as usize;
        map.tiles[idx] = tile;
    }

    fn view_index(width: u16, x: u16, y: u16) -> usize {
        y as usize * width as usize + x as usize
    }

    #[test]
    fn full_wall_blocks_direct_light_behind_it() {
        let mut map = open_map(10, 8);
        for y in 0..8u16 {
            set_tile(&mut map, 4, y, Tile::Wall);
        }

        let source = LightSource {
            x: 2,
            y: 4,
            intensity: 1.0,
            range: 6,
            core_radius: 1,
        };
        let field = compute_light_field(&map, 0, 0, 10, 8, &[source]);

        for y in 0..8u16 {
            for x in 5..10u16 {
                assert!(
                    field.brightness_at(x, y) < f32::EPSILON,
                    "tile ({x}, {y}) behind full wall should be dark, got {}",
                    field.brightness_at(x, y)
                );
            }
        }
    }

    #[test]
    fn open_corner_tile_gets_lit() {
        let map = open_map(4, 4);
        let source = LightSource {
            x: 1,
            y: 1,
            intensity: 1.0,
            range: 4,
            core_radius: 1,
        };

        let field = compute_light_field(&map, 0, 0, 4, 4, &[source]);
        assert!(field.brightness_at(0, 0) > 0.0);
    }

    #[test]
    fn reflection_only_from_hit_face() {
        let mut map = open_map(5, 3);
        set_tile(&mut map, 2, 1, Tile::Wall);

        let mut face_lights = vec![FaceLight::default(); 5 * 3];
        let wall_idx = view_index(5, 2, 1);
        face_lights[wall_idx].set_max(WallFace::West, 1.0);

        let mut floor_light = vec![0.0; 5 * 3];
        apply_directional_wall_bounce(&map, 0, 0, 5, 3, &face_lights, &mut floor_light);

        assert!(floor_light[view_index(5, 1, 1)] > 0.0);
        assert!(floor_light[view_index(5, 3, 1)] < f32::EPSILON);
    }

    #[test]
    fn reflection_reach_is_one_tile() {
        let mut map = open_map(5, 3);
        set_tile(&mut map, 2, 1, Tile::Wall);

        let mut face_lights = vec![FaceLight::default(); 5 * 3];
        let wall_idx = view_index(5, 2, 1);
        face_lights[wall_idx].set_max(WallFace::West, 1.0);

        let mut floor_light = vec![0.0; 5 * 3];
        apply_directional_wall_bounce(&map, 0, 0, 5, 3, &face_lights, &mut floor_light);

        assert!(floor_light[view_index(5, 1, 1)] > 0.0);
        assert!(floor_light[view_index(5, 0, 1)] < f32::EPSILON);
    }

    #[test]
    fn multiple_sources_add_and_clamp() {
        let map = open_map(7, 7);
        let source_a = LightSource {
            x: 3,
            y: 3,
            intensity: 0.7,
            range: 6,
            core_radius: 1,
        };
        let source_b = LightSource {
            x: 3,
            y: 3,
            intensity: 0.7,
            range: 6,
            core_radius: 1,
        };

        let one = compute_light_field(&map, 0, 0, 7, 7, &[source_a]);
        let two = compute_light_field(&map, 0, 0, 7, 7, &[source_a, source_b]);

        let center_one = one.brightness_at(3, 3);
        let center_two = two.brightness_at(3, 3);

        assert!(center_two > center_one);
        assert!(center_two <= 1.0 + f32::EPSILON);
        assert!(center_two > 0.99);
    }

    #[test]
    fn deterministic_for_same_inputs() {
        let mut map = open_map(10, 8);
        set_tile(&mut map, 5, 4, Tile::Wall);

        let sources = vec![
            LightSource {
                x: 2,
                y: 4,
                intensity: 1.0,
                range: 6,
                core_radius: 1,
            },
            LightSource {
                x: 7,
                y: 3,
                intensity: 0.35,
                range: 2,
                core_radius: 0,
            },
        ];

        let a = compute_light_field(&map, 0, 0, 10, 8, &sources);
        let b = compute_light_field(&map, 0, 0, 10, 8, &sources);
        assert_eq!(a.brightness, b.brightness);
    }

    #[test]
    fn water_does_not_block() {
        let mut map = open_map(10, 8);
        set_tile(&mut map, 5, 4, Tile::Water);

        let source = LightSource {
            x: 3,
            y: 4,
            intensity: 1.0,
            range: 6,
            core_radius: 1,
        };
        let field = compute_light_field(&map, 0, 0, 10, 8, &[source]);

        assert!(field.brightness_at(7, 4) > 0.15);
    }
}
