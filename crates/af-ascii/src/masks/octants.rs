//! Implémentation O(1) des Octants (Unicode 16.0)
//! Symbols for Legacy Computing Supplement — U+1CD00 à U+1CDE5
//!
//! LUT auto-générée depuis UnicodeData.txt (Unicode 16.0).
//! 230 vrais octants + 18 Block Elements préexistants + 6 Braille fallback.
//!
//! Bits activés (column-major) :
//! +---+---+
//! | 0 | 4 |
//! +---+---+
//! | 1 | 5 |
//! +---+---+
//! | 2 | 6 |
//! +---+---+
//! | 3 | 7 |
//! +---+---+
//!
//! Unicode cell numbering (row-major) :
//! +---+---+
//! | 1 | 2 |
//! +---+---+
//! | 3 | 4 |
//! +---+---+
//! | 5 | 6 |
//! +---+---+
//! | 7 | 8 |
//! +---+---+

/// LUT pré-calculée : 256 entrées bitmask → caractère Unicode.
/// Générée depuis Unicode 16.0 UnicodeData.txt via `scripts/gen_octant_lut.py`.
///
/// - 230 vrais octants (U+1CD00–U+1CDE5)
/// - 18 Block Elements (U+2580–U+259F, U+1FB82, U+1FB85) pour patterns déjà existants
/// - 6 Braille fallback (U+2800+) pour patterns non couverts ({1}, {2}, {7}, {8}, {3,5}, {4,6})
pub const OCTANT_LUT: [char; 256] = [
    ' ',         //   0: 0x00 — vide
    '\u{2801}',  //   1: 0x01 — Braille fallback {1}
    '\u{1CD00}', //   2: 0x02 — Octant-3
    '\u{2598}',  //   3: 0x03 — ▘ Quadrant upper left {1,3}
    '\u{1CD09}', //   4: 0x04 — Octant-5
    '\u{1CD0A}', //   5: 0x05 — Octant-15
    '\u{280A}',  //   6: 0x06 — Braille fallback {3,5}
    '\u{1CD0D}', //   7: 0x07 — Octant-135
    '\u{2810}',  //   8: 0x08 — Braille fallback {7}
    '\u{1CD36}', //   9: 0x09 — Octant-17
    '\u{1CD39}', //  10: 0x0A — Octant-37
    '\u{1CD3A}', //  11: 0x0B — Octant-137
    '\u{2596}',  //  12: 0x0C — ▖ Quadrant lower left {5,7}
    '\u{1CD45}', //  13: 0x0D — Octant-157
    '\u{1CD48}', //  14: 0x0E — Octant-357
    '\u{258C}',  //  15: 0x0F — ▌ Left half {1,3,5,7}
    '\u{2804}',  //  16: 0x10 — Braille fallback {2}
    '\u{1FB82}', //  17: 0x11 — Upper one quarter {1,2}
    '\u{1CD01}', //  18: 0x12 — Octant-23
    '\u{1CD02}', //  19: 0x13 — Octant-123
    '\u{1CD0B}', //  20: 0x14 — Octant-25
    '\u{1CD0C}', //  21: 0x15 — Octant-125
    '\u{1CD0E}', //  22: 0x16 — Octant-235
    '\u{1CD0F}', //  23: 0x17 — Octant-1235
    '\u{1CD37}', //  24: 0x18 — Octant-27
    '\u{1CD38}', //  25: 0x19 — Octant-127
    '\u{1CD3B}', //  26: 0x1A — Octant-237
    '\u{1CD3C}', //  27: 0x1B — Octant-1237
    '\u{1CD46}', //  28: 0x1C — Octant-257
    '\u{1CD47}', //  29: 0x1D — Octant-1257
    '\u{1CD49}', //  30: 0x1E — Octant-2357
    '\u{1CD4A}', //  31: 0x1F — Octant-12357
    '\u{1CD03}', //  32: 0x20 — Octant-4
    '\u{1CD04}', //  33: 0x21 — Octant-14
    '\u{1CD06}', //  34: 0x22 — Octant-34
    '\u{1CD07}', //  35: 0x23 — Octant-134
    '\u{1CD10}', //  36: 0x24 — Octant-45
    '\u{1CD11}', //  37: 0x25 — Octant-145
    '\u{1CD14}', //  38: 0x26 — Octant-345
    '\u{1CD15}', //  39: 0x27 — Octant-1345
    '\u{1CD3D}', //  40: 0x28 — Octant-47
    '\u{1CD3E}', //  41: 0x29 — Octant-147
    '\u{1CD41}', //  42: 0x2A — Octant-347
    '\u{1CD42}', //  43: 0x2B — Octant-1347
    '\u{1CD4B}', //  44: 0x2C — Octant-457
    '\u{1CD4C}', //  45: 0x2D — Octant-1457
    '\u{1CD4E}', //  46: 0x2E — Octant-3457
    '\u{1CD4F}', //  47: 0x2F — Octant-13457
    '\u{259D}',  //  48: 0x30 — ▝ Quadrant upper right {2,4}
    '\u{1CD05}', //  49: 0x31 — Octant-124
    '\u{1CD08}', //  50: 0x32 — Octant-234
    '\u{2580}',  //  51: 0x33 — ▀ Upper half {1,2,3,4}
    '\u{1CD12}', //  52: 0x34 — Octant-245
    '\u{1CD13}', //  53: 0x35 — Octant-1245
    '\u{1CD16}', //  54: 0x36 — Octant-2345
    '\u{1CD17}', //  55: 0x37 — Octant-12345
    '\u{1CD3F}', //  56: 0x38 — Octant-247
    '\u{1CD40}', //  57: 0x39 — Octant-1247
    '\u{1CD43}', //  58: 0x3A — Octant-2347
    '\u{1CD44}', //  59: 0x3B — Octant-12347
    '\u{259E}',  //  60: 0x3C — ▞ Quadrant UR+LL {2,4,5,7}
    '\u{1CD4D}', //  61: 0x3D — Octant-12457
    '\u{1CD50}', //  62: 0x3E — Octant-23457
    '\u{259B}',  //  63: 0x3F — ▛ UL+UR+LL {1,2,3,4,5,7}
    '\u{1CD18}', //  64: 0x40 — Octant-6
    '\u{1CD19}', //  65: 0x41 — Octant-16
    '\u{1CD1C}', //  66: 0x42 — Octant-36
    '\u{1CD1D}', //  67: 0x43 — Octant-136
    '\u{1CD27}', //  68: 0x44 — Octant-56
    '\u{1CD28}', //  69: 0x45 — Octant-156
    '\u{1CD2B}', //  70: 0x46 — Octant-356
    '\u{1CD2C}', //  71: 0x47 — Octant-1356
    '\u{1CD51}', //  72: 0x48 — Octant-67
    '\u{1CD52}', //  73: 0x49 — Octant-167
    '\u{1CD55}', //  74: 0x4A — Octant-367
    '\u{1CD56}', //  75: 0x4B — Octant-1367
    '\u{1CD61}', //  76: 0x4C — Octant-567
    '\u{1CD62}', //  77: 0x4D — Octant-1567
    '\u{1CD65}', //  78: 0x4E — Octant-3567
    '\u{1CD66}', //  79: 0x4F — Octant-13567
    '\u{1CD1A}', //  80: 0x50 — Octant-26
    '\u{1CD1B}', //  81: 0x51 — Octant-126
    '\u{1CD1E}', //  82: 0x52 — Octant-236
    '\u{1CD1F}', //  83: 0x53 — Octant-1236
    '\u{1CD29}', //  84: 0x54 — Octant-256
    '\u{1CD2A}', //  85: 0x55 — Octant-1256
    '\u{1CD2D}', //  86: 0x56 — Octant-2356
    '\u{1CD2E}', //  87: 0x57 — Octant-12356
    '\u{1CD53}', //  88: 0x58 — Octant-267
    '\u{1CD54}', //  89: 0x59 — Octant-1267
    '\u{1CD57}', //  90: 0x5A — Octant-2367
    '\u{1CD58}', //  91: 0x5B — Octant-12367
    '\u{1CD63}', //  92: 0x5C — Octant-2567
    '\u{1CD64}', //  93: 0x5D — Octant-12567
    '\u{1CD67}', //  94: 0x5E — Octant-23567
    '\u{1CD68}', //  95: 0x5F — Octant-123567
    '\u{2860}',  //  96: 0x60 — Braille fallback {4,6}
    '\u{1CD20}', //  97: 0x61 — Octant-146
    '\u{1CD23}', //  98: 0x62 — Octant-346
    '\u{1CD24}', //  99: 0x63 — Octant-1346
    '\u{1CD2F}', // 100: 0x64 — Octant-456
    '\u{1CD30}', // 101: 0x65 — Octant-1456
    '\u{1CD33}', // 102: 0x66 — Octant-3456
    '\u{1CD34}', // 103: 0x67 — Octant-13456
    '\u{1CD59}', // 104: 0x68 — Octant-467
    '\u{1CD5A}', // 105: 0x69 — Octant-1467
    '\u{1CD5D}', // 106: 0x6A — Octant-3467
    '\u{1CD5E}', // 107: 0x6B — Octant-13467
    '\u{1CD69}', // 108: 0x6C — Octant-4567
    '\u{1CD6A}', // 109: 0x6D — Octant-14567
    '\u{1CD6D}', // 110: 0x6E — Octant-34567
    '\u{1CD6E}', // 111: 0x6F — Octant-134567
    '\u{1CD21}', // 112: 0x70 — Octant-246
    '\u{1CD22}', // 113: 0x71 — Octant-1246
    '\u{1CD25}', // 114: 0x72 — Octant-2346
    '\u{1CD26}', // 115: 0x73 — Octant-12346
    '\u{1CD31}', // 116: 0x74 — Octant-2456
    '\u{1CD32}', // 117: 0x75 — Octant-12456
    '\u{1CD35}', // 118: 0x76 — Octant-23456
    '\u{1FB85}', // 119: 0x77 — Upper three quarters {1,2,3,4,5,6}
    '\u{1CD5B}', // 120: 0x78 — Octant-2467
    '\u{1CD5C}', // 121: 0x79 — Octant-12467
    '\u{1CD5F}', // 122: 0x7A — Octant-23467
    '\u{1CD60}', // 123: 0x7B — Octant-123467
    '\u{1CD6B}', // 124: 0x7C — Octant-24567
    '\u{1CD6C}', // 125: 0x7D — Octant-124567
    '\u{1CD6F}', // 126: 0x7E — Octant-234567
    '\u{1CD70}', // 127: 0x7F — Octant-1234567
    '\u{2880}',  // 128: 0x80 — Braille fallback {8}
    '\u{1CD71}', // 129: 0x81 — Octant-18
    '\u{1CD74}', // 130: 0x82 — Octant-38
    '\u{1CD75}', // 131: 0x83 — Octant-138
    '\u{1CD80}', // 132: 0x84 — Octant-58
    '\u{1CD81}', // 133: 0x85 — Octant-158
    '\u{1CD84}', // 134: 0x86 — Octant-358
    '\u{1CD85}', // 135: 0x87 — Octant-1358
    '\u{2582}',  // 136: 0x88 — ▂ Lower one quarter {7,8}
    '\u{1CDAC}', // 137: 0x89 — Octant-178
    '\u{1CDAF}', // 138: 0x8A — Octant-378
    '\u{1CDB0}', // 139: 0x8B — Octant-1378
    '\u{1CDBB}', // 140: 0x8C — Octant-578
    '\u{1CDBC}', // 141: 0x8D — Octant-1578
    '\u{1CDBF}', // 142: 0x8E — Octant-3578
    '\u{1CDC0}', // 143: 0x8F — Octant-13578
    '\u{1CD72}', // 144: 0x90 — Octant-28
    '\u{1CD73}', // 145: 0x91 — Octant-128
    '\u{1CD76}', // 146: 0x92 — Octant-238
    '\u{1CD77}', // 147: 0x93 — Octant-1238
    '\u{1CD82}', // 148: 0x94 — Octant-258
    '\u{1CD83}', // 149: 0x95 — Octant-1258
    '\u{1CD86}', // 150: 0x96 — Octant-2358
    '\u{1CD87}', // 151: 0x97 — Octant-12358
    '\u{1CDAD}', // 152: 0x98 — Octant-278
    '\u{1CDAE}', // 153: 0x99 — Octant-1278
    '\u{1CDB1}', // 154: 0x9A — Octant-2378
    '\u{1CDB2}', // 155: 0x9B — Octant-12378
    '\u{1CDBD}', // 156: 0x9C — Octant-2578
    '\u{1CDBE}', // 157: 0x9D — Octant-12578
    '\u{1CDC1}', // 158: 0x9E — Octant-23578
    '\u{1CDC2}', // 159: 0x9F — Octant-123578
    '\u{1CD78}', // 160: 0xA0 — Octant-48
    '\u{1CD79}', // 161: 0xA1 — Octant-148
    '\u{1CD7C}', // 162: 0xA2 — Octant-348
    '\u{1CD7D}', // 163: 0xA3 — Octant-1348
    '\u{1CD88}', // 164: 0xA4 — Octant-458
    '\u{1CD89}', // 165: 0xA5 — Octant-1458
    '\u{1CD8C}', // 166: 0xA6 — Octant-3458
    '\u{1CD8D}', // 167: 0xA7 — Octant-13458
    '\u{1CDB3}', // 168: 0xA8 — Octant-478
    '\u{1CDB4}', // 169: 0xA9 — Octant-1478
    '\u{1CDB7}', // 170: 0xAA — Octant-3478
    '\u{1CDB8}', // 171: 0xAB — Octant-13478
    '\u{1CDC3}', // 172: 0xAC — Octant-4578
    '\u{1CDC4}', // 173: 0xAD — Octant-14578
    '\u{1CDC7}', // 174: 0xAE — Octant-34578
    '\u{1CDC8}', // 175: 0xAF — Octant-134578
    '\u{1CD7A}', // 176: 0xB0 — Octant-248
    '\u{1CD7B}', // 177: 0xB1 — Octant-1248
    '\u{1CD7E}', // 178: 0xB2 — Octant-2348
    '\u{1CD7F}', // 179: 0xB3 — Octant-12348
    '\u{1CD8A}', // 180: 0xB4 — Octant-2458
    '\u{1CD8B}', // 181: 0xB5 — Octant-12458
    '\u{1CD8E}', // 182: 0xB6 — Octant-23458
    '\u{1CD8F}', // 183: 0xB7 — Octant-123458
    '\u{1CDB5}', // 184: 0xB8 — Octant-2478
    '\u{1CDB6}', // 185: 0xB9 — Octant-12478
    '\u{1CDB9}', // 186: 0xBA — Octant-23478
    '\u{1CDBA}', // 187: 0xBB — Octant-123478
    '\u{1CDC5}', // 188: 0xBC — Octant-24578
    '\u{1CDC6}', // 189: 0xBD — Octant-124578
    '\u{1CDC9}', // 190: 0xBE — Octant-234578
    '\u{1CDCA}', // 191: 0xBF — Octant-1234578
    '\u{2597}',  // 192: 0xC0 — ▗ Quadrant lower right {6,8}
    '\u{1CD90}', // 193: 0xC1 — Octant-168
    '\u{1CD93}', // 194: 0xC2 — Octant-368
    '\u{259A}',  // 195: 0xC3 — ▚ UL+LR diagonal {1,3,6,8}
    '\u{1CD9C}', // 196: 0xC4 — Octant-568
    '\u{1CD9D}', // 197: 0xC5 — Octant-1568
    '\u{1CDA0}', // 198: 0xC6 — Octant-3568
    '\u{1CDA1}', // 199: 0xC7 — Octant-13568
    '\u{1CDCB}', // 200: 0xC8 — Octant-678
    '\u{1CDCC}', // 201: 0xC9 — Octant-1678
    '\u{1CDCF}', // 202: 0xCA — Octant-3678
    '\u{1CDD0}', // 203: 0xCB — Octant-13678
    '\u{2584}',  // 204: 0xCC — ▄ Lower half {5,6,7,8}
    '\u{1CDDB}', // 205: 0xCD — Octant-15678
    '\u{1CDDE}', // 206: 0xCE — Octant-35678
    '\u{2599}',  // 207: 0xCF — ▙ UL+LL+LR {1,3,5,6,7,8}
    '\u{1CD91}', // 208: 0xD0 — Octant-268
    '\u{1CD92}', // 209: 0xD1 — Octant-1268
    '\u{1CD94}', // 210: 0xD2 — Octant-2368
    '\u{1CD95}', // 211: 0xD3 — Octant-12368
    '\u{1CD9E}', // 212: 0xD4 — Octant-2568
    '\u{1CD9F}', // 213: 0xD5 — Octant-12568
    '\u{1CDA2}', // 214: 0xD6 — Octant-23568
    '\u{1CDA3}', // 215: 0xD7 — Octant-123568
    '\u{1CDCD}', // 216: 0xD8 — Octant-2678
    '\u{1CDCE}', // 217: 0xD9 — Octant-12678
    '\u{1CDD1}', // 218: 0xDA — Octant-23678
    '\u{1CDD2}', // 219: 0xDB — Octant-123678
    '\u{1CDDC}', // 220: 0xDC — Octant-25678
    '\u{1CDDD}', // 221: 0xDD — Octant-125678
    '\u{1CDDF}', // 222: 0xDE — Octant-235678
    '\u{1CDE0}', // 223: 0xDF — Octant-1235678
    '\u{1CD96}', // 224: 0xE0 — Octant-468
    '\u{1CD97}', // 225: 0xE1 — Octant-1468
    '\u{1CD99}', // 226: 0xE2 — Octant-3468
    '\u{1CD9A}', // 227: 0xE3 — Octant-13468
    '\u{1CDA4}', // 228: 0xE4 — Octant-4568
    '\u{1CDA5}', // 229: 0xE5 — Octant-14568
    '\u{1CDA8}', // 230: 0xE6 — Octant-34568
    '\u{1CDA9}', // 231: 0xE7 — Octant-134568
    '\u{1CDD3}', // 232: 0xE8 — Octant-4678
    '\u{1CDD4}', // 233: 0xE9 — Octant-14678
    '\u{1CDD7}', // 234: 0xEA — Octant-34678
    '\u{1CDD8}', // 235: 0xEB — Octant-134678
    '\u{1CDE1}', // 236: 0xEC — Octant-45678
    '\u{1CDE2}', // 237: 0xED — Octant-145678
    '\u{2586}',  // 238: 0xEE — ▆ Lower three quarters {3,4,5,6,7,8}
    '\u{1CDE4}', // 239: 0xEF — Octant-1345678
    '\u{2590}',  // 240: 0xF0 — ▐ Right half {2,4,6,8}
    '\u{1CD98}', // 241: 0xF1 — Octant-12468
    '\u{1CD9B}', // 242: 0xF2 — Octant-23468
    '\u{259C}',  // 243: 0xF3 — ▜ UL+UR+LR {1,2,3,4,6,8}
    '\u{1CDA6}', // 244: 0xF4 — Octant-24568
    '\u{1CDA7}', // 245: 0xF5 — Octant-124568
    '\u{1CDAA}', // 246: 0xF6 — Octant-234568
    '\u{1CDAB}', // 247: 0xF7 — Octant-1234568
    '\u{1CDD5}', // 248: 0xF8 — Octant-24678
    '\u{1CDD6}', // 249: 0xF9 — Octant-124678
    '\u{1CDD9}', // 250: 0xFA — Octant-234678
    '\u{1CDDA}', // 251: 0xFB — Octant-1234678
    '\u{259F}',  // 252: 0xFC — ▟ UR+LL+LR {2,4,5,6,7,8}
    '\u{1CDE3}', // 253: 0xFD — Octant-1245678
    '\u{1CDE5}', // 254: 0xFE — Octant-2345678
    '\u{2588}',  // 255: 0xFF — █ Full block
];

