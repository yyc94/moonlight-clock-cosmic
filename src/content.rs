use std::fmt::Write;

use chrono::format::{Item, StrftimeItems};
use chrono::{DateTime, Datelike, FixedOffset, NaiveDate, Timelike};

use crate::astronomy::{next_quarter, phase_code};
use crate::config::{AppConfig, CountdownConfig};
use crate::weather::{WeatherErrorKind, WeatherIcon, WeatherSnapshot};

#[derive(Clone, Debug, PartialEq)]
pub enum Icon {
    Moon(&'static str),
    Weather(WeatherIcon),
    Warning,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClockContent {
    pub date_text: String,
    pub weekday: String,
    pub time_text: String,
    pub icon: Option<Icon>,
    pub caption: String,
    pub heading: String,
    pub number: String,
    pub unit: String,
    pub secondary: String,
    pub warning: bool,
}

pub fn build_content(
    config: &AppConfig,
    now: DateTime<FixedOffset>,
    weather: Option<&WeatherSnapshot>,
    weather_error: Option<WeatherErrorKind>,
) -> ClockContent {
    let language = resolve_language(&config.clock.language);
    let phase = phase_code(now.date_naive());
    let period = phase_of_day(now.hour(), language);
    let date_format = config.clock.date_format.replace("%!", period);
    let time_format = config.clock.time_format.replace("%!", period);
    let mut content = ClockContent {
        date_text: format_datetime(now, &date_format),
        weekday: if config.clock.show_weekday {
            weekday(now.weekday().num_days_from_monday(), language).into()
        } else {
            String::new()
        },
        time_text: format_datetime(now, &time_format),
        ..ClockContent::default()
    };

    match config.bottom.mode.as_str() {
        "moon" => {
            content.icon = Some(Icon::Moon(phase));
            content.caption = phase_name(phase, language).into();
        }
        "moon-countdown" => {
            content.icon = Some(Icon::Moon(phase));
            let (target, target_phase) =
                next_quarter(now.date_naive(), &config.bottom.moon_targets);
            let days = (target - now.date_naive()).num_days();
            if days == 0 {
                content.caption = quarter_name(target_phase, language).into();
            } else {
                content.heading = if language == "zh" {
                    "下次："
                } else {
                    "Next:"
                }
                .into();
                content.number = days.to_string();
                content.unit = "/".into();
            }
        }
        "weather" => weather_content(&mut content, weather, weather_error, language, "weather"),
        "weather-temp-c" => {
            weather_content(&mut content, weather, weather_error, language, "temp-c")
        }
        "weather-temp-f" => {
            weather_content(&mut content, weather, weather_error, language, "temp-f")
        }
        "weather-rain" => weather_content(&mut content, weather, weather_error, language, "rain"),
        "custom-countdown" | "moon-custom-countdown" => {
            if config.bottom.mode.starts_with("moon-") {
                content.icon = Some(Icon::Moon(phase));
            }
            if let Some(item) = countdown_at(&config.countdowns, now.date_naive(), 0) {
                content.heading = format!(
                    "{}：",
                    if item.name.is_empty() {
                        if language == "zh" { "期限" } else { "Limit" }
                    } else {
                        &item.name
                    }
                );
                let days = days_left(item, now.date_naive()).unwrap_or_default();
                content.number = if days == 0 {
                    if language == "zh" { "今天" } else { "Today" }.into()
                } else {
                    days.to_string()
                };
                if content.icon.is_some() && days != 0 {
                    content.unit = "/".into();
                }
            } else {
                content.heading = if language == "zh" { "无：" } else { "None:" }.into();
                content.number = "--".into();
            }
        }
        _ => {}
    }

    if config.bottom.show_secondary_countdowns && config.bottom.mode != "none" {
        let start = usize::from(config.bottom.mode.contains("custom-countdown"));
        content.secondary = (start..start + 2)
            .filter_map(|index| countdown_at(&config.countdowns, now.date_naive(), index))
            .map(|item| {
                let name = if item.name.is_empty() {
                    if language == "zh" { "期限" } else { "Limit" }
                } else {
                    &item.name
                };
                format!(
                    "{name}: {}",
                    days_left(item, now.date_naive()).unwrap_or_default()
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
    }
    content.warning = weather.is_some() && weather_error.is_some();
    content
}

fn weather_content(
    content: &mut ClockContent,
    weather: Option<&WeatherSnapshot>,
    error: Option<WeatherErrorKind>,
    language: &str,
    mode: &str,
) {
    content.icon = weather
        .map(|snapshot| Icon::Weather(snapshot.icon()))
        .or_else(|| error.map(|_| Icon::Warning));
    if let Some(kind) = error {
        content.caption = weather_error_label(kind, language).into();
        return;
    }
    let Some(weather) = weather else { return };
    match mode {
        "weather" => content.caption = weather.label(language),
        "rain" => {
            content.heading = if language == "zh" {
                "降雨："
            } else {
                "Rain:"
            }
            .into();
            content.number = weather
                .rain_chance
                .map_or_else(|| "--".into(), |value| value.to_string());
            if weather.rain_chance.is_some() {
                content.unit = "%".into();
            }
        }
        "temp-c" => {
            content.number = number(weather.temp_c);
            content.unit = "°C".into();
        }
        "temp-f" => {
            content.number = number(weather.temp_f);
            content.unit = "°F".into();
        }
        _ => {}
    }
}

fn resolve_language(configured: &str) -> &'static str {
    if configured == "zh" {
        return "zh";
    }
    if configured == "en" {
        return "en";
    }
    if sys_locale::get_locale()
        .unwrap_or_default()
        .to_lowercase()
        .starts_with("zh")
    {
        "zh"
    } else {
        "en"
    }
}

pub fn phase_of_day(hour: u32, language: &str) -> &'static str {
    let values = if language == "zh" {
        [
            (5, "深夜"),
            (7, "清晨"),
            (10, "早晨"),
            (15, "白天"),
            (19, "下午"),
            (24, "夜晚"),
        ]
    } else {
        [
            (5, "Late Night"),
            (7, "Early Morning"),
            (10, "Morning"),
            (15, "Daytime"),
            (19, "Afternoon"),
            (24, "Evening"),
        ]
    };
    values
        .into_iter()
        .find(|(upper, _)| hour < *upper)
        .map(|(_, label)| label)
        .unwrap_or("Evening")
}

fn countdown_at(
    items: &[CountdownConfig],
    today: NaiveDate,
    index: usize,
) -> Option<&CountdownConfig> {
    items
        .iter()
        .filter(|item| item.enabled)
        .filter(|item| days_left(item, today).is_some_and(|days| item.persistent || days >= 0))
        .nth(index)
}

fn format_datetime(moment: DateTime<FixedOffset>, pattern: &str) -> String {
    let items = StrftimeItems::new(pattern).collect::<Vec<_>>();
    if items.iter().any(|item| matches!(item, Item::Error)) {
        return "!! FORMAT !!".into();
    }
    let mut formatted = String::new();
    if write!(
        formatted,
        "{}",
        moment.format_with_items(items.iter().cloned())
    )
    .is_err()
    {
        return "!! FORMAT !!".into();
    }
    formatted
}

fn days_left(item: &CountdownConfig, today: NaiveDate) -> Option<i64> {
    NaiveDate::parse_from_str(&item.date, "%Y-%m-%d")
        .ok()
        .map(|date| (date - today).num_days())
}

fn number(value: f64) -> String {
    if value.fract().abs() < f64::EPSILON {
        format!("{value:.0}")
    } else {
        format!("{value:.1}")
    }
}

fn weekday(day: u32, language: &str) -> &'static str {
    const EN: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    const ZH: [&str; 7] = ["周一", "周二", "周三", "周四", "周五", "周六", "周日"];
    if language == "zh" {
        ZH[day as usize]
    } else {
        EN[day as usize]
    }
}

fn phase_name(phase: &str, language: &str) -> &'static str {
    match (language, phase) {
        ("zh", "new") => "新月",
        ("zh", "new-first") => "娥眉月",
        ("zh", "first") => "上弦月",
        ("zh", "first-full") => "盈凸月",
        ("zh", "full") => "满月",
        ("zh", "full-last") => "亏凸月",
        ("zh", "last") => "下弦月",
        ("zh", _) => "残月",
        (_, "new") => "New Moon",
        (_, "new-first") => "Waxing Crescent",
        (_, "first") => "First Quarter",
        (_, "first-full") => "Waxing Gibbous",
        (_, "full") => "Full Moon",
        (_, "full-last") => "Waning Gibbous",
        (_, "last") => "Last Quarter",
        (_, _) => "Waning Crescent",
    }
}

fn quarter_name(phase: &str, language: &str) -> &'static str {
    match (language, phase) {
        ("zh", "new") => "新月",
        ("zh", "first") => "上弦",
        ("zh", "full") => "满月",
        ("zh", _) => "下弦",
        (_, "new") => "New",
        (_, "full") => "Full",
        (_, _) => "Half",
    }
}

