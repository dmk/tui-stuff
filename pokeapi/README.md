# PokeAPI TUI

A terminal user interface for browsing Pokemon data from the PokeAPI, built with Rust using the tui-dispatch framework.

## Features

- Multi-region Pokedex list of base forms with search and type filters
- Detail panel with stats, moves, abilities, encounters, type matchup, and evolution paths
- Move/ability detail pane with power, accuracy, PP, and effect text
- Ghostty Kitty graphics protocol sprites (animated when available)
- Built-in cry playback from PokeAPI audio
- Favorites and team roster

## Controls

- `j`/`k` or arrow keys: Move selection
- `PageUp`/`PageDown`: Page scroll
- `Tab`/`Shift+Tab`: Cycle focus between widgets
- `/`: Search (type to filter, Enter to apply, Esc to clear)
- `[`/`]`: Previous/next type filter (Encounter tab cycles version)
- `r`/`R`: Next/previous region (header focus)
- `j`/`k`: Navigate list, tabs content, or evolution stages (focused widget)
- `Tab`/`Shift+Tab`: Focus header, list, tabs, evolution
- `h`/`l`: Switch detail tabs (General/Moves/Abilities)
- `c`: Clear type filter
- `f`: Toggle favorite
- `t`: Add/remove team member
- `p`: Play Pokemon cry
- `q`: Quit
