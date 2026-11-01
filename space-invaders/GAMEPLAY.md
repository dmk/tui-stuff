# Gameplay reference and pacing

This is a terminal adaptation, with measured gameplay constants rather than a
pixel-exact arcade emulator.

## Arcade reference

| Item | Original arcade behavior | Source |
| --- | --- | --- |
| Display | 224×256 portrait after cabinet rotation; approximately 60 Hz | [Hardware, Video](https://computerarcheology.com/Arcade/SpaceInvaders/Hardware.html#video) |
| Formation | 5×11 aliens, positioned on a 16-pixel grid | [ROM, GetAlienCoords](https://computerarcheology.com/Arcade/SpaceInvaders/Code.html#getaliencoords) |
| Marching | 2 pixels horizontally; 8-pixel descent when reversing at an edge | [ROM, RackBump](https://computerarcheology.com/Arcade/SpaceInvaders/Code.html#rackbump) |
| Cadence | Roughly one alien processed per refresh: a full-fleet cycle takes 55/60 seconds; fewer survivors cycle faster | [Game timing, The Aliens](https://computerarcheology.com/Arcade/SpaceInvaders/#the-aliens) |
| Enemy shots | Three shot objects; normal speed 4 pixels every third video frame, increasing to 5 with eight or fewer aliens | [Game objects](https://computerarcheology.com/Arcade/SpaceInvaders/#game-objects) |
| Later waves | First reference Y is hexadecimal 78; subsequent values are 60, 50, 48, 48, 48, 40, 40, 40, then repeat the later-wave table | [ROM, 07EA and AlienStartTable](https://computerarcheology.com/Arcade/SpaceInvaders/Code.html#alienstarttable) |

The ROM uses rotated coordinates. Those reference Y values must not be used
directly as top-down terminal row positions.

## Terminal mapping

| Setting | Value |
| --- | --- |
| Logical world | 80 columns × 76 half-row units |
| Small layout / minimum terminal | 80×40, including HUD and controls |
| Medium / large layouts | 112×59 / 176×78 |
| First-wave rows | Y = 7, 13, 19, 25, 31; alien height 4 |
| Bunker / ship positions | Y = 64 / 72 |
| Player movement | Held input, 50 horizontal units/s on the fixed clock; about 1.5 s across the field |
| First-wave room below lowest alien | 37 vertical units |
| Sideways travel with a full fleet | 15 units between opposing bounds |
| Whole-fleet cadence | `ceil(alive × 1000 / (60 × 20))` simulation ticks, minimum 1 |
| Opening cadence | 46 ticks = 920 ms |
| Edge descent | 4 units; never during ordinary sideways movement |
| Enemy projectile motion | 2 units every 3 simulation ticks = 33⅓ units/s |
| Enemy projectile limit | 3, independent of player projectiles |
| Later-wave downward offsets | 0 initially; then 12, 20, 24, 24, 24, 28, 28, 28, cycling afterward |

The visual sprite scale never changes those collision or timing constants.
Resizing only changes presentation or suspends the game below minimum size.
Arcade timing is quantized to our 50 Hz fixed clock. Cooldown-based player
rapid fire and seeded enemy selection remain terminal-game design choices.

## Pacing checks

The previous 72×44 arena allowed only seven units of sideways movement and
eleven units between the lowest initial alien and the ship. With a 600 ms fleet
step, an untouched fleet invaded in **27 seconds**. Opening enemy shots could
cross the gap in approximately **0.22 seconds**.

The revised opening fleet is tested to remain above the ship for at least
**two minutes**, and still invade before **three minutes** when left intact.
The straight opening shot reaches the ship in approximately **1.14 seconds**.
The invasion benchmark uses an invulnerable stationary player and no firing,
so it measures fleet descent rather than enemy-shot survival. Actual games
depend on player movement, kills, and the resulting fleet acceleration.

Regression tests cover that invasion window, opening-shot reaction time, the
920 ms first step, survivor acceleration, lower wave starts, and the shot cap.
