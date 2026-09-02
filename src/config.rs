use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

pub const BASE_WIDTH: u32 = 614;
pub const BASE_HEIGHT: u32 = 387;

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct AppConfig {
    pub window: WindowConfig,
    pub appearance: AppearanceConfig,
    pub clock: ClockConfig,
    pub bottom: BottomConfig,
    pub weather: WeatherConfig,
    pub countdowns: Vec<CountdownConfig>,
}

impl AppConfig {
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let source =
            fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
        Self::parse(&source).with_context(|| format!("invalid configuration {}", path.display()))
    }

    pub fn parse(source: &str) -> Result<Self> {
        let raw: RawConfig = toml::from_str(source)?;
        let mut config = AppConfig {
            window: raw.window,
            appearance: raw.appearance,
            clock: raw.clock,
            bottom: raw.bottom.into_config(),
            weather: raw.weather,
            countdowns: raw.countdowns,
        };
        config.sanitize();
        Ok(config)
    }

    fn sanitize(&mut self) {
        self.window.anchor = choice(
            &self.window.anchor,
            &[
                "top left",
                "top center",
                "top right",
                "center left",
                "center",
                "center right",
                "bottom left",
                "bottom center",
                "bottom right",
            ],
            "top right",
        );
        self.window.scale = self.window.scale.clamp(0.25, 4.0);
        self.window.stacking = choice(
            &self.window.stacking,
            &["background", "bottom", "top", "overlay"],
            "bottom",
        );
        self.window.focusable = choice(
            &self.window.focusable,
            &["none", "exclusive", "ondemand"],
            "none",
        );
        self.appearance.scheme = choice(
            &self.appearance.scheme,
            &[
                "blue",
                "pink",
                "green",
                "yellow",
                "red",
                "light-green",
                "purple",
                "dark-blue",
                "grey",
                "custom",
            ],
            "blue",
        );
        self.appearance.emoji_size = self.appearance.emoji_size.clamp(10.0, 180.0);
        self.appearance.time_shadow_offset = self.appearance.time_shadow_offset.clamp(0.0, 30.0);
        self.appearance.caption_shadow_offset =
            self.appearance.caption_shadow_offset.clamp(0.0, 30.0);
        self.appearance.time_font.sanitize();
        self.appearance.date_font.sanitize();
        self.appearance.caption_font.sanitize();
        self.clock.language = choice(&self.clock.language, &["auto", "en", "zh"], "auto");
        self.bottom.mode = choice(
            &self.bottom.mode,
            &[
                "none",
                "moon",
                "moon-countdown",
                "weather",
                "weather-temp-c",
                "weather-temp-f",
                "weather-rain",
                "custom-countdown",
                "moon-custom-countdown",
            ],
            "moon-countdown",
        );
        self.bottom
            .moon_targets
            .retain(|value| matches!(value.as_str(), "new" | "first" | "full" | "last"));
        self.bottom.moon_targets.dedup();
        if self.bottom.moon_targets.is_empty() {
            self.bottom.moon_targets.push("full".into());
        }
        self.weather.provider = choice(&self.weather.provider, &["amap", "weatherapi"], "amap");
        self.weather.update_minutes = self.weather.update_minutes.max(15);
        self.appearance
            .custom_colors
            .retain(|key, _| Palette::KEYS.contains(&key.as_str()));
    }

    pub fn window_size(&self) -> (u32, u32) {
        (
            ((BASE_WIDTH as f32 * self.window.scale).round() as u32).max(1),
            ((BASE_HEIGHT as f32 * self.window.scale).round() as u32).max(1),
        )
    }

    pub fn uses_weather(&self) -> bool {
        matches!(
            self.bottom.mode.as_str(),
            "weather" | "weather-temp-c" | "weather-temp-f" | "weather-rain"
        )
    }

    pub fn palette(&self) -> Palette {
        let mut palette = Palette::named(&self.appearance.scheme);
        if self.appearance.scheme == "custom" {
            palette.apply(&self.appearance.custom_colors);
        }
        if self.appearance.invert_bottom {
            std::mem::swap(&mut palette.bottom, &mut palette.highlight);
        }
        palette
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default)]
pub struct WindowConfig {
    pub monitor: String,
    pub anchor: String,
    pub x: i32,
    pub y: i32,
    pub scale: f32,
    pub stacking: String,
    pub exclusive: bool,
    pub focusable: String,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            monitor: "primary".into(),
            anchor: "top right".into(),
            x: 0,
            y: 0,
            scale: 1.0,
            stacking: "bottom".into(),
            exclusive: false,
            focusable: "none".into(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default)]
