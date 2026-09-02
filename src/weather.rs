use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::config::WeatherConfig;

const AMAP_ENDPOINT: &str = "https://restapi.amap.com/v3/weather/weatherInfo";
const WEATHERAPI_ENDPOINT: &str = "https://api.weatherapi.com/v1/forecast.json";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WeatherIcon {
    Sun,
    PartlyCloudy,
    Cloud,
    Fog,
    Shower,
    Rain,
    Storm,
    Snow,
    Thermometer,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct WeatherSnapshot {
    pub code: i64,
    pub temp_c: f64,
    pub temp_f: f64,
    pub rain_chance: Option<i64>,
    pub fetched_at: f64,
    #[serde(default)]
    pub condition: String,
    #[serde(default)]
    pub source: String,
}

impl WeatherSnapshot {
    pub fn icon(&self) -> WeatherIcon {
        if !self.condition.is_empty() {
            return condition_icon(&self.condition);
        }
        code_icon(self.code)
    }

    pub fn label(&self, language: &str) -> String {
        if language == "zh" && !self.condition.is_empty() {
            return self.condition.clone();
        }
        match (language, self.icon()) {
            ("zh", WeatherIcon::Sun) => "晴",
            ("zh", WeatherIcon::PartlyCloudy) => "多云",
            ("zh", WeatherIcon::Cloud) => "阴",
            ("zh", WeatherIcon::Fog) => "雾",
            ("zh", WeatherIcon::Shower) => "阵雨",
            ("zh", WeatherIcon::Rain) => "雨",
            ("zh", WeatherIcon::Storm) => "雷雨",
            ("zh", WeatherIcon::Snow) => "冰雪",
            ("zh", WeatherIcon::Thermometer) => "天气",
            (_, WeatherIcon::Sun) => "Clear",
            (_, WeatherIcon::PartlyCloudy | WeatherIcon::Cloud) => "Cloudy",
            (_, WeatherIcon::Fog) => "Fog",
            (_, WeatherIcon::Shower | WeatherIcon::Rain) => "Rain",
            (_, WeatherIcon::Storm) => "Storm",
            (_, WeatherIcon::Snow) => "Cold precip.",
            (_, WeatherIcon::Thermometer) => "Weather",
        }
        .into()
    }

    pub fn source_matches(&self, config: &WeatherConfig) -> bool {
        self.source == source_id(config)
    }

    pub fn is_due(&self, config: &WeatherConfig, now: f64) -> bool {
        now - self.fetched_at >= (config.update_minutes * 60) as f64
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WeatherErrorKind {
    NotConfigured,
    Auth,
    Location,
    RateLimit,
    Network,
    Service,
    Response,
}

#[derive(Clone, Debug, Error)]
#[error("{kind:?}: {detail}")]
pub struct WeatherError {
    pub kind: WeatherErrorKind,
    detail: String,
}

impl WeatherError {
    fn new(kind: WeatherErrorKind, detail: impl Into<String>) -> Self {
        Self {
            kind,
            detail: detail.into(),
        }
    }
}

pub async fn fetch(config: WeatherConfig) -> Result<WeatherSnapshot, WeatherError> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent("Moonlight-Clock-COSMIC/0.4")
        .build()
        .map_err(map_request_error)?;
    let source = source_id(&config);
    let fetched_at = unix_time();

    if config.provider == "amap" {
        let response = client
            .get(AMAP_ENDPOINT)
            .query(&[
                ("key", config.api_key.as_str()),
                ("city", config.query.as_str()),
                ("extensions", "base"),
                ("output", "JSON"),
            ])
            .send()
            .await
            .map_err(map_request_error)?;
        let payload = response_json(response).await?;
        parse_amap(&payload, fetched_at, source)
    } else {
        let response = client
            .get(WEATHERAPI_ENDPOINT)
            .query(&[
                ("key", config.api_key.as_str()),
                ("q", config.query.as_str()),
                ("days", "1"),
                ("aqi", "no"),
                ("alerts", "no"),
            ])
            .send()
            .await
            .map_err(map_request_error)?;
        let payload = response_json(response).await?;
        parse_weatherapi(&payload, fetched_at, source)
    }
}

pub async fn fetch_and_cache(
    config: WeatherConfig,
    cache_file: &Path,
) -> Result<WeatherSnapshot, WeatherError> {
    let snapshot = fetch(config).await?;
    write_cache(cache_file, &snapshot).await?;
    Ok(snapshot)
}

async fn response_json(response: reqwest::Response) -> Result<Value, WeatherError> {
    let status = response.status();
    if !status.is_success() {
        let kind = match status {
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => WeatherErrorKind::Auth,
            StatusCode::BAD_REQUEST => WeatherErrorKind::Location,
            StatusCode::TOO_MANY_REQUESTS => WeatherErrorKind::RateLimit,
            _ => WeatherErrorKind::Service,
        };
        return Err(WeatherError::new(kind, format!("HTTP {status}")));
    }
    response
        .json::<Value>()
        .await
        .map_err(|error| WeatherError::new(WeatherErrorKind::Response, error.to_string()))
}

pub fn parse_amap(
    payload: &Value,
    fetched_at: f64,
    source: String,
) -> Result<WeatherSnapshot, WeatherError> {
    if payload.get("status").and_then(Value::as_str) != Some("1") {
        let info_code = payload
            .get("infocode")
            .and_then(Value::as_str)
            .unwrap_or("");
        let detail = payload
            .get("info")
            .and_then(Value::as_str)
            .unwrap_or("AMap request failed");
        return Err(WeatherError::new(amap_error_kind(info_code), detail));
    }
    let live = payload
        .get("lives")
        .and_then(Value::as_array)
        .and_then(|lives| lives.first())
        .ok_or_else(|| {
            WeatherError::new(WeatherErrorKind::Location, "AMap returned no live weather")
        })?;
    let temp_c = value_f64(live.get("temperature"), "temperature")?;
    Ok(WeatherSnapshot {
        code: 0,
        temp_c,
        temp_f: temp_c * 9.0 / 5.0 + 32.0,
        rain_chance: None,
        fetched_at,
        condition: live
            .get("weather")
            .and_then(Value::as_str)
            .unwrap_or("")
            .into(),
        source,
    })
}

pub fn parse_weatherapi(
    payload: &Value,
    fetched_at: f64,
    source: String,
) -> Result<WeatherSnapshot, WeatherError> {
    let current = payload
        .get("current")
        .ok_or_else(|| WeatherError::new(WeatherErrorKind::Response, "missing current weather"))?;
    let day = payload
        .pointer("/forecast/forecastday/0/day")
        .ok_or_else(|| WeatherError::new(WeatherErrorKind::Response, "missing forecast day"))?;
    Ok(WeatherSnapshot {
        code: value_i64(current.pointer("/condition/code"), "condition code")?,
        temp_c: value_f64(current.get("temp_c"), "temp_c")?,
        temp_f: value_f64(current.get("temp_f"), "temp_f")?,
        rain_chance: Some(value_i64(day.get("daily_chance_of_rain"), "rain chance")?),
        fetched_at,
        condition: String::new(),
        source,
    })
}

pub fn read_cache(path: &Path) -> Option<WeatherSnapshot> {
    let source = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&source).ok()
}

async fn write_cache(path: &Path, snapshot: &WeatherSnapshot) -> Result<(), WeatherError> {
    let parent = path
        .parent()
        .ok_or_else(|| WeatherError::new(WeatherErrorKind::Response, "cache path has no parent"))?;
    tokio::fs::create_dir_all(parent)
        .await
        .map_err(|error| WeatherError::new(WeatherErrorKind::Response, error.to_string()))?;
    let temporary = path.with_extension("tmp");
    let encoded = serde_json::to_vec(snapshot)
        .map_err(|error| WeatherError::new(WeatherErrorKind::Response, error.to_string()))?;
    tokio::fs::write(&temporary, encoded)
        .await
        .map_err(|error| WeatherError::new(WeatherErrorKind::Response, error.to_string()))?;
    tokio::fs::rename(&temporary, path)
        .await
        .map_err(|error| WeatherError::new(WeatherErrorKind::Response, error.to_string()))
}

pub fn source_id(config: &WeatherConfig) -> String {
    format!("{}:{}", config.provider, config.query.trim().to_lowercase())
}

pub fn unix_time() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

fn value_f64(value: Option<&Value>, name: &str) -> Result<f64, WeatherError> {
    value
        .and_then(|value| value.as_f64().or_else(|| value.as_str()?.parse().ok()))
        .ok_or_else(|| WeatherError::new(WeatherErrorKind::Response, format!("invalid {name}")))
}

fn value_i64(value: Option<&Value>, name: &str) -> Result<i64, WeatherError> {
    value
        .and_then(|value| value.as_i64().or_else(|| value.as_str()?.parse().ok()))
        .ok_or_else(|| WeatherError::new(WeatherErrorKind::Response, format!("invalid {name}")))
}

fn map_request_error(error: reqwest::Error) -> WeatherError {
    WeatherError::new(
        if error.is_connect() || error.is_timeout() {
            WeatherErrorKind::Network
        } else {
            WeatherErrorKind::Service
        },
        error.to_string(),
    )
}

fn amap_error_kind(code: &str) -> WeatherErrorKind {
    match code {
        "10001" | "10005" | "10006" | "10007" | "10008" | "10009" | "10012" | "10013" => {
            WeatherErrorKind::Auth
        }
        "10003" | "10004" | "10010" | "10014" | "10019" | "10020" | "10021" => {
            WeatherErrorKind::RateLimit
        }
        "10002" | "10015" | "10016" | "10017" => WeatherErrorKind::Service,
        _ => WeatherErrorKind::Response,
    }
}

fn code_icon(code: i64) -> WeatherIcon {
    match code {
        1000 => WeatherIcon::Sun,
        1003 => WeatherIcon::PartlyCloudy,
        1006 | 1009 => WeatherIcon::Cloud,
        1030 | 1135 | 1147 => WeatherIcon::Fog,
        1063 | 1072 | 1150 | 1153 | 1168 | 1171 | 1180 | 1186 | 1192 => WeatherIcon::Shower,
        1087 | 1273 | 1276 => WeatherIcon::Storm,
        1066 | 1069 | 1114 | 1117 | 1204 | 1207 | 1210 | 1213 | 1216 | 1219 | 1222 | 1225
        | 1237 | 1249 | 1252 | 1255 | 1258 | 1261 | 1264 | 1279 | 1282 => WeatherIcon::Snow,
        1183 | 1189 | 1195 | 1198 | 1201 | 1240 | 1243 | 1246 => WeatherIcon::Rain,
        _ => WeatherIcon::Thermometer,
    }
}

fn condition_icon(condition: &str) -> WeatherIcon {
    if ["雷", "电"].iter().any(|token| condition.contains(token)) {
        WeatherIcon::Storm
    } else if ["雪", "冻雨", "冰雹", "冰粒"]
        .iter()
        .any(|token| condition.contains(token))
    {
        WeatherIcon::Snow
    } else if condition.contains('雨') {
        WeatherIcon::Rain
    } else if ["雾", "霾", "沙", "尘"]
        .iter()
        .any(|token| condition.contains(token))
    {
        WeatherIcon::Fog
    } else if condition.contains('阴') {
        WeatherIcon::Cloud
    } else if condition.contains('云') {
        WeatherIcon::PartlyCloudy
    } else if condition.contains('晴') {
        WeatherIcon::Sun
    } else {
        WeatherIcon::Thermometer
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_amap_without_fake_rain_probability() {
        let snapshot = parse_amap(
            &json!({"status":"1","lives":[{"weather":"小雨","temperature":"26"}]}),
            123.0,
            "amap:440300".into(),
        )
        .unwrap();
        assert_eq!(snapshot.icon(), WeatherIcon::Rain);
        assert_eq!(snapshot.label("zh"), "小雨");
        assert_eq!(snapshot.temp_c, 26.0);
        assert!((snapshot.temp_f - 78.8).abs() < 0.001);
        assert_eq!(snapshot.rain_chance, None);
    }

    #[test]
    fn maps_amap_quota_error() {
        let error = parse_amap(
            &json!({"status":"0","info":"DAILY_QUERY_OVER_LIMIT","infocode":"10003"}),
            123.0,
            String::new(),
        )
        .unwrap_err();
        assert_eq!(error.kind, WeatherErrorKind::RateLimit);
    }

    #[test]
    fn reads_python_compatible_cache() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("weather.json");
        std::fs::write(
            &path,
            r#"{"code":1000,"temp_c":22.5,"temp_f":72.5,"rain_chance":10,"fetched_at":123}"#,
        )
        .unwrap();
        let snapshot = read_cache(&path).unwrap();
        assert_eq!(snapshot.icon(), WeatherIcon::Sun);
        assert_eq!(snapshot.temp_c, 22.5);
    }
}
