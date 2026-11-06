//! Weather service for fetching current weather via IP geolocation or custom location.
//! Uses free APIs (ip-api.com / freeipapi.com and open-meteo.com) with no API key required.

use std::sync::{Arc, RwLock};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::Duration;

use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WeatherCondition {
    Clear,
    PartlyCloudy,
    Overcast,
    Fog,
    Drizzle,
    Rain,
    Snow,
    Thunderstorm,
}

impl WeatherCondition {
    pub fn from_wmo_code(code: u8) -> Self {
        match code {
            0 => Self::Clear,
            1 | 2 => Self::PartlyCloudy,
            3 => Self::Overcast,
            45 | 48 => Self::Fog,
            51..=57 => Self::Drizzle,
            61..=67 | 80..=82 => Self::Rain,
            71..=77 | 85 | 86 => Self::Snow,
            95 | 96 | 99 => Self::Thunderstorm,
            _ => Self::PartlyCloudy,
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::Clear => "Clear sky",
            Self::PartlyCloudy => "Partly cloudy",
            Self::Overcast => "Overcast",
            Self::Fog => "Foggy",
            Self::Drizzle => "Drizzle",
            Self::Rain => "Rain",
            Self::Snow => "Snow",
            Self::Thunderstorm => "Thunderstorm",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct WeatherInfo {
    pub temperature: f32,
    pub condition: WeatherCondition,
    pub location_name: String,
}

pub struct WeatherService {
    data: Arc<RwLock<Option<WeatherInfo>>>,
    trigger_tx: Sender<String>,
}

impl WeatherService {
    pub fn start(initial_location: &str, ctx: egui::Context) -> Self {
        let data = Arc::new(RwLock::new(None));
        let (trigger_tx, trigger_rx) = channel::<String>();

        let thread_data = Arc::clone(&data);
        let init_loc = initial_location.trim().to_string();

        std::thread::Builder::new()
            .name("weather-service".into())
            .spawn(move || {
                run_weather_loop(init_loc, trigger_rx, thread_data, ctx);
            })
            .expect("Failed to spawn weather-service thread");

        Self { data, trigger_tx }
    }

    pub fn current(&self) -> Option<WeatherInfo> {
        self.data.read().ok().and_then(|guard| guard.clone())
    }

    pub fn update_location(&self, location: &str) {
        let _ = self.trigger_tx.send(location.trim().to_string());
    }
}

fn run_weather_loop(
    mut location: String,
    trigger_rx: Receiver<String>,
    data: Arc<RwLock<Option<WeatherInfo>>>,
    ctx: egui::Context,
) {
    const REFRESH_INTERVAL: Duration = Duration::from_secs(30 * 60); // 30 minutes
    const RETRY_INTERVAL: Duration = Duration::from_secs(60); // 1 minute on failure

    loop {
        match fetch_weather(&location) {
            Ok(info) => {
                if let Ok(mut guard) = data.write() {
                    *guard = Some(info);
                }
                ctx.request_repaint();

                // Wait for either a location change trigger or interval timeout
                match trigger_rx.recv_timeout(REFRESH_INTERVAL) {
                    Ok(new_loc) => {
                        location = new_loc;
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                        // Periodic refresh
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                        break;
                    }
                }
            }
            Err(err) => {
                eprintln!("[Weather] Fetch error: {err}");
                // Wait briefly before retrying, or wake up on location change
                match trigger_rx.recv_timeout(RETRY_INTERVAL) {
                    Ok(new_loc) => {
                        location = new_loc;
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                        break;
                    }
                }
            }
        }
    }
}

pub fn fetch_weather(location_override: &str) -> Result<WeatherInfo, String> {
    let (lat, lon, location_name) = resolve_coordinates(location_override)?;

    let url = format!(
        "https://api.open-meteo.com/v1/forecast?latitude={lat}&longitude={lon}&current=temperature_2m,weather_code"
    );

    let body = http_get(&url)?;
    parse_weather_response(&body, location_name)
}

fn resolve_coordinates(location: &str) -> Result<(f64, f64, String), String> {
    let loc = location.trim();
    if loc.is_empty() || loc.eq_ignore_ascii_case("auto") {
        // Try IP geolocation: ip-api.com then freeipapi.com
        if let Ok(res) = fetch_ip_location_ip_api() {
            return Ok(res);
        }
        fetch_ip_location_freeipapi()
    } else {
        // Use Open-Meteo Geocoding
        geocode_city(loc)
    }
}

fn fetch_ip_location_ip_api() -> Result<(f64, f64, String), String> {
    let body = http_get("http://ip-api.com/json/")?;
    let v: Value = serde_json::from_str(&body).map_err(|e| e.to_string())?;

    if v["status"].as_str() != Some("success") {
        return Err("ip-api returned non-success".to_string());
    }

    let lat = v["lat"].as_f64().ok_or("missing lat")?;
    let lon = v["lon"].as_f64().ok_or("missing lon")?;
    let city = v["city"].as_str().unwrap_or("Local").to_string();

    Ok((lat, lon, city))
}

fn fetch_ip_location_freeipapi() -> Result<(f64, f64, String), String> {
    let body = http_get("https://freeipapi.com/api/json")?;
    let v: Value = serde_json::from_str(&body).map_err(|e| e.to_string())?;

    let lat = v["latitude"].as_f64().ok_or("missing latitude")?;
    let lon = v["longitude"].as_f64().ok_or("missing longitude")?;
    let city = v["cityName"].as_str().unwrap_or("Local").to_string();

    Ok((lat, lon, city))
}

fn geocode_city(city: &str) -> Result<(f64, f64, String), String> {
    let encoded = url_encode(city);
    let url = format!("https://geocoding-api.open-meteo.com/v1/search?name={encoded}&count=1");
    let body = http_get(&url)?;
    let v: Value = serde_json::from_str(&body).map_err(|e| e.to_string())?;

    let results = v["results"]
        .as_array()
        .ok_or_else(|| format!("Location not found: {city}"))?;

    let first = results
        .first()
        .ok_or_else(|| format!("Location not found: {city}"))?;

    let lat = first["latitude"].as_f64().ok_or("missing latitude")?;
    let lon = first["longitude"].as_f64().ok_or("missing longitude")?;
    let name = first["name"].as_str().unwrap_or(city).to_string();

    Ok((lat, lon, name))
}

fn parse_weather_response(json: &str, location_name: String) -> Result<WeatherInfo, String> {
    let v: Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let current = &v["current"];

    let temp = current["temperature_2m"]
        .as_f64()
        .ok_or("missing temperature_2m")? as f32;
    let code = current["weather_code"]
        .as_u64()
        .ok_or("missing weather_code")? as u8;

    Ok(WeatherInfo {
        temperature: temp,
        condition: WeatherCondition::from_wmo_code(code),
        location_name,
    })
}

fn http_get(url: &str) -> Result<String, String> {
    let mut response = ureq::get(url)
        .call()
        .map_err(|e| format!("HTTP GET error: {e}"))?;

    let mut reader = response.body_mut().as_reader();
    let mut body = String::new();
    std::io::Read::read_to_string(&mut reader, &mut body)
        .map_err(|e| format!("Read body error: {e}"))?;

    Ok(body)
}

pub fn url_encode(input: &str) -> String {
    let mut encoded = String::new();
    for byte in input.bytes() {
        match byte {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            b' ' => encoded.push_str("%20"),
            _ => {
                encoded.push_str(&format!("%{:02X}", byte));
            }
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_url_encode() {
        assert_eq!(url_encode("Seoul"), "Seoul");
        assert_eq!(url_encode("New York"), "New%20York");
        assert_eq!(url_encode("서울"), "%EC%84%9C%EC%9A%B8");
    }

    #[test]
    fn test_weather_condition_mapping() {
        assert_eq!(WeatherCondition::from_wmo_code(0), WeatherCondition::Clear);
        assert_eq!(WeatherCondition::from_wmo_code(1), WeatherCondition::PartlyCloudy);
        assert_eq!(WeatherCondition::from_wmo_code(3), WeatherCondition::Overcast);
        assert_eq!(WeatherCondition::from_wmo_code(45), WeatherCondition::Fog);
        assert_eq!(WeatherCondition::from_wmo_code(61), WeatherCondition::Rain);
        assert_eq!(WeatherCondition::from_wmo_code(71), WeatherCondition::Snow);
        assert_eq!(WeatherCondition::from_wmo_code(95), WeatherCondition::Thunderstorm);
    }

    #[test]
    fn test_parse_weather_response() {
        let json = r#"{
            "current": {
                "temperature_2m": 18.5,
                "weather_code": 2
            }
        }"#;

        let info = parse_weather_response(json, "Seoul".to_string()).unwrap();
        assert_eq!(info.temperature, 18.5);
        assert_eq!(info.condition, WeatherCondition::PartlyCloudy);
        assert_eq!(info.location_name, "Seoul");
        assert_eq!(info.condition.description(), "Partly cloudy");
    }
}

