use std::collections::HashMap;
use std::fs;

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default)]
    pub nicknames: HashMap<String, String>,
    #[serde(default)]
    pub start_minimized: bool,
    #[serde(default = "default_window_width")]
    pub window_width: i32,
    #[serde(default = "default_window_height")]
    pub window_height: i32,
}

fn default_theme() -> String {
    "system".to_string()
}

fn default_window_width() -> i32 {
    720
}

fn default_window_height() -> i32 {
    420
}

impl Default for Config {
    fn default() -> Self {
        Config {
            theme: default_theme(),
            nicknames: HashMap::new(),
            start_minimized: false,
            window_width: default_window_width(),
            window_height: default_window_height(),
        }
    }
}

fn config_path() -> Option<std::path::PathBuf> {
    let dirs = ProjectDirs::from("com", "verso", "gnome-brightness")?;
    Some(dirs.config_dir().join("config.toml"))
}

impl Config {
    pub fn load() -> Config {
        let Some(path) = config_path() else {
            return Config::default();
        };
        let Ok(contents) = fs::read_to_string(&path) else {
            return Config::default();
        };
        toml::from_str(&contents).unwrap_or_default()
    }

    pub fn save(&self) {
        let Some(path) = config_path() else { return };
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(contents) = toml::to_string_pretty(self) {
            let _ = fs::write(path, contents);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_window_size_through_toml() {
        let mut cfg = Config::default();
        cfg.window_width = 900;
        cfg.window_height = 560;

        let serialized = toml::to_string_pretty(&cfg).unwrap();
        let parsed: Config = toml::from_str(&serialized).unwrap();

        assert_eq!(parsed.window_width, 900);
        assert_eq!(parsed.window_height, 560);
    }

    #[test]
    fn missing_window_size_falls_back_to_defaults() {
        let parsed: Config = toml::from_str("theme = \"dark\"\n").unwrap();
        assert_eq!(parsed.window_width, default_window_width());
        assert_eq!(parsed.window_height, default_window_height());
    }
}
