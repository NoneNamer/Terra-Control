//! Sensor data collection: read sensors (via `gpio`), update shared state, feed the
//! overheat protection, log threshold warnings and store the values.

use crate::modules::config::{SensorConfig, Thresholds};
use crate::modules::gpio::{Relay, SharedHardware};
use crate::modules::lightControl;
use crate::modules::logs::{self, Level};
use crate::modules::models::Readings;
use crate::modules::state::AppState;
use crate::modules::storage::{self, RelaySnapshot};
use anyhow::{Context, Result};
use chrono::Local;
use std::collections::HashSet;
use std::time::{Duration, Instant};

/// Calls `read` up to `retries` times with a short pause in between.
fn with_retries<T>(retries: u8, mut read: impl FnMut() -> Result<T>) -> Result<T> {
    let mut last_err = None;
    for attempt in 1..=retries.max(1) {
        match read() {
            Ok(v) => return Ok(v),
            Err(e) => {
                last_err = Some(e);
                if attempt < retries {
                    std::thread::sleep(Duration::from_millis(500));
                }
            }
        }
    }
    Err(last_err.expect("at least one attempt"))
}

/// Reads all sensors (blocking). Failed sensors are reported in the returned error list.
fn read_all(hw: &SharedHardware, cfg: &SensorConfig) -> (Readings, Vec<String>) {
    let mut errors = Vec::new();
    let mut onewire = |name: &str, id: &str| {
        // Lock per read so relay writes are not blocked for the whole cycle
        match with_retries(cfg.retries, || hw.lock().read_onewire(id)) {
            Ok(t) => Some(t),
            Err(e) => {
                errors.push(format!("{name} sensor ({id}): {e:#}"));
                None
            }
        }
    };
    let basking_temp = onewire("basking", &cfg.basking_id);
    let cool_temp = onewire("cool", &cfg.cool_id);
    let enclosure_temp = onewire("enclosure", &cfg.enclosure_id);
    let climate = match with_retries(cfg.retries, || hw.lock().read_climate()) {
        Ok(c) => Some(c),
        Err(e) => {
            errors.push(format!("SHT3x: {e:#}"));
            None
        }
    };
    let readings = Readings {
        timestamp: Local::now().naive_local(),
        basking_temp,
        cool_temp,
        enclosure_temp,
        air_temp: climate.map(|c| c.temperature),
        humidity: climate.map(|c| c.humidity),
    };
    (readings, errors)
}

/// Threshold checks: returns (key, message) for every value outside its range.
fn threshold_violations(r: &Readings, t: &Thresholds) -> Vec<(&'static str, String)> {
    let mut out = Vec::new();
    let mut check = |key, value: Option<f32>, bad: bool, msg: String| {
        if value.is_some() && bad {
            out.push((key, msg));
        }
    };
    let v = |x: Option<f32>| x.unwrap_or(f32::NAN);
    check("basking_max", r.basking_temp, v(r.basking_temp) > t.basking_max,
        format!("Basking spot too hot: {:.1} °C (> {:.1})", v(r.basking_temp), t.basking_max));
    check("cool_min", r.cool_temp, v(r.cool_temp) < t.cool_min,
        format!("Cool side too cold: {:.1} °C (< {:.1})", v(r.cool_temp), t.cool_min));
    check("cool_max", r.cool_temp, v(r.cool_temp) > t.cool_max,
        format!("Cool side too warm: {:.1} °C (> {:.1})", v(r.cool_temp), t.cool_max));
    check("humidity_min", r.humidity, v(r.humidity) < t.humidity_min,
        format!("Humidity too low: {:.1} % (< {:.1})", v(r.humidity), t.humidity_min));
    check("humidity_max", r.humidity, v(r.humidity) > t.humidity_max,
        format!("Humidity too high: {:.1} % (> {:.1})", v(r.humidity), t.humidity_max));
    check("enclosure_max", r.enclosure_temp, v(r.enclosure_temp) > t.enclosure_max,
        format!("Enclosure too hot: {:.1} °C (> {:.1})", v(r.enclosure_temp), t.enclosure_max));
    out
}

/// Remembers which warnings are active so each one is logged only when it starts and ends.
#[derive(Default)]
struct WarningState {
    active: HashSet<String>,
}

impl WarningState {
    /// Returns the log messages for warnings / sensor errors that started or ended.
    fn update(&mut self, current: Vec<(String, String)>) -> Vec<(Level, String)> {
        let mut out = Vec::new();
        let keys: HashSet<String> = current.iter().map(|(k, _)| k.clone()).collect();
        for (key, msg) in current {
            if !self.active.contains(&key) {
                out.push((Level::Warning, msg));
            }
        }
        for key in self.active.difference(&keys) {
            out.push((Level::Info, format!("Resolved: {key}")));
        }
        self.active = keys;
        out
    }
}

/// One measurement cycle.
async fn collect(state: &AppState, warnings: &mut WarningState) -> Result<()> {
    let hw = state.hw.clone();
    let cfg = state.config.sensors.clone();
    let (readings, errors) = tokio::task::spawn_blocking(move || read_all(&hw, &cfg))
        .await
        .context("sensor task panicked")?;

    *state.readings.write().await = Some(readings.clone());

    // Overheat protection first: switch the heat spot off immediately if needed
    let now = Instant::now();
    let event = state.light.lock().await.update_temperature(readings.basking_temp, now);
    if let Some((level, msg)) = event {
        logs::log(&state.pool, level, &msg).await;
    }
    lightControl::apply_and_log(state).await?;

    // Warnings and sensor errors (logged on change only)
    let mut current: Vec<(String, String)> = threshold_violations(&readings, &state.config.thresholds)
        .into_iter()
        .map(|(k, m)| (k.to_string(), m))
        .collect();
    for e in errors {
        let key = format!("sensor error: {}", e.split(':').next().unwrap_or("sensor"));
        current.push((key, format!("Sensor error: {e}")));
    }
    for (level, msg) in warnings.update(current) {
        logs::log(&state.pool, level, &msg).await;
    }

    let snapshot = {
        let overheat = state.light.lock().await.is_overheat(Instant::now());
        let hw = state.hw.lock();
        RelaySnapshot {
            uv1_on: hw.relay(Relay::Uv1),
            uv2_on: hw.relay(Relay::Uv2),
            heat_on: hw.relay(Relay::Heat),
            overheat,
        }
    };
    storage::insert_reading(&state.pool, &readings, snapshot).await
}

/// Background task: measures every `sensors.interval_secs`.
pub async fn run(state: AppState) {
    let mut interval = tokio::time::interval(Duration::from_secs(state.config.sensors.interval_secs));
    let mut warnings = WarningState::default();
    loop {
        interval.tick().await;
        if let Err(e) = collect(&state, &mut warnings).await {
            logs::log(&state.pool, Level::Error, &format!("Measurement failed: {e:#}")).await;
        }
    }
}
