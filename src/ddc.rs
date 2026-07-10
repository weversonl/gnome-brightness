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
}
