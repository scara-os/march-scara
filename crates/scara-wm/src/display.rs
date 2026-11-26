use serde::{Deserialize, Serialize};
use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::mpsc::Sender,
    time::Instant,
};

pub const PREVIEW_TIMEOUT_SECS: u64 = 15;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct DisplayMode {
    pub width: u16,
    pub height: u16,
    pub refresh_millihz: u32,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct DisplayConfig {
    pub name: String,
    pub mode: DisplayMode,
    pub scale: i32,
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct DisplayOutputInfo {
    pub name: String,
    pub backend: String,
    pub current: DisplayMode,
    pub modes: Vec<DisplayMode>,
    pub scale: i32,
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Debug)]
pub struct PendingPreview {
    pub previous: DisplayConfig,
    pub expires_at: Instant,
}

pub struct DisplayIpcRequest {
    pub command: String,
    pub reply: Sender<String>,
}

pub enum DisplayAction {
    List,
    Preview(DisplayConfig),
    Confirm,
    Cancel,
}

pub fn parse_command(command: &str) -> Result<DisplayAction, String> {
    let fields = command.split(':').collect::<Vec<_>>();
    match fields.as_slice() {
        ["display", "list"] => Ok(DisplayAction::List),
        ["display", "confirm"] => Ok(DisplayAction::Confirm),
        ["display", "cancel"] => Ok(DisplayAction::Cancel),
        ["display", "preview", name, width, height, refresh, scale, x, y] => {
            let config = DisplayConfig {
                name: (*name).to_string(),
                mode: DisplayMode {
                    width: width
                        .parse()
                        .map_err(|_| "Invalid display mode width.".to_string())?,
                    height: height
                        .parse()
                        .map_err(|_| "Invalid display mode height.".to_string())?,
                    refresh_millihz: refresh
                        .parse()
                        .map_err(|_| "Invalid display refresh rate.".to_string())?,
                },
                scale: scale
                    .parse()
                    .map_err(|_| "Invalid display scale.".to_string())?,
                x: x.parse()
                    .map_err(|_| "Invalid display horizontal position.".to_string())?,
                y: y.parse()
                    .map_err(|_| "Invalid display vertical position.".to_string())?,
            };
            validate_config(&config).map_err(|error| error.to_string())?;
            Ok(DisplayAction::Preview(config))
        }
        _ => Err("Invalid display command.".to_string()),
    }
}

pub fn config_path() -> io::Result<PathBuf> {
    let config_dir = if let Some(path) = std::env::var_os("XDG_CONFIG_HOME") {
        PathBuf::from(path)
    } else {
        PathBuf::from(std::env::var_os("HOME").ok_or_else(|| {
            io::Error::other("HOME is not set; cannot resolve display configuration path")
        })?)
        .join(".config")
    };
    Ok(config_dir.join("march-scara").join("displays.json"))
}

pub fn load_configs() -> io::Result<Vec<DisplayConfig>> {
    let path = config_path()?;
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let configs: Vec<DisplayConfig> = serde_json::from_str(&content)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    for config in &configs {
        validate_config(config)?;
    }
    Ok(configs)
}

pub fn save_configs(configs: &[DisplayConfig]) -> io::Result<()> {
    for config in configs {
        validate_config(config)?;
    }
    let path = config_path()?;
    write_configs(&path, configs)
}

pub fn update_configs(configs: &[DisplayConfig]) -> io::Result<()> {
    let mut saved = load_configs()?;
    for config in configs {
        validate_config(config)?;
        if let Some(existing) = saved.iter_mut().find(|saved| saved.name == config.name) {
            *existing = config.clone();
        } else {
            saved.push(config.clone());
        }
    }
    save_configs(&saved)
}

fn write_configs(path: &Path, configs: &[DisplayConfig]) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("Display configuration path has no parent directory"))?;
    fs::create_dir_all(parent)?;

    let temporary_path = path.with_extension("json.tmp");
    let contents = serde_json::to_vec_pretty(configs)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    fs::write(&temporary_path, contents)?;
    if let Err(error) = fs::rename(&temporary_path, path) {
        let _ = fs::remove_file(temporary_path);
        return Err(error);
    }
    Ok(())
}

pub fn validate_config(config: &DisplayConfig) -> io::Result<()> {
    if config.name.is_empty() || config.name.contains('\n') || config.name.contains('\r') {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Display name must be non-empty and cannot contain line breaks",
        ));
    }
    if !(1..=4).contains(&config.scale) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Display scale must be between 1 and 4",
        ));
    }
    if !(-16384..=16384).contains(&config.x) || !(-16384..=16384).contains(&config.y) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Display position must be within -16384 and 16384 logical pixels",
        ));
    }
    if config.mode.width == 0 || config.mode.height == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Display mode dimensions must be non-zero",
        ));
    }
    if !(1_000..=1_000_000).contains(&config.mode.refresh_millihz) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Display refresh rate must be between 1 and 1000 Hz",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{parse_command, DisplayAction};

    #[test]
    fn parses_display_preview_with_supported_config_fields() {
        let action = parse_command("display:preview:DP-1:1920:1080:60000:2:-1920:0")
            .expect("valid display preview");
        let DisplayAction::Preview(config) = action else {
            panic!("expected preview action");
        };
        assert_eq!(config.name, "DP-1");
        assert_eq!(config.mode.width, 1920);
        assert_eq!(config.mode.height, 1080);
        assert_eq!(config.mode.refresh_millihz, 60000);
        assert_eq!(config.scale, 2);
        assert_eq!((config.x, config.y), (-1920, 0));
    }

    #[test]
    fn rejects_invalid_display_preview_values() {
        assert!(parse_command("display:preview:DP-1:0:1080:60000:1:0:0").is_err());
        assert!(parse_command("display:preview:DP-1:1920:1080:60000:5:0:0").is_err());
        assert!(parse_command("display:preview:DP-1:1920:1080:60000:1:0:0:extra").is_err());
    }
}
