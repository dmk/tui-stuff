use std::sync::OnceLock;

use ratatui::{
    Frame,
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};
use tui_map::core::TileKind;
use tui_map::render::{
    Camera, MapRenderResult, MapRenderer, RenderConfig, TextureVariant, TilePalette, TileTheme,
    adjust_color,
};

use crate::lighting::{LightSource, apply_light_field_to_buffer, compute_light_field};
use crate::state::{AppState, DangerMode, GameMode, RuntimeAnchorKind};

const BG: Color = Color::Rgb(16, 18, 24);
const FG: Color = Color::Rgb(230, 228, 218);
const MUTED: Color = Color::Rgb(146, 148, 154);
const ACCENT: Color = Color::Rgb(233, 199, 104);
const DANGER_HUNTER: Color = Color::Rgb(210, 88, 78);
const DANGER_COLLAPSE: Color = Color::Rgb(222, 158, 78);
const TRAIL_GLOW: Color = Color::Rgb(228, 186, 88);
const TRAIL_EDGE: Color = Color::Rgb(186, 138, 62);
const PLAYER_CORE: Color = Color::Rgb(255, 252, 244);
const PLAYER_EDGE: Color = Color::Rgb(218, 228, 248);
const PLAYER_DIM: Color = Color::Rgb(128, 148, 184);

// Player light range tuning:
// range = (BASE + light_current / DIVISOR).clamp(MIN, MAX)
const PLAYER_LIGHT_RANGE_BASE: u16 = 1;
const PLAYER_LIGHT_RANGE_DIVISOR: u16 = 30;
const PLAYER_LIGHT_RANGE_MIN: u16 = 1;
const PLAYER_LIGHT_RANGE_MAX: u16 = 6;

const CELL_ASPECT: f32 = 2.0;
const MAP_TILES_V: u16 = 10;

static MAP_RENDERER: OnceLock<MapRenderer> = OnceLock::new();

fn map_renderer() -> &'static MapRenderer {
    MAP_RENDERER.get_or_init(|| {
        MapRenderer::builder()
            .config(RenderConfig {
                map_tiles_vertical_hint: MAP_TILES_V,
                cell_aspect: CELL_ASPECT,
            })
            .theme(lightline_map_theme())
            .build()
    })
}

fn lightline_map_theme() -> TileTheme {
    // Walkable tiles are intentionally brighter than blocked tiles.
    let floor_base = Color::Rgb(102, 108, 128);
    let trail_base = Color::Rgb(170, 128, 74);
    let grass_base = Color::Rgb(82, 148, 92);
    let wall_base = Color::Rgb(18, 20, 24);
    let water_base = Color::Rgb(14, 58, 142);

    let floor = TilePalette::new(
        floor_base,
        adjust_color(floor_base, 8),
        [
            TextureVariant::new('.', adjust_color(floor_base, 36), 14),
            TextureVariant::new('`', adjust_color(floor_base, 24), 16),
            TextureVariant::new(' ', adjust_color(floor_base, 12), 3),
        ],
    );
    let trail = TilePalette::new(
        trail_base,
        adjust_color(trail_base, 8),
        [
            TextureVariant::new(':', adjust_color(trail_base, 56), 8),
            TextureVariant::new('.', adjust_color(trail_base, 38), 9),
            TextureVariant::new('=', adjust_color(trail_base, 22), 10),
        ],
    );
    let grass = TilePalette::new(
        grass_base,
        adjust_color(grass_base, 8),
        [
            TextureVariant::new('"', adjust_color(grass_base, 56), 8),
            TextureVariant::new('.', adjust_color(grass_base, 38), 9),
            TextureVariant::new('`', adjust_color(grass_base, 20), 10),
        ],
    );
    let wall = TilePalette::new(
        wall_base,
        adjust_color(wall_base, 4),
        [
            TextureVariant::new('#', adjust_color(wall_base, 130), 5),
            TextureVariant::new('#', adjust_color(wall_base, 100), 6),
            TextureVariant::new('.', adjust_color(wall_base, 70), 8),
        ],
    );
    let water = TilePalette::new(
        water_base,
        adjust_color(water_base, 4),
        [
            TextureVariant::new('~', adjust_color(water_base, 70), 6),
            TextureVariant::new('-', adjust_color(water_base, 52), 7),
            TextureVariant::new('=', adjust_color(water_base, 36), 8),
        ],
    );

    TileTheme::builder()
        .fallback(floor)
        .tile(TileKind::Floor, floor)
        .tile(TileKind::Trail, trail)
        .tile(TileKind::Grass, grass)
        .tile(TileKind::Wall, wall)
        .tile(TileKind::Water, water)
        .build()
}

