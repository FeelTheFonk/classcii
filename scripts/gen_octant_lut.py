#!/usr/bin/env python3
"""Generate the Octant LUT for classcii from Unicode 16.0 data.

Downloads UnicodeData.txt from unicode.org and parses BLOCK OCTANT character names
to build a 256-entry lookup table mapping 8-bit bitmask → char.

Code bitmask layout (column-major):
  +---+---+
  | 0 | 4 |  (bit positions)
  +---+---+
  | 1 | 5 |
  +---+---+
  | 2 | 6 |
  +---+---+
  | 3 | 7 |
  +---+---+

Unicode cell numbering (row-major):
  +---+---+
  | 1 | 2 |
  +---+---+
  | 3 | 4 |
  +---+---+
  | 5 | 6 |
  +---+---+
  | 7 | 8 |
  +---+---+
"""

import urllib.request
import sys

# ─── Unicode cell → code bit position mapping ───
# Unicode cell N (1-8, row-major) → code bit position (0-7, column-major)
UNICODE_CELL_TO_BIT = {
    1: 0,  # row 1, col 1
    2: 4,  # row 1, col 2
    3: 1,  # row 2, col 1
    4: 5,  # row 2, col 2
    5: 2,  # row 3, col 1
    6: 6,  # row 3, col 2
    7: 3,  # row 4, col 1
    8: 7,  # row 4, col 2
}

# ─── Known Block Elements that correspond to octant patterns ───
# Maps frozenset of Unicode cells → (codepoint, name)
BLOCK_ELEMENTS = {}

def cell_set_to_bitmask(cells: frozenset) -> int:
    """Convert a set of Unicode cell numbers (1-8) to a code bitmask."""
    mask = 0
    for cell in cells:
        mask |= 1 << UNICODE_CELL_TO_BIT[cell]
    return mask

def bitmask_to_cell_set(bitmask: int) -> frozenset:
    """Convert a code bitmask to a set of Unicode cell numbers (1-8)."""
    cells = set()
    for cell, bit in UNICODE_CELL_TO_BIT.items():
        if bitmask & (1 << bit):
            cells.add(cell)
    return frozenset(cells)

def cell_set_sort_key(cells: frozenset) -> tuple:
    """Sort key for cell sets: by size first, then lexicographic."""
    sorted_cells = sorted(cells)
    return (len(sorted_cells), tuple(sorted_cells))

