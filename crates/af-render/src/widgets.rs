#[derive(Clone, Debug, Default)]
pub struct AudioPanelState {
    pub selected_row: usize, // 0=sensitivity, 1=smoothing, 2..=mapping[i-2]
    pub selected_col: usize, // 0=enabled, 1=source, 2=target, 3=amount, 4=offset
    pub total_rows: usize,   // 2 + mappings.len()
}

impl AudioPanelState {
    #[must_use]
    pub fn new(mapping_count: usize) -> Self {
        Self {
            selected_row: 0,
            selected_col: 0,
            total_rows: 2 + mapping_count,
        }
    }
}
