use std::process::Command;

use crate::monitor::Monitor;

fn parse_detect(output: &str) -> Vec<(u32, String, String)> {
    let mut result = Vec::new();
    let mut current_id: Option<u32> = None;

    for line in output.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("Display ") {
            current_id = rest.trim().parse().ok();
        } else if let Some(rest) = trimmed.strip_prefix("Monitor:") {
            if let Some(id) = current_id {
                let edid_key = rest.trim().to_string();
                let name = friendly_name(&edid_key);
                result.push((id, edid_key, name));
            }
        }
    }

    result
}

fn friendly_name(edid_key: &str) -> String {
    let mut parts = edid_key.split(':');
    let mfg = parts.next().unwrap_or("").trim();
    let model = parts.next().unwrap_or("").trim();
    if model.is_empty() {
        mfg.to_string()
    } else if mfg.is_empty() {
        model.to_string()
    } else {
        format!("{mfg} {model}")
    }
}

fn parse_getvcp_brief(output: &str) -> Option<(u8, u8)> {
    let line = output.lines().find(|l| l.trim_start().starts_with("VCP 10"))?;
    let mut fields = line.split_whitespace();
    fields.next()?;
    fields.next()?;
    fields.next()?;
    let current: u8 = fields.next()?.parse().ok()?;
    let max: u8 = fields.next()?.parse().ok()?;
    Some((current, max))
}

pub fn detect_monitors() -> Vec<Monitor> {
    let output = match Command::new("ddcutil").args(["detect", "--brief"]).output() {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    let raw = parse_detect(&stdout);

    raw.into_iter()
        .map(|(display_id, edid_key, name)| {
            match probe_brightness(display_id) {
                Some((current, max)) => Monitor {
                    display_id,
                    edid_key,
                    name,
                    value: current,
                    max_value: max,
                    supports_brightness: true,
                },
                None => Monitor {
                    display_id,
                    edid_key,
                    name,
                    value: 0,
                    max_value: 100,
                    supports_brightness: false,
                },
            }
        })
        .collect()
}

// Slow (probes the monitor over DDC/CI, can take seconds), so callers must
// run this off the GTK main thread.
pub fn get_input_sources_and_current(display_id: u32) -> (Vec<(u8, String)>, Option<u8>) {
    let sources = get_input_sources(display_id);
    if sources.is_empty() {
        return (sources, None);
    }
    let current = get_current_input_source(display_id);
    (sources, current)
}

fn parse_input_sources(output: &str) -> Vec<(u8, String)> {
    let mut lines = output.lines();
    let mut result = Vec::new();
    while let Some(line) = lines.next() {
        if !line.trim_start().starts_with("Feature: 60 ") {
            continue;
        }
        for value_line in lines.by_ref() {
            let trimmed = value_line.trim();
            if trimmed.eq_ignore_ascii_case("Values:") {
                continue;
            }
            let Some((code, name)) = trimmed.split_once(':') else {
                break;
            };
            let Ok(value) = u8::from_str_radix(code.trim(), 16) else {
                break;
            };
            result.push((value, name.trim().to_string()));
        }
        break;
    }
    result
}

fn parse_current_input(output: &str) -> Option<u8> {
    let line = output.lines().find(|l| l.trim_start().starts_with("VCP 60"))?;
    let field = line.split_whitespace().last()?;
    let hex = field.strip_prefix('x').or_else(|| field.strip_prefix("0x"))?;
    u8::from_str_radix(hex, 16).ok()
}

pub fn get_input_sources(display_id: u32) -> Vec<(u8, String)> {
    let output = match Command::new("ddcutil")
        .args(["--display", &display_id.to_string(), "capabilities"])
        .output()
    {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_input_sources(&stdout)
}

pub fn get_current_input_source(display_id: u32) -> Option<u8> {
    let output = Command::new("ddcutil")
        .args(["--display", &display_id.to_string(), "getvcp", "60", "--brief"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_current_input(&stdout)
}

// Fire-and-forget: errors are swallowed intentionally.
pub fn set_input_source(display_id: u32, value: u8) {
    let _ = Command::new("ddcutil")
        .args([
            "--display",
            &display_id.to_string(),
            "setvcp",
            "60",
            &value.to_string(),
        ])
        .output();
}

pub fn probe_brightness(display_id: u32) -> Option<(u8, u8)> {
    let output = Command::new("ddcutil")
        .args([
            "--display",
            &display_id.to_string(),
            "getvcp",
            "10",
            "--brief",
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_getvcp_brief(&stdout)
}

// Fire-and-forget: errors are swallowed intentionally.
pub fn set_brightness(display_id: u32, level: u8) {
    let _ = Command::new("ddcutil")
        .args([
            "--display",
            &display_id.to_string(),
            "--noverify",
            "setvcp",
            "10",
            &level.to_string(),
        ])
        .output();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_detect_brief() {
        let sample = "\
Display 1
   I2C bus:          /dev/i2c-8
   DRM connector:    card1-DP-1
   drm_connector_id: 383
   Monitor:          DEL:AW2725DM:JFH5F94

Display 2
   I2C bus:          /dev/i2c-9
   DRM connector:    card1-DP-2
   drm_connector_id: 393
   Monitor:          GSM:LG ULTRAGEAR:
";
        let parsed = parse_detect(sample);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0], (1, "DEL:AW2725DM:JFH5F94".to_string(), "DEL AW2725DM".to_string()));
        assert_eq!(parsed[1], (2, "GSM:LG ULTRAGEAR:".to_string(), "GSM LG ULTRAGEAR".to_string()));
    }

    #[test]
    fn parses_getvcp_brief() {
        assert_eq!(parse_getvcp_brief("VCP 10 C 5 100"), Some((5, 100)));
        assert_eq!(parse_getvcp_brief("some noise\nVCP 10 C 80 100\n"), Some((80, 100)));
        assert_eq!(parse_getvcp_brief("no vcp line here"), None);
    }

    #[test]
    fn parses_input_sources_from_capabilities() {
        let sample = "\
   Feature: 52 (Active control)
   Feature: 60 (Input Source)
      Values:
         0f: DisplayPort-1
         11: HDMI-1
         12: HDMI-2
   Feature: AC (Horizontal frequency)
";
        let parsed = parse_input_sources(sample);
        assert_eq!(
            parsed,
            vec![
                (0x0f, "DisplayPort-1".to_string()),
                (0x11, "HDMI-1".to_string()),
                (0x12, "HDMI-2".to_string()),
            ]
        );
    }

    #[test]
    fn parses_input_sources_missing_feature() {
        assert_eq!(parse_input_sources("   Feature: 10 (Brightness)\n"), Vec::new());
    }

    #[test]
    fn parses_current_input() {
        assert_eq!(parse_current_input("VCP 60 SNC x0f"), Some(0x0f));
        assert_eq!(parse_current_input("no vcp line here"), None);
    }
}
