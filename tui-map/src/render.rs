use std::collections::HashMap;
use std::sync::Arc;

use ratatui::{layout::Rect, style::Color, Frame};

use crate::core::{viewport_centered, MapRead, TileKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextureVariant {
    pub ch: char,
    pub fg: Color,
    pub density: u8,
}

impl TextureVariant {
    pub const fn new(ch: char, fg: Color, density: u8) -> Self {
        Self { ch, fg, density }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TilePalette {
    pub main: Color,
    pub alt: Color,
    pub variants: [TextureVariant; 3],
}

impl TilePalette {
    pub const fn new(main: Color, alt: Color, variants: [TextureVariant; 3]) -> Self {
        Self {
            main,
            alt,
            variants,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TilePaint {
    pub background_main: Color,
    pub background_alt: Color,
    pub texture: TextureVariant,
}

type VariantSelector =
    Arc<dyn Fn(TileKind, u16, u16, u32, &TilePalette) -> TextureVariant + Send + Sync>;

#[derive(Clone)]
pub struct TileTheme {
    palettes: HashMap<TileKind, TilePalette>,
    fallback: TilePalette,
    variant_selector: VariantSelector,
}

impl TileTheme {
    pub fn builder() -> TileThemeBuilder {
        TileThemeBuilder::default()
    }

    pub fn paint(&self, tile: TileKind, map_x: u16, map_y: u16) -> TilePaint {
        let palette = self.palettes.get(&tile).copied().unwrap_or(self.fallback);
        let seed = tile_seed(map_x, map_y);
        let texture = (self.variant_selector)(tile, map_x, map_y, seed, &palette);
        TilePaint {
            background_main: palette.main,
            background_alt: palette.alt,
            texture,
        }
    }
}

#[derive(Clone)]
pub struct TileThemeBuilder {
    palettes: HashMap<TileKind, TilePalette>,
    fallback: TilePalette,
    variant_selector: Option<VariantSelector>,
}

impl Default for TileThemeBuilder {
    fn default() -> Self {
        let fallback = TilePalette::new(
            Color::Rgb(34, 112, 58),
            Color::Rgb(38, 120, 64),
            [
                TextureVariant::new('.', adjust_color(Color::Rgb(34, 112, 58), 10), 6),
                TextureVariant::new('\'', adjust_color(Color::Rgb(34, 112, 58), 6), 7),
                TextureVariant::new('`', adjust_color(Color::Rgb(34, 112, 58), -4), 8),
            ],
        );

        Self {
            palettes: HashMap::new(),
            fallback,
            variant_selector: None,
        }
    }
}

impl TileThemeBuilder {
    pub fn tile(mut self, kind: TileKind, palette: TilePalette) -> Self {
        self.palettes.insert(kind, palette);
        self
    }

    pub fn fallback(mut self, palette: TilePalette) -> Self {
        self.fallback = palette;
        self
    }

    pub fn variant_selector<F>(mut self, selector: F) -> Self
    where
        F: Fn(TileKind, u16, u16, u32, &TilePalette) -> TextureVariant + Send + Sync + 'static,
    {
        self.variant_selector = Some(Arc::new(selector));
        self
    }

    pub fn build(self) -> TileTheme {
        let selector: VariantSelector = self.variant_selector.unwrap_or_else(|| {
            Arc::new(|_tile, _x, _y, seed, palette| {
                let idx = (seed % palette.variants.len() as u32) as usize;
                palette.variants[idx]
            })
        });

        TileTheme {
            palettes: self.palettes,
            fallback: self.fallback,
            variant_selector: selector,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct RenderConfig {
    pub map_tiles_vertical_hint: u16,
    pub cell_aspect: f32,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            map_tiles_vertical_hint: 9,
            cell_aspect: 2.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Camera {
    pub focus_x: u16,
    pub focus_y: u16,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MapRenderResult {
    pub start_x: u16,
    pub start_y: u16,
    pub view_tiles_h: u16,
    pub view_tiles_v: u16,
    pub origin_x: u16,
    pub origin_y: u16,
    pub cols_per_tile: u16,
    pub rows_per_tile: u16,
}

impl MapRenderResult {
    pub fn marker_cell(&self, map_x: u16, map_y: u16) -> Option<(u16, u16)> {
        let (cell_x, cell_y) = self.tile_cell_origin(map_x, map_y)?;
        Some((
            cell_x + self.cols_per_tile / 2,
            cell_y + self.rows_per_tile / 2,
        ))
    }

    pub fn tile_cell_origin(&self, map_x: u16, map_y: u16) -> Option<(u16, u16)> {
        if self.view_tiles_h == 0 || self.view_tiles_v == 0 {
            return None;
        }
        if map_x < self.start_x
            || map_y < self.start_y
            || map_x >= self.start_x + self.view_tiles_h
            || map_y >= self.start_y + self.view_tiles_v
        {
            return None;
        }

        let tile_col = map_x - self.start_x;
        let tile_row = map_y - self.start_y;
        Some((
            self.origin_x + tile_col * self.cols_per_tile,
            self.origin_y + tile_row * self.rows_per_tile,
        ))
    }
}

#[derive(Clone)]
pub struct MapRendererBuilder {
    config: RenderConfig,
    theme: TileTheme,
}

impl Default for MapRendererBuilder {
    fn default() -> Self {
        Self {
            config: RenderConfig::default(),
            theme: TileTheme::builder().build(),
        }
    }
}

impl MapRendererBuilder {
    pub fn config(mut self, config: RenderConfig) -> Self {
        self.config = config;
        self
    }

    pub fn map_tiles_vertical_hint(mut self, value: u16) -> Self {
        self.config.map_tiles_vertical_hint = value;
        self
    }

    pub fn cell_aspect(mut self, value: f32) -> Self {
        self.config.cell_aspect = value;
        self
    }

    pub fn theme(mut self, theme: TileTheme) -> Self {
        self.theme = theme;
        self
    }

    pub fn build(self) -> MapRenderer {
        MapRenderer {
            config: self.config,
            theme: self.theme,
        }
    }
}

#[derive(Clone)]
pub struct MapRenderer {
    config: RenderConfig,
    theme: TileTheme,
}

impl MapRenderer {
    pub fn builder() -> MapRendererBuilder {
        MapRendererBuilder::default()
    }

    pub fn render_base<M: MapRead>(
        &self,
        frame: &mut Frame,
        area: Rect,
        map: &M,
        camera: Camera,
        _focused: bool,
    ) -> MapRenderResult {
        let mut result = MapRenderResult::default();
        let map_size = map.map_size();

        if area.width == 0 || area.height == 0 || map_size.width == 0 || map_size.height == 0 {
            return result;
        }

        let hint = self.config.map_tiles_vertical_hint.max(1);
        let rows_per_tile = (area.height / hint).max(2);
        let cols_per_tile = ((rows_per_tile as f32 * self.config.cell_aspect).round() as u16).max(2);

        let view_tiles_h = (area.width / cols_per_tile).min(map_size.width);
        let view_tiles_v = (area.height / rows_per_tile).min(map_size.height);

        result.cols_per_tile = cols_per_tile;
        result.rows_per_tile = rows_per_tile;
        result.view_tiles_h = view_tiles_h;
        result.view_tiles_v = view_tiles_v;

        if view_tiles_h == 0 || view_tiles_v == 0 {
            return result;
        }

        let used_cols = view_tiles_h * cols_per_tile;
        let used_rows = view_tiles_v * rows_per_tile;
        let pad_x = (area.width.saturating_sub(used_cols)) / 2;
        let pad_y = (area.height.saturating_sub(used_rows)) / 2;
        let origin_x = area.x + pad_x;
        let origin_y = area.y + pad_y;

        let (start_x, start_y) = viewport_centered(
            camera.focus_x,
            camera.focus_y,
            map_size,
            view_tiles_h,
            view_tiles_v,
        );

        result.start_x = start_x;
        result.start_y = start_y;
        result.origin_x = origin_x;
        result.origin_y = origin_y;

        let buf = frame.buffer_mut();
        for tile_row in 0..view_tiles_v {
            for tile_col in 0..view_tiles_h {
                let map_x = start_x + tile_col;
                let map_y = start_y + tile_row;
                let tile = map.tile_kind(map_x, map_y);

                let paint = self.theme.paint(tile, map_x, map_y);
                let bg_seed = tile_seed(map_x, map_y);
                let bg = if bg_seed % 2 == 0 {
                    paint.background_main
                } else {
                    paint.background_alt
                };

                let cell_x = origin_x + tile_col * cols_per_tile;
                let cell_y = origin_y + tile_row * rows_per_tile;
                let density = paint.texture.density.max(1) as u32;

                for dy in 0..rows_per_tile {
                    for dx in 0..cols_per_tile {
                        let x = cell_x + dx;
                        let y = cell_y + dy;
                        if let Some(cell) = buf.cell_mut((x, y)) {
                            let sprinkle = cell_seed(map_x, map_y, dx, dy);
                            if sprinkle % density == 0 {
                                cell.set_bg(bg)
                                    .set_fg(paint.texture.fg)
                                    .set_char(paint.texture.ch);
                            } else {
                                cell.set_bg(bg).set_fg(bg).set_char(' ');
                            }
                        }
                    }
                }
            }
        }

        result
    }
}

pub fn adjust_color(color: Color, delta: i16) -> Color {
    match color {
        Color::Rgb(r, g, b) => {
            let clamp = |v: i16| v.max(0).min(255) as u8;
            Color::Rgb(
                clamp(r as i16 + delta),
                clamp(g as i16 + delta),
                clamp(b as i16 + delta),
            )
        }
        other => other,
    }
}

pub fn tile_seed(x: u16, y: u16) -> u32 {
    let mut n = x as u32;
    n = n
        .wrapping_mul(374_761_393)
        .wrapping_add((y as u32).wrapping_mul(668_265_263));
    n ^= n >> 13;
    n = n.wrapping_mul(1_274_126_177);
    n ^= n >> 16;
    n
}

pub fn cell_seed(x: u16, y: u16, dx: u16, dy: u16) -> u32 {
    let mut n = tile_seed(x, y);
    n ^= (dx as u32).wrapping_mul(2_246_822_519);
    n ^= (dy as u32).wrapping_mul(3_266_489_917);
    n ^= n >> 15;
    n = n.wrapping_mul(668_265_263);
    n ^= n >> 13;
    n
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{MapGrid, MapSize};
    use ratatui::{backend::TestBackend, Terminal};

    #[test]
    fn tile_seed_is_deterministic() {
        assert_eq!(tile_seed(7, 9), tile_seed(7, 9));
        assert_ne!(tile_seed(7, 9), tile_seed(7, 10));
    }

    #[test]
    fn marker_projection_is_stable() {
        let result = MapRenderResult {
            start_x: 10,
            start_y: 20,
            view_tiles_h: 8,
            view_tiles_v: 6,
            origin_x: 3,
            origin_y: 4,
            cols_per_tile: 2,
            rows_per_tile: 3,
        };

        assert_eq!(result.tile_cell_origin(10, 20), Some((3, 4)));
        assert_eq!(result.marker_cell(11, 21), Some((6, 8)));
        assert_eq!(result.marker_cell(100, 100), None);
    }

    #[test]
    fn render_output_is_deterministic_for_same_inputs() {
        let map = MapGrid::filled("demo", MapSize::new(12, 12), TileKind::Grass);
        let renderer = MapRenderer::builder().build();

        let mut terminal = Terminal::new(TestBackend::new(40, 20)).expect("terminal");
        let mut first = MapRenderResult::default();
        terminal
            .draw(|frame| {
                first = renderer.render_base(
                    frame,
                    Rect::new(0, 0, 40, 20),
                    &map,
                    Camera {
                        focus_x: 6,
                        focus_y: 6,
                    },
                    true,
                );
            })
            .expect("draw 1");

        let (sample_x, sample_y) = first.marker_cell(6, 6).expect("sample in view");
        let first_symbol = terminal
            .backend()
            .buffer()
            .cell((sample_x, sample_y))
            .expect("cell")
            .symbol()
            .to_string();

        terminal
            .draw(|frame| {
                let second = renderer.render_base(
                    frame,
                    Rect::new(0, 0, 40, 20),
                    &map,
                    Camera {
                        focus_x: 6,
                        focus_y: 6,
                    },
                    true,
                );
                assert_eq!(first, second);
            })
            .expect("draw 2");

        let second_symbol = terminal
            .backend()
            .buffer()
            .cell((sample_x, sample_y))
            .expect("cell")
            .symbol()
            .to_string();

        assert_eq!(first_symbol, second_symbol);
    }
}
