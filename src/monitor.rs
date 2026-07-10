#[derive(Debug, Clone)]
pub struct Monitor {
    pub display_id: u32,
    // Stable identity across replugs (ddcutil's display_id is not).
    pub edid_key: String,
    pub name: String,
    pub value: u8,
    pub max_value: u8,
    pub supports_brightness: bool,
}

impl Monitor {
    pub fn percent(&self) -> u8 {
        if self.max_value == 0 {
            0
        } else {
            ((self.value as u32 * 100) / self.max_value as u32) as u8
        }
    }
}