fn weather_error_label(kind: WeatherErrorKind, language: &str) -> &'static str {
    match (language, kind) {
        ("zh", WeatherErrorKind::NotConfigured) => "请配置天气密钥",
        ("zh", WeatherErrorKind::Auth) => "天气密钥无效",
        ("zh", WeatherErrorKind::Location) => "地点无效",
        ("zh", WeatherErrorKind::RateLimit) => "天气额度受限",
        ("zh", WeatherErrorKind::Network) => "网络不可用",
        ("zh", WeatherErrorKind::Service) => "天气服务错误",
        ("zh", WeatherErrorKind::Response) => "天气数据错误",
        (_, WeatherErrorKind::NotConfigured) => "Set weather key",
        (_, WeatherErrorKind::Auth) => "Invalid API key",
        (_, WeatherErrorKind::Location) => "Unknown place",
        (_, WeatherErrorKind::RateLimit) => "Weather limit",
        (_, WeatherErrorKind::Network) => "No network",
        (_, WeatherErrorKind::Service) => "Weather error",
        (_, WeatherErrorKind::Response) => "Weather error",
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    #[test]
    fn temperature_keeps_compact_complete_unit() {
        let mut config = AppConfig::default();
        config.bottom.mode = "weather-temp-c".into();
        config.clock.language = "zh".into();
        let weather = WeatherSnapshot {
            code: 0,
            temp_c: 26.0,
            temp_f: 78.8,
            rain_chance: None,
            fetched_at: 1.0,
            condition: "晴".into(),
            source: "amap:440300".into(),
        };
        let offset = FixedOffset::east_opt(8 * 3600).unwrap();
        let now = offset.with_ymd_and_hms(2026, 9, 1, 12, 0, 0).unwrap();
        let content = build_content(&config, now, Some(&weather), None);
        assert_eq!(content.number, "26");
        assert_eq!(content.unit, "°C");
        assert_eq!(content.icon, Some(Icon::Weather(WeatherIcon::Sun)));
    }

    #[test]
    fn phase_of_day_boundaries_match_python_version() {
        assert_eq!(phase_of_day(4, "en"), "Late Night");
        assert_eq!(phase_of_day(5, "en"), "Early Morning");
        assert_eq!(phase_of_day(19, "zh"), "夜晚");
    }

    #[test]
    fn invalid_time_formats_and_countdown_dates_do_not_panic() {
        let mut config = AppConfig::default();
        config.clock.date_format = "%Q".into();
        config.clock.language = "en".into();
        config.bottom.mode = "custom-countdown".into();
        config.countdowns.push(CountdownConfig {
            name: "Broken".into(),
            date: "not-a-date".into(),
            enabled: true,
            persistent: true,
        });
        let offset = FixedOffset::east_opt(8 * 3600).unwrap();
        let now = offset.with_ymd_and_hms(2026, 9, 1, 12, 0, 0).unwrap();
        let content = build_content(&config, now, None, None);
        assert_eq!(content.date_text, "!! FORMAT !!");
        assert_eq!(content.heading, "None:");
        assert_eq!(content.number, "--");
    }
}