/// Lookup O(1) : bitmask 0..=255 → caractère octant.
#[must_use]
#[inline(always)]
pub const fn get_octant_char(bitmask: u8) -> char {
    OCTANT_LUT[bitmask as usize]
}

use af_core::config::RenderConfig;
use af_core::frame::{AsciiCell, AsciiGrid, FrameBuffer};

/// Process frame in octant mode (2×4 sub-pixels per terminal cell).
pub fn process_octant(frame: &FrameBuffer, config: &RenderConfig, grid: &mut AsciiGrid) {
    let pixel_w = u32::from(grid.width) * 2;
    let pixel_h = u32::from(grid.height) * 4;
    crate::for_each_row(&mut grid.cells, grid.width as usize, |cy, row| {
        for (cx, cell) in row.iter_mut().enumerate() {
            let base_x = (cx as u32) * 2 * frame.width / pixel_w.max(1);
            let base_y = (cy as u32) * 4 * frame.height / pixel_h.max(1);

            // Passe 1 : collecter luminances et couleurs
            let mut lum_values = [0u8; 8];
            let mut lum_sum = 0u32;
            let mut avg_r = 0u32;
            let mut avg_g = 0u32;
            let mut avg_b = 0u32;

            for dy in 0..4u32 {
                for dx in 0..2u32 {
                    let px = (base_x + dx * frame.width / pixel_w.max(1))
                        .min(frame.width.saturating_sub(1));
                    let py = (base_y + dy * frame.height / pixel_h.max(1))
                        .min(frame.height.saturating_sub(1));

                    let raw_lum = frame.luminance_linear(px, py);
                    let lum = crate::adjust_lum(raw_lum, config.contrast, config.brightness);
                    let (r, g, b, _) = frame.pixel(px, py);
                    let idx = (dy * 2 + dx) as usize;
                    lum_values[idx] = lum;
                    lum_sum += u32::from(lum);

                    avg_r += u32::from(r);
                    avg_g += u32::from(g);
                    avg_b += u32::from(b);
                }
            }

            // Passe 2 : seuil adaptatif (moyenne locale)
            let local_threshold = (lum_sum / 8) as u8;
            let mut bitmask = 0u8;
            for bit in 0..8u8 {
                let on = if config.invert {
                    lum_values[bit as usize] < local_threshold
                } else {
                    lum_values[bit as usize] > local_threshold
                };
                if on {
                    bitmask |= 1 << bit;
                }
            }

            let ch = get_octant_char(bitmask);
            let fg = ((avg_r / 8) as u8, (avg_g / 8) as u8, (avg_b / 8) as u8);

            *cell = AsciiCell {
                ch,
                fg,
                bg: (0, 0, 0),
            };
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lut_boundaries() {
        assert_eq!(OCTANT_LUT[0], ' ', "index 0 must be space");
        assert_eq!(OCTANT_LUT[255], '\u{2588}', "index 255 must be full block");
    }

    #[test]
    fn lut_block_elements_preserved() {
        assert_eq!(OCTANT_LUT[3], '\u{2598}', "index 3 must be ▘ quadrant UL");
        assert_eq!(
            OCTANT_LUT[204], '\u{2584}',
            "index 204 must be ▄ lower half"
        );
        assert_eq!(OCTANT_LUT[51], '\u{2580}', "index 51 must be ▀ upper half");
        assert_eq!(OCTANT_LUT[15], '\u{258C}', "index 15 must be ▌ left half");
        assert_eq!(
            OCTANT_LUT[240], '\u{2590}',
            "index 240 must be ▐ right half"
        );
    }

    #[test]
    fn lut_real_octants_in_range() {
        // Index 2 (bitmask 0x02 = cell 3 only) must be a real octant U+1CD00-U+1CDE5
        let ch = OCTANT_LUT[2];
        let cp = ch as u32;
        assert!(
            (0x1CD00..=0x1CDE5).contains(&cp),
            "index 2 should be real octant, got U+{cp:04X}"
        );
    }

    #[test]
    fn lut_braille_fallbacks_in_range() {
        // 6 patterns without octant/block element coverage use Braille fallback
        let braille_indices = [1, 6, 8, 16, 96, 128];
        for &idx in &braille_indices {
            let cp = OCTANT_LUT[idx] as u32;
            assert!(
                (0x2800..=0x28FF).contains(&cp),
                "index {idx} should be braille fallback, got U+{cp:04X}"
            );
        }
    }

    #[test]
    fn lut_all_chars_valid() {
        for (i, &ch) in OCTANT_LUT.iter().enumerate() {
            assert!(ch != '\0', "index {i} must not be null char");
        }
    }
}