pub fn render(frame: &mut Frame, area: Rect, state: &AppState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(8), Constraint::Length(4)])
        .split(area);

    let title = format!(
        "Lightline  Floor {}  [{:?}]",
        state.floor_index + 1,
        state.danger_mode
    );
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .style(Style::default().bg(BG).fg(FG));
    let map_inner = block.inner(chunks[0]);
    frame.render_widget(block, chunks[0]);
    render_map(frame, map_inner, state);

    let danger_color = match state.danger_mode {
        DangerMode::SoundHunter => DANGER_HUNTER,
        DangerMode::ImminentCollapse => DANGER_COLLAPSE,
    };
    let status = state
        .last_status
        .clone()
        .unwrap_or_else(|| "Find the exit (>) and descend.".to_string());
    let lines = vec![
        Line::from(vec![
            Span::styled(
                format!(
                    "Light {}/{}  ",
                    state.player.light_current, state.player.light_max
                ),
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("Danger {:?}  ", state.danger_mode),
                Style::default().fg(danger_color),
            ),
            Span::styled(
                format!("Pos ({}, {})", state.player.x, state.player.y),
                Style::default().fg(MUTED),
            ),
        ]),
        Line::from(Span::styled(status, Style::default().fg(FG))),
        Line::from(Span::styled(
            controls_line(state.mode),
            Style::default().fg(MUTED),
        )),
    ];
    let footer = Paragraph::new(lines).alignment(Alignment::Left);

    frame.render_widget(footer, chunks[1]);
}

fn controls_line(mode: GameMode) -> &'static str {
    match mode {
        GameMode::Exploration => {
            "Move: WASD/arrows  Reclaim trail: Shift+move  Interact: E  Pause: Esc  Quit: Q"
        }
        GameMode::Pause => "Paused: Esc to resume  Quit: Q",
        GameMode::GameOver => "Game Over: R restart  Quit: Q",
        GameMode::Boot => "Generating floor...",
    }
}

fn render_map(frame: &mut Frame, area: Rect, state: &AppState) {
    if area.width < 8 || area.height < 4 {
        let warning = Paragraph::new("Resize for map view.")
            .style(Style::default().fg(MUTED))
            .alignment(Alignment::Center);
        frame.render_widget(warning, area);
        return;
    }

    let render = map_renderer().render_base(
        frame,
        area,
        &state.map,
        Camera {
            focus_x: state.player.x,
            focus_y: state.player.y,
        },
        true,
    );

    if render.view_tiles_h == 0 || render.view_tiles_v == 0 {
        return;
    }

    let sources = build_light_sources(render, state);
    let light_field = compute_light_field(
        &state.map,
        render.start_x,
        render.start_y,
        render.view_tiles_h,
        render.view_tiles_v,
        &sources,
    );

    let buf = frame.buffer_mut();
    apply_light_field_to_buffer(buf, render, &state.map, &light_field);
    render_trail_overlay(buf, render, state);

    for anchor in &state.anchors {
        if anchor.x == state.player.x && anchor.y == state.player.y {
            continue;
        }
        if let Some((ch, fg)) = anchor_marker(anchor.kind) {
            draw_marker(buf, anchor.x, anchor.y, render, ch, fg);
        }
    }

    draw_player_bulb(buf, render, state);
}

