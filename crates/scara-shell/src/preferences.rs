use serde::{Deserialize, Serialize};
use std::{
    fs, io,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeMode {
    #[default]
    Auto,
    Light,
    Dark,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GlassStyle {
    #[default]
    Standard,
    Clear,
    Tinted,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AccentColor {
    #[default]
    Blue,
    Purple,
    Pink,
    Red,
    Orange,
    Yellow,
    Green,
    Graphite,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct UserPreferences {
    pub theme: ThemeMode,
    pub glass: GlassStyle,
    pub accent: AccentColor,
    pub reduce_transparency: bool,
    pub wallpaper: Option<PathBuf>,
    pub display_name: Option<String>,
    pub avatar: Option<PathBuf>,
    pub clock_24_hour: bool,
    pub menu_bar: MenuBarModules,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct MenuBarModules {
    pub bluetooth: bool,
    pub battery: bool,
    pub wifi: bool,
    pub spotlight: bool,
    pub control_center: bool,
    pub clock: bool,
    pub sound: bool,
}

impl Default for MenuBarModules {
    fn default() -> Self {
        Self {
            bluetooth: true,
            battery: true,
            wifi: true,
            spotlight: true,
            control_center: true,
            clock: true,
            sound: true,
        }
    }
}

impl Default for UserPreferences {
    fn default() -> Self {
        Self {
            theme: ThemeMode::Auto,
            glass: GlassStyle::Standard,
            accent: AccentColor::Blue,
            reduce_transparency: false,
            wallpaper: None,
            display_name: None,
            avatar: None,
            clock_24_hour: false,
            menu_bar: MenuBarModules::default(),
        }
    }
}

fn preferences_path() -> io::Result<PathBuf> {
    let config_dir = if let Some(path) = std::env::var_os("XDG_CONFIG_HOME") {
        PathBuf::from(path)
    } else {
        PathBuf::from(std::env::var_os("HOME").ok_or_else(|| io::Error::other("HOME is not set"))?)
            .join(".config")
    };

    Ok(config_dir.join("march-scara").join("settings.json"))
}

pub fn load() -> io::Result<UserPreferences> {
    let path = preferences_path()?;
    read_from(&path)
}

fn read_from(path: &Path) -> io::Result<UserPreferences> {
    let contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(UserPreferences::default());
        }
        Err(error) => return Err(error),
    };

    serde_json::from_str(&contents)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

pub fn save(preferences: &UserPreferences) -> io::Result<()> {
    let path = preferences_path()?;
    write_to(&path, preferences)
}

fn write_to(path: &Path, preferences: &UserPreferences) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("Settings path has no parent directory"))?;
    fs::create_dir_all(parent)?;

    let temporary_path = path.with_extension("json.tmp");
    let contents = serde_json::to_vec_pretty(preferences)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    fs::write(&temporary_path, contents)?;
    if let Err(error) = fs::rename(&temporary_path, &path) {
        let _ = fs::remove_file(&temporary_path);
        return Err(error);
    }
    Ok(())
}

pub fn validate_wallpaper(path: &Path) -> io::Result<()> {
    if !path.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "Selected wallpaper is not a regular file",
        ));
    }

    let supported = path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "jpg" | "jpeg" | "png" | "webp"
            )
        });
    if !supported {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Choose a JPG, PNG, or WebP image",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{read_from, write_to, AccentColor, GlassStyle, ThemeMode, UserPreferences};
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn preferences_round_trip_through_json_storage() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after UNIX epoch")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "march-scara-settings-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&directory).expect("temporary settings directory should be created");
        let path = directory.join("settings.json");
        let expected = UserPreferences {
            theme: ThemeMode::Light,
            glass: GlassStyle::Tinted,
            accent: AccentColor::Purple,
            reduce_transparency: true,
            wallpaper: Some(directory.join("wallpaper.png")),
            display_name: Some("Test User".to_string()),
            avatar: Some(directory.join("avatar.png")),
            clock_24_hour: true,
            menu_bar: UserPreferences::default().menu_bar,
        };

        write_to(&path, &expected).expect("preferences should be written");
        assert_eq!(read_from(&path).expect("preferences should load"), expected);

        fs::remove_dir_all(directory).expect("temporary settings directory should be removed");
    }

    #[test]
    fn missing_preferences_use_defaults() {
        let directory = std::env::temp_dir().join(format!(
            "march-scara-missing-settings-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be after UNIX epoch")
                .as_nanos()
        ));
        fs::create_dir_all(&directory).expect("temporary settings directory should be created");
        let path = directory.join("settings.json");
        assert_eq!(
            read_from(&path).expect("missing settings should use defaults"),
            UserPreferences::default()
        );
        fs::remove_dir_all(directory).expect("temporary settings directory should be removed");
    }
}