pub struct AppearanceConfig {
    pub scheme: String,
    pub invert_bottom: bool,
    pub highlight_numbers: bool,
    pub emoji_size: f32,
    pub time_shadow: bool,
    pub time_shadow_offset: f32,
    pub caption_shadow: bool,
    pub caption_shadow_offset: f32,
    pub time_font: FontConfig,
    pub date_font: FontConfig,
    pub caption_font: FontConfig,
    pub custom_colors: BTreeMap<String, String>,
}

impl Default for AppearanceConfig {
    fn default() -> Self {
        Self {
            scheme: "blue".into(),
            invert_bottom: false,
            highlight_numbers: true,
            emoji_size: 70.0,
            time_shadow: true,
            time_shadow_offset: 5.0,
            caption_shadow: false,
            caption_shadow_offset: 3.0,
            time_font: FontConfig::new("sans-serif", 70.0, 400),
            date_font: FontConfig::new("sans-serif", 52.0, 700),
            caption_font: FontConfig::new("sans-serif", 50.0, 700),
            custom_colors: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default)]
pub struct FontConfig {
    pub family: String,
    pub size: f32,
    pub weight: u16,
    pub style: String,
}

impl FontConfig {
    fn new(family: &str, size: f32, weight: u16) -> Self {
        Self {
            family: family.into(),
            size,
            weight,
            style: "normal".into(),
        }
    }