fn build_light_sources(render: MapRenderResult, state: &AppState) -> Vec<LightSource> {
    let mut sources = Vec::new();
    sources.push(LightSource {
        x: state.player.x,
        y: state.player.y,
        intensity: 1.0,
        range: player_light_range(state.player.light_current),
        core_radius: 1,
    });

    for row in 0..render.view_tiles_v {
        for col in 0..render.view_tiles_h {
            let map_x = render.start_x + col;
            let map_y = render.start_y + row;
            let charge = state.trail.charge_at(map_x, map_y);
            if charge == 0 {
                continue;
            }

            let intensity = (0.22 + 0.06 * charge as f32).min(0.42);
            let range = 1 + (charge / 3).min(1);
            sources.push(LightSource {
                x: map_x,
                y: map_y,
                intensity,
                range,
                core_radius: 0,
            });
        }
    }

    sources
}

fn player_light_range(light_current: u16) -> u16 {
    (PLAYER_LIGHT_RANGE_BASE + light_current / PLAYER_LIGHT_RANGE_DIVISOR)
        .clamp(PLAYER_LIGHT_RANGE_MIN, PLAYER_LIGHT_RANGE_MAX)
}

fn render_trail_overlay(buf: &mut Buffer, render: MapRenderResult, state: &AppState) {
    for row in 0..render.view_tiles_v {
        for col in 0..render.view_tiles_h {
            let map_x = render.start_x + col;
            let map_y = render.start_y + row;
            let charge = state.trail.charge_at(map_x, map_y);
            if charge == 0 {
                continue;
            }

            let strength = charge.min(6) as f32 / 6.0;
            // Keep trail blobs visible even on small tile cell sizes.
            let radius = 0.56 + 0.18 * strength;
            let core_radius = radius * 0.52;
            let glow = charge.min(6) as u8 * 5;
            let core_color = lighten_color(TRAIL_GLOW, glow);
            let edge_color = lighten_color(TRAIL_EDGE, glow / 2);
            draw_blob_in_tile(
                buf,
                map_x,
                map_y,
                render,
                radius,
                core_radius,
                '▒',
                '░',
                core_color,
                edge_color,
            );
            draw_marker(
                buf,
                map_x,
                map_y,
                render,
                '•',
                lighten_color(core_color, 16),
            );
        }
    }
}

fn draw_player_bulb(buf: &mut Buffer, render: MapRenderResult, state: &AppState) {
    let ratio = if state.player.light_max == 0 {
        0.0
    } else {
        (state.player.light_current as f32 / state.player.light_max as f32).clamp(0.0, 1.0)
    };
    let Some(radius) = player_bulb_radius(ratio) else {
        return;
    };

    let core_radius = (radius * 0.64).max(0.16);
    let core_color = scale_color(PLAYER_CORE, 0.35 + 0.65 * ratio);
    let edge_color = scale_color(PLAYER_EDGE, 0.25 + 0.65 * ratio);
    let dim_color = scale_color(PLAYER_DIM, 0.20 + 0.60 * ratio);

    draw_blob_in_tile(
        buf,
        state.player.x,
        state.player.y,
        render,
        radius,
        core_radius,
        '█',
        '▓',
        core_color,
        edge_color,
    );

    // Small center filament to keep player orientation/visibility crisp at low light.
    draw_marker(
        buf,
        state.player.x,
        state.player.y,
        render,
        '●',
        if ratio > 0.12 { core_color } else { dim_color },
    );
}

fn player_bulb_radius(light_ratio: f32) -> Option<f32> {
    if light_ratio <= 0.0 {
        return None;
    }
    let radius = if light_ratio >= 0.85 {
        1.00
    } else if light_ratio >= 0.65 {
        0.90
    } else if light_ratio >= 0.45 {
        0.78
    } else if light_ratio >= 0.25 {
        0.64
    } else if light_ratio >= 0.10 {
        0.52
    } else {
        0.38
    };
    Some(radius)
}

