use ratatui::style::Color;
use tui_dispatch_components::{BaseStyle, ModalStyle, Padding, TextInputStyle};

// Shared form and modal styles across the terminal apps.
pub fn input() -> TextInputStyle {
    TextInputStyle {
        base: BaseStyle {
            border: None,
            padding: Padding::all(1),
            bg: Some(Color::Rgb(50, 50, 60)),
            fg: None,
        },
        placeholder_style: None,
        cursor_style: None,
    }
}

pub fn modal() -> ModalStyle {
    ModalStyle {
        base: BaseStyle {
            bg: Some(Color::Rgb(35, 35, 45)),
            padding: Padding::default(),
            border: None,
            fg: None,
        },
        ..Default::default()
    }
}