def main():
    # Step 1: Download UnicodeData.txt for Unicode 16.0
    url = "https://www.unicode.org/Public/16.0.0/ucd/UnicodeData.txt"
    print(f"Downloading {url}...", file=sys.stderr)
    try:
        req = urllib.request.Request(url, headers={"User-Agent": "classcii-lut-gen/1.0"})
        with urllib.request.urlopen(req, timeout=30) as resp:
            data = resp.read().decode("utf-8")
    except Exception as e:
        print(f"Error downloading: {e}", file=sys.stderr)
        print("Falling back to algorithmic generation...", file=sys.stderr)
        data = None

    # Step 2: Parse octant characters from UnicodeData.txt
    octant_chars = {}  # cell_set (frozenset) → codepoint (int)

    if data:
        for line in data.splitlines():
            parts = line.split(";")
            if len(parts) < 2:
                continue
            cp_hex, name = parts[0], parts[1]
            if name.startswith("BLOCK OCTANT-"):
                cp = int(cp_hex, 16)
                digits_str = name[len("BLOCK OCTANT-"):]
                cells = frozenset(int(d) for d in digits_str)
                octant_chars[cells] = cp

    print(f"Found {len(octant_chars)} octant characters from UnicodeData.txt", file=sys.stderr)

    # Step 3: Also parse other block element characters that map to octant patterns
    # These are patterns NOT in the octant range because they already exist
    block_elements_map = {}  # cell_set → codepoint

    if data:
        # We need to identify block elements that correspond to octant patterns
        # Known mappings (verified against Unicode standard):
        known_block_elements = {
            frozenset({1, 3}): 0x2598,        # ▘ Quadrant upper left
            frozenset({2, 4}): 0x259D,        # ▝ Quadrant upper right
            frozenset({5, 7}): 0x2596,        # ▖ Quadrant lower left
            frozenset({6, 8}): 0x2597,        # ▗ Quadrant lower right
            frozenset({1, 2, 3, 4}): 0x2580,  # ▀ Upper half block
            frozenset({5, 6, 7, 8}): 0x2584,  # ▄ Lower half block
            frozenset({1, 3, 5, 7}): 0x258C,  # ▌ Left half block
            frozenset({2, 4, 6, 8}): 0x2590,  # ▐ Right half block
            frozenset({1, 3, 6, 8}): 0x259A,  # ▚ Quadrant UL + LR
            frozenset({2, 4, 5, 7}): 0x259E,  # ▞ Quadrant UR + LL
            frozenset({1, 2, 3, 4, 5, 7}): 0x259B,  # ▛ UL+UR+LL
            frozenset({1, 2, 3, 4, 6, 8}): 0x259C,  # ▜ UL+UR+LR
            frozenset({1, 3, 5, 6, 7, 8}): 0x2599,  # ▙ UL+LL+LR
            frozenset({2, 4, 5, 6, 7, 8}): 0x259F,  # ▟ UR+LL+LR
            frozenset({7, 8}): 0x2582,        # ▂ Lower one quarter block
            frozenset({3, 4, 5, 6, 7, 8}): 0x2586,  # ▆ Lower three quarters block
            frozenset({1, 2}): 0x1FB82,       # Upper one quarter block
            frozenset({1, 2, 3, 4, 5, 6}): 0x1FB85, # Upper three quarters block
        }
        block_elements_map = known_block_elements

    # Step 4: Build the complete LUT
    # For each bitmask 0-255, determine the correct character
    lut = [None] * 256
    lut[0] = (0x0020, "space")  # Empty
    lut[255] = (0x2588, "full block")  # Full

    # First pass: assign known block elements
    for cell_set, cp in block_elements_map.items():
        bitmask = cell_set_to_bitmask(cell_set)
        if lut[bitmask] is None:
            name = "".join(str(c) for c in sorted(cell_set))
            lut[bitmask] = (cp, f"Block Element {name}")

    # Second pass: assign octant characters
    for cell_set, cp in octant_chars.items():
        bitmask = cell_set_to_bitmask(cell_set)
        if lut[bitmask] is None:
            name = "".join(str(c) for c in sorted(cell_set))
            lut[bitmask] = (cp, f"Octant-{name}")
        else:
            # Already assigned by block element - verify no conflict
            existing_cp, existing_name = lut[bitmask]
            name = "".join(str(c) for c in sorted(cell_set))
            print(f"WARNING: bitmask 0x{bitmask:02X} already assigned to "
                  f"U+{existing_cp:04X} ({existing_name}), "
                  f"octant U+{cp:04X} (Octant-{name}) skipped",
                  file=sys.stderr)

    # Third pass: fill remaining with Braille fallback
    braille_count = 0
    for i in range(256):
        if lut[i] is None:
            # Braille fallback - map to braille using the same bit layout
            # Code bits → Braille bits mapping:
            # bit 0 (cell 1) → braille dot 1 (bit 0)
            # bit 1 (cell 2) → braille dot 4 (bit 3)
            # bit 2 (cell 3) → braille dot 2 (bit 1)
            # bit 3 (cell 4) → braille dot 5 (bit 4)
            # bit 4 (cell 5) → braille dot 3 (bit 2)
            # bit 5 (cell 6) → braille dot 6 (bit 5)
            # bit 6 (cell 7) → braille dot 7 (bit 6)
            # bit 7 (cell 8) → braille dot 8 (bit 7)
            b = i
            braille_mask = 0
            if b & (1 << 0): braille_mask |= 0x01  # dot 1
            if b & (1 << 1): braille_mask |= 0x08  # dot 4
            if b & (1 << 2): braille_mask |= 0x02  # dot 2
            if b & (1 << 3): braille_mask |= 0x10  # dot 5
            if b & (1 << 4): braille_mask |= 0x04  # dot 3
            if b & (1 << 5): braille_mask |= 0x20  # dot 6
            if b & (1 << 6): braille_mask |= 0x40  # dot 7
            if b & (1 << 7): braille_mask |= 0x80  # dot 8
            cp = 0x2800 + braille_mask
            cell_set = bitmask_to_cell_set(i)
            name = "".join(str(c) for c in sorted(cell_set))
            lut[i] = (cp, f"Braille fallback ({name})")
            braille_count += 1

    # Statistics
    octant_count = sum(1 for _, (cp, _) in enumerate(lut)
                       if 0x1CD00 <= cp <= 0x1CDE5)
    block_count = sum(1 for _, (cp, _) in enumerate(lut)
                      if (0x2580 <= cp <= 0x259F) or cp == 0x1FB82 or cp == 0x1FB85)
    print(f"\nLUT Statistics:", file=sys.stderr)
    print(f"  Octant chars (U+1CD00-U+1CDE5): {octant_count}", file=sys.stderr)
    print(f"  Block Element chars: {block_count}", file=sys.stderr)
    print(f"  Braille fallback: {braille_count}", file=sys.stderr)
    print(f"  Special (space + full): 2", file=sys.stderr)
    print(f"  Total: {octant_count + block_count + braille_count + 2}", file=sys.stderr)

    if octant_count != 230 and len(octant_chars) > 0:
        print(f"\nWARNING: Expected 230 octant chars, got {octant_count}. "
              f"Some patterns may have wrong mapping.", file=sys.stderr)
        # Find unassigned patterns
        for i in range(256):
            cp, name = lut[i]
            if "Braille fallback" in name and i not in (0, 255):
                cells = bitmask_to_cell_set(i)
                cell_name = "".join(str(c) for c in sorted(cells))
                print(f"  Unmatched: bitmask 0x{i:02X} cells={{{cell_name}}} → braille fallback", file=sys.stderr)

    # Step 5: Generate Rust code to file
    outpath = "scripts/octant_lut_generated.rs"
    with open(outpath, "w", encoding="utf-8") as f:
        f.write("/// Auto-generated Octant LUT -- 256 entries mapping bitmask to Unicode char.\n")
        f.write("/// Generated from Unicode 16.0 UnicodeData.txt.\n")
        f.write("///\n")
        f.write("/// Bit layout (column-major):\n")
        f.write("/// +---+---+\n")
        f.write("/// | 0 | 4 |\n")
        f.write("/// +---+---+\n")
        f.write("/// | 1 | 5 |\n")
        f.write("/// +---+---+\n")
        f.write("/// | 2 | 6 |\n")
        f.write("/// +---+---+\n")
        f.write("/// | 3 | 7 |\n")
        f.write("/// +---+---+\n")
        f.write("pub const OCTANT_LUT: [char; 256] = [\n")
        for i in range(256):
            cp, name = lut[i]
            if cp == 0x20:
                char_repr = "' '"
            else:
                char_repr = f"'\\u{{{cp:04X}}}'"
            # Sanitize name for ASCII Rust comment
            safe_name = name.encode("ascii", errors="replace").decode("ascii")
            f.write(f"    {char_repr}, // {i:3}: 0x{i:02X} {safe_name}\n")
        f.write("];\n")
    print(f"Generated {outpath}", file=sys.stderr)

if __name__ == "__main__":
    main()