#[allow(clippy::too_many_arguments)]
fn draw_blob_in_tile(
    buf: &mut Buffer,
    map_x: u16,
    map_y: u16,
    render: MapRenderResult,
    radius: f32,
    core_radius: f32,
    core_ch: char,
    edge_ch: char,
    core_color: Color,
    edge_color: Color,
) {
    let Some((tile_x, tile_y)) = render.tile_cell_origin(map_x, map_y) else {
        return;
    };

    let cols = render.cols_per_tile.max(1);
    let rows = render.rows_per_tile.max(1);
    let cx = tile_x as f32 + cols as f32 / 2.0;
    let cy = tile_y as f32 + rows as f32 / 2.0;
    let max_rx = (cols as f32 / 2.0).max(1.0);
    let max_ry = (rows as f32 / 2.0).max(1.0);
    let radius = radius.clamp(0.0, 1.0);
    let core_radius = core_radius.clamp(0.0, radius);
    let mut drew_any = false;

    for dy in 0..rows {
        for dx in 0..cols {
            let px = tile_x + dx;
            let py = tile_y + dy;
            let norm_x = (((px as f32 + 0.5) - cx) / max_rx).powi(2);
            let norm_y = (((py as f32 + 0.5) - cy) / max_ry).powi(2);
            let norm_dist = (norm_x + norm_y).sqrt();
            if norm_dist > radius {
                continue;
            }

            if let Some(cell) = buf.cell_mut((px, py)) {
                if norm_dist <= core_radius {
                    cell.set_char(core_ch)
                        .set_fg(core_color)
                        .set_bg(dim_rgb(core_color, 0.35));
                } else {
                    cell.set_char(edge_ch)
                        .set_fg(edge_color)
                        .set_bg(dim_rgb(edge_color, 0.45));
                }
                drew_any = true;
            }
        }
    }

    // Guarantee a visible center blob even on tiny tiles / strict radius.
    if !drew_any {
        let center_x = tile_x + cols / 2;
        let center_y = tile_y + rows / 2;
        if let Some(cell) = buf.cell_mut((center_x, center_y)) {
            cell.set_char(core_ch)
                .set_fg(core_color)
                .set_bg(dim_rgb(core_color, 0.35));
        }
    }
}

fn anchor_marker(kind: RuntimeAnchorKind) -> Option<(char, Color)> {
    Some(match kind {
        RuntimeAnchorKind::PlayerStart => ('S', Color::Rgb(150, 180, 220)),
        RuntimeAnchorKind::Exit => ('>', Color::Rgb(220, 220, 120)),
        RuntimeAnchorKind::Beacon => ('B', Color::Rgb(240, 188, 126)),
        RuntimeAnchorKind::Relic => ('*', Color::Rgb(198, 142, 224)),
        RuntimeAnchorKind::Switch => ('=', Color::Rgb(138, 188, 154)),
    })
}

fn draw_marker(
    buf: &mut Buffer,
    map_x: u16,
    map_y: u16,
    render: MapRenderResult,
    ch: char,
    fg: Color,
) {
    if let Some((center_x, center_y)) = render.marker_cell(map_x, map_y) {
        if let Some(cell) = buf.cell_mut((center_x, center_y)) {
            cell.set_fg(fg).set_char(ch);
        }
    }
}

fn lighten_color(color: Color, amount: u8) -> Color {
    match color {
        Color::Rgb(r, g, b) => Color::Rgb(
            r.saturating_add(amount),
            g.saturating_add(amount),
            b.saturating_add(amount),
        ),
        other => other,
    }
}

fn scale_color(color: Color, scale: f32) -> Color {
    match color {
        Color::Rgb(r, g, b) => {
            let scale = scale.clamp(0.0, 1.0);
            Color::Rgb(
                ((r as f32) * scale).round().clamp(0.0, 255.0) as u8,
                ((g as f32) * scale).round().clamp(0.0, 255.0) as u8,
                ((b as f32) * scale).round().clamp(0.0, 255.0) as u8,
            )
        }
        other => other,
    }
}

fn dim_rgb(color: Color, factor: f32) -> Color {
    match color {
        Color::Rgb(r, g, b) => {
            let f = factor.clamp(0.0, 1.0);
            Color::Rgb(
                ((r as f32) * f).round().clamp(0.0, 255.0) as u8,
                ((g as f32) * f).round().clamp(0.0, 255.0) as u8,
                ((b as f32) * f).round().clamp(0.0, 255.0) as u8,
            )
        }
        other => other,
    }
}
