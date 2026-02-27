//! Topologies vectorielles complexes (Masques de Bits)
//! - Unicode 13.0 (Sextants)
//! - Unicode 16.0 (Octants)
//! - Unicode Braille Patterns

pub mod braille;
pub mod octants;
pub mod sextants;

pub use braille::get_braille_char;
pub use octants::get_octant_char;
pub use sextants::get_sextant_char;
