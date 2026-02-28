# Charset Reference

Complete reference for classcii's built-in character sets and custom charset creation.

## Built-in Charsets

Charsets are selected with keys `1`–`0` (where `0` = index 10). Characters are ordered from lightest (space) to densest (visually heaviest).

| Key | Index | Name | Characters | Length | Best For |
|-----|-------|------|------------|--------|----------|
| `1` | 0 | Full | `$@B%8&WM#*oahkbdpqwmZO0QLCJUYXzcvunxrjft/\|()1{}?-_+~<>i!lI;:,"^'. ` | 70 | Photos, portraits — maximum tonal range |
| `2` | 1 | Dense | `Ñ@#W$9876543210?!abc;:+=-,._ ` | 29 | Dense imagery, high detail |
| `3` | 2 | Short 1 | `.:-=+*#%@` | 9 | Quick rendering, low complexity |
| `4` | 3 | Short 2 | `@%#*+=-:. ` | 10 | Inverted gradient (dense-to-light) |
| `5` | 4 | Binary | ` #` | 2 | High contrast, 1-bit style |
| `6` | 5 | Extended | `=======--------:::::::::........` (repeating pattern) | 70 | Patterned rendering |
| `7` | 6 | Discrete | `1234 ` | 5 | Matrix/digital aesthetic |
| `8` | 7 | Edge | `.,*+#@` | 6 | Edge detection emphasis |
| `9` | 8 | Blocks | ` ░▒▓█` | 5 | Pseudo-pixel rendering, retro |
| `0` | 9 | Minimal | ` .:░▒▓█` | 7 | High contrast with Unicode blocks |

### Additional Charsets (config-only)

These charsets are available by setting the `charset` field in TOML config files but are not assigned to a number key by default:

| Name | Characters | Length | Best For |
|------|------------|--------|----------|
| Glitch 1 | ` .°*O0@#&%` | 10 | Organic contrast, glitch art |
| Glitch 2 | ` ▂▃▄▅▆▇█` | 8 | Spectrum bars, data visualization |
| Digital | ` 01` | 3 | Binary/cryptographic aesthetic |

## Charset Mechanics

### Luminance Mapping

Each charset defines a luminance ramp — characters ordered from visually lightest to visually densest. At startup, a 256-entry lookup table (`LuminanceLut`) maps each luminance value [0–255] to a character via linear projection:

```
char_index = round(luminance / 255 × (charset_length − 1))
```

This is O(1) per pixel with zero allocation.

### Charset Length vs Quality

- **2 characters** (Binary): 1-bit quantization — high contrast, no gradients
- **5–10 characters**: Coarse gradients — visible banding, stylistic
- **29–70 characters**: Fine gradients — smooth tonal transitions, photographic quality

More characters = smoother gradients but also more visual "noise" from character shape variation. For clean results with photos, Full (70) or Dense (29) are optimal. For stylistic rendering, shorter charsets create intentional posterization.

### Render Mode Compatibility

Charsets are only used in **Ascii** render mode. The other 5 render modes use fixed Unicode block characters:

| Render Mode | Characters Used | Charset Applies? |
|-------------|----------------|-----------------|
| Ascii | Charset characters | Yes |
| HalfBlock | `▄` `▀` with fg/bg colors | No |
| Braille | Unicode Braille patterns (U+2800–U+28FF) | No |
| Quadrant | 2×2 block elements | No |
| Sextant | 2×3 Unicode 13.0 blocks (U+1FB00) | No |
| Octant | 2×4 Unicode 16.0 blocks (U+1CD00) | No |

## Custom Charset Editor

Press `C` in TUI mode to open the custom charset editor.

### Workflow

1. Press `C` to open the editor
2. Type your custom characters from lightest to densest
3. Press `Enter` to apply
4. Press `Esc` to cancel

### Design Principles

- **Order matters**: Characters must go from visually lightest (leftmost) to densest (rightmost). Space is typically the lightest character.
- **Visual density**: The perceived "ink coverage" of a character determines its position. `@` and `#` are dense; `.` and `:` are light.
- **Minimum 2 characters**: The LUT requires at least 2 characters to function. A single-character charset will be replaced with ` @`.
- **Unicode support**: Any Unicode characters supported by your terminal font can be used, including block elements, CJK characters, emoji, etc.
- **No duplicates needed**: Each unique character provides one tonal level. Duplicates waste gradient resolution.

### Example Custom Charsets

```
 ·•●                     # Dot progression (4 levels)
 ░▒▓█                    # Block progression (5 levels)
 .oO0@                   # Circle progression (5 levels)
 -=≡■█                   # Weight progression (5 levels)
 ∙∘○◎●                   # Geometric circles (5 levels)
```
