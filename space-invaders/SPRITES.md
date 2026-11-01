# Space Invaders — sprite sketches

Tiny Unicode silhouettes, colored with ANSI foreground colors when rendered.
Spaces are transparent. Keep color separate from sprite data so the eventual
Ratatui renderer can use styles instead of embedded escape sequences.

## Invaders

Each frame occupies **5 columns × 2 rows**, including surrounding spaces.
Switch frames on each formation step, rather than on every render.

### Squid · magenta · top rank

```text
 ▄█▄
▟▀ ▀▙
```

```text
 ▄█▄
▘▀ ▀▝
```

### Crab · cyan · middle rank

```text
▚▄ ▄▞
 ▜█▛
```

```text
▗▄ ▄▖
▘▜█▛▝
```

### Octopus · green · bottom rank

```text
▗███▖
▛▙ ▟▜
```

```text
▗███▖
▙▘ ▝▟
```

## Player · bright white · 5 × 2

```text
  ▄
▄███▄
```

The barrel marks the firing column. Brief cyan/white flashes can communicate
respawn invulnerability without changing the silhouette.

## Mystery saucer · red · 7 × 2

```text
 ▄███▄
▀▟▀▟▀▟▀
```

Alternate the underside while flying:

```text
 ▄███▄
▀▙▀▙▀▙▀
```

## Shield · green · 7 × 3

```text
 ▄███▄
███████
██   ██
```

Use destructible half-cell pixels: `█ → ▀ / ▄ → space`. Shots chip the actual
shield mask; the upper and lower halves can be removed independently.

## Shots · 1 × 1

| Object | Frames | Color |
| --- | --- | --- |
| Player bolt | `╵` | bright yellow |
| Alien bolt | `╽` / `╿` | red |
| Alien zigzag | `╲` / `╱` | yellow |
| Hit spark | `×` | white |

## Explosion · yellow to red · 5 × 2

```text
 ╲│╱
 ╱│╲
```

```text
·   ·
  ×
```

Then disappear. Keep the same logical bounds throughout the animation.

## Formation sketch

```text
 ▄█▄     ▄█▄     ▄█▄     ▄█▄     ▄█▄
▟▀ ▀▙   ▟▀ ▀▙   ▟▀ ▀▙   ▟▀ ▀▙   ▟▀ ▀▙

▚▄ ▄▞   ▚▄ ▄▞   ▚▄ ▄▞   ▚▄ ▄▞   ▚▄ ▄▞
 ▜█▛     ▜█▛     ▜█▛     ▜█▛     ▜█▛

▗███▖   ▗███▖   ▗███▖   ▗███▖   ▗███▖
▛▙ ▟▜   ▛▙ ▟▜   ▛▙ ▟▜   ▛▙ ▟▜   ▛▙ ▟▜

                  ╵

 ▄███▄           ▄███▄           ▄███▄
███████         ███████         ███████
██   ██         ██   ██         ██   ██

                   ▄
                 ▄███▄
```

## Rendering notes

- Pad each row to its declared width when rendering; trailing spaces are
  omitted in these sketches.
- Start with three empty columns between aliens and one empty row between ranks.
- Use a black background and normal-weight glyphs; terminal bold can change
  the appearance of block joins.
- Check the chosen terminal font before locking these in. Unicode block and
  line glyphs can vary across fonts.
- For a strict ASCII mode, use the silhouettes below with the same 5 × 2 bounds.

```text
Squid A   Squid B   Crab A    Crab B    Octopus A Octopus B
 /M\       /M\      \o o/     _o o_      .www.     .www.
/   \     '   '      /W\      / W \      /| |\     \' '/

Player    Saucer (7 × 2)    Shield (7 × 3)
  ^         .---.           .###.
 /#\       <_o_o_>         #######
                           ##   ##
```