    fn sanitize(&mut self) {
        self.size = self.size.clamp(8.0, 180.0);
        self.weight = self.weight.clamp(100, 900);
        self.style = choice(&self.style, &["normal", "italic", "oblique"], "normal");
    }
}

impl Default for FontConfig {
    fn default() -> Self {
        Self::new("sans-serif", 50.0, 400)
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default)]
pub struct ClockConfig {
    pub date_format: String,
    pub time_format: String,
    pub show_weekday: bool,
    pub language: String,
}

impl Default for ClockConfig {
    fn default() -> Self {
        Self {
            date_format: "%m / %d".into(),
            time_format: "%H : %M".into(),
            show_weekday: true,
            language: "auto".into(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default)]
pub struct BottomConfig {
    pub mode: String,
    pub moon_targets: Vec<String>,
    pub show_secondary_countdowns: bool,
}

impl Default for BottomConfig {
    fn default() -> Self {
        Self {
            mode: "moon-countdown".into(),
            moon_targets: vec!["full".into()],
            show_secondary_countdowns: true,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default)]
pub struct WeatherConfig {
    pub provider: String,
    pub enabled: bool,
    pub api_key: String,
    pub query: String,
    pub update_minutes: u64,
}

impl Default for WeatherConfig {
    fn default() -> Self {
        Self {
            provider: "amap".into(),
            enabled: false,
            api_key: String::new(),
            query: "110000".into(),
            update_minutes: 60,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default)]
pub struct CountdownConfig {
    pub name: String,
    pub date: String,
    pub enabled: bool,
    pub persistent: bool,
}

impl Default for CountdownConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            date: String::new(),
            enabled: true,
            persistent: false,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Default)]
#[serde(default)]
struct RawConfig {
    window: WindowConfig,
    appearance: AppearanceConfig,
    clock: ClockConfig,
    bottom: RawBottomConfig,
    weather: WeatherConfig,
    countdowns: Vec<CountdownConfig>,
}

#[derive(Clone, Debug, Deserialize, Default)]
#[serde(default)]
struct RawBottomConfig {
    mode: Option<String>,
    emoji: Option<String>,
    caption: Option<String>,
    moon_targets: Vec<String>,
    show_secondary_countdowns: Option<bool>,
}

impl RawBottomConfig {
    fn into_config(self) -> BottomConfig {
        let defaults = BottomConfig::default();
        let mode = self.mode.unwrap_or_else(|| {
            legacy_mode(
                self.emoji.as_deref().unwrap_or("moon"),
                self.caption.as_deref().unwrap_or("moon-countdown"),
            )
            .unwrap_or(defaults.mode)
        });
        BottomConfig {
            mode,
            moon_targets: if self.moon_targets.is_empty() {
                defaults.moon_targets
            } else {
                self.moon_targets
            },
            show_secondary_countdowns: self
                .show_secondary_countdowns
                .unwrap_or(defaults.show_secondary_countdowns),
        }
    }
}

fn legacy_mode(emoji: &str, caption: &str) -> Option<String> {
    let mode = match (emoji, caption) {
        ("none", "none") => "none",
        ("moon", "moon") => "moon",
        ("moon", "moon-countdown") => "moon-countdown",
        ("weather", "weather") => "weather",
        ("weather", "temp-c") => "weather-temp-c",
        ("weather", "temp-f") => "weather-temp-f",
        ("weather", "rain") => "weather-rain",
        ("none", "custom-countdown") => "custom-countdown",
        ("moon", "custom-countdown") => "moon-custom-countdown",
        _ => return None,
    };
    Some(mode.into())
}

fn choice(value: &str, choices: &[&str], default: &str) -> String {
    if choices.contains(&value) {
        value.into()
    } else {
        default.into()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Palette {
    pub corner1: String,
    pub corner2: String,
    pub time: String,
    pub date: String,
    pub bottom: String,
    pub shadow: String,
    pub highlight: String,
}

impl Palette {
    const KEYS: [&'static str; 7] = [
        "corner1",
        "corner2",
        "time",
        "date",
        "bottom",
        "shadow",
        "highlight",
    ];

    fn named(name: &str) -> Self {
        let values = match name {
            "pink" => [
                "#fc99b6", "#000000", "#ffffff", "#9f262d", "#fff0f5", "#9f262d", "#ffa4d8",
            ],
            "green" => [
                "#3ba960", "#000000", "#f5fffa", "#052808", "#f5fffa", "#052808", "#74ff8c",
            ],
            "yellow" => [
                "#fbfa0b", "#000000", "#ffffff", "#776e03", "#ffffe0", "#b6a906", "#ffff80",
            ],
            "red" => [
                "#ee1600", "#000000", "#ffffff", "#400000", "#fff0f0", "#400000", "#fe0000",
            ],
            "light-green" => [
                "#98fe1e", "#000000", "#ffffff", "#468000", "#f5fffa", "#79b82b", "#c2ff7a",
            ],
            "purple" => [
                "#8a2be2", "#000000", "#e6e6fa", "#3b0066", "#e6e6fa", "#4b0082", "#9370db",
            ],
            "dark-blue" => [
                "#0000b0", "#000000", "#ffffff", "#000020", "#f0f8ff", "#000040", "#0000ff",
            ],
            "grey" => [
                "#d3d3d3", "#000000", "#ffffff", "#696969", "#f0f0f0", "#808080", "#999999",
            ],
            _ => [
                "#67baff", "#000000", "#ffffff", "#226182", "#f0f8ff", "#447fab", "#7bffff",
            ],
        };
        Self {
            corner1: values[0].into(),
            corner2: values[1].into(),
            time: values[2].into(),
            date: values[3].into(),
            bottom: values[4].into(),
            shadow: values[5].into(),
            highlight: values[6].into(),
        }
    }

    fn apply(&mut self, values: &BTreeMap<String, String>) {
        for (key, value) in values {
            match key.as_str() {
                "corner1" => self.corner1.clone_from(value),
                "corner2" => self.corner2.clone_from(value),
                "time" => self.time.clone_from(value),
                "date" => self.date.clone_from(value),
                "bottom" => self.bottom.clone_from(value),
                "shadow" => self.shadow.clone_from(value),
                "highlight" => self.highlight.clone_from(value),
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_current_template() {
        let config = AppConfig::parse(crate::paths::DEFAULT_CONFIG).unwrap();
        assert_eq!(config.window_size(), (614, 387));
        assert_eq!(config.weather.provider, "amap");
        assert_eq!(config.bottom.mode, "moon-countdown");
    }

    #[test]
    fn generated_template_documents_every_section() {
        let template = crate::paths::DEFAULT_CONFIG;
        for section in [
            "[window]",
            "[appearance]",
            "[appearance.time_font]",
            "[appearance.date_font]",
            "[appearance.caption_font]",
            "[appearance.custom_colors]",
            "[clock]",
            "[bottom]",
            "[weather]",
            "#[[countdowns]]",
        ] {
            assert!(template.contains(section), "missing {section}");
        }
    }

    #[test]
    fn bounds_values_and_maps_legacy_bottom_mode() {
        let config = AppConfig::parse(
            r#"
                [window]
                scale = 99
                anchor = "invalid"
                [appearance]
                emoji_size = 2
                [bottom]
                emoji = "weather"
                caption = "temp-c"
                moon_targets = ["new", "invalid", "full"]
            "#,
        )
        .unwrap();
        assert_eq!(config.window.scale, 4.0);
        assert_eq!(config.window.anchor, "top right");
        assert_eq!(config.appearance.emoji_size, 10.0);
        assert_eq!(config.bottom.mode, "weather-temp-c");
        assert_eq!(config.bottom.moon_targets, ["new", "full"]);
    }
}
