mod modules;

use anyhow::{Context, Result};
use modules::config::{Config, CONFIG_PATH};
use modules::gpio::Relay;
use modules::ledStrip::LedController;
use modules::lightControl::LightController;
use modules::logs::{self, Level};
use modules::models::Rgbww;
use modules::state::AppState;
use modules::{getData, gpio, ledStrip, lightControl, storage, web};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock};

/// Entry point: loads the config, opens the database, creates the hardware backend and runs
/// the background tasks (measurement, lamps, LEDs) plus the web server until Ctrl+C / SIGTERM.
/// On shutdown all lamps and LEDs are switched off.
#[tokio::main]
async fn main() -> Result<()> {
    let config = Arc::new(Config::load(CONFIG_PATH)?);
    let pool = storage::connect(&config.storage.database).await?;

    let hw = gpio::create(&config)?;
    switch_everything_off(&hw);

    let led_settings = storage::load_led_settings(&pool, &config).await?;
    let led_presets = storage::load_led_presets(&pool, &config).await?;

    // Re-strike pause across restarts (see lightControl::restore_restrike)
    let mut light = LightController::new(&config.light_control, Duration::from_secs(config.sensors.interval_secs));
    let lamp_states = match storage::load_lamp_states(&pool).await {
        Ok(rows) => rows,
        Err(e) => {
            // Unknown history: treat both lamps as "was on" so the full pause applies
            eprintln!("Failed to load lamp states ({e:#}), applying the full re-strike pause");
            ["UV1", "UV2"]
                .map(|relay| storage::LampStateRow { relay: relay.into(), is_on: true, changed_at: chrono::Utc::now() })
                .to_vec()
        }
    };
    let restrike_messages = lightControl::restore_restrike(&mut light, &lamp_states, chrono::Utc::now(), Instant::now());
    // Lamps that were "on" are off now (all outputs were just switched off)
    for row in lamp_states.iter().filter(|r| r.is_on) {
        storage::save_lamp_state(&pool, &row.relay, false, chrono::Utc::now()).await?;
    }

    let state = AppState {
        light: Arc::new(Mutex::new(light)),
        led: Arc::new(Mutex::new(LedController::new(led_settings, led_presets))),
        readings: Arc::new(RwLock::new(None)),
        started: Instant::now(),
        hw: hw.clone(),
        pool: pool.clone(),
        config: config.clone(),
    };

    let backend = hw.lock().backend();
    logs::log(
        &pool,
        Level::Info,
        &format!(
            "Terrarium controller {} starting (hardware: {backend}, setup mode: {})",
            env!("CARGO_PKG_VERSION"),
            config.main.setup_mode
        ),
    )
    .await;
    for msg in restrike_messages {
        logs::log(&pool, Level::Info, &msg).await;
    }

    // Apply the schedule once before the tasks start (heat stays off until the first reading)
    if let Err(e) = lightControl::update_lights(&state).await {
        logs::log(&pool, Level::Error, &format!("Initial light update failed: {e:#}")).await;
    }
    if let Err(e) = ledStrip::update_leds(&state).await {
        logs::log(&pool, Level::Error, &format!("Initial LED update failed: {e:#}")).await;
    }

    let tasks = [
        tokio::spawn(getData::run(state.clone())),
        tokio::spawn(lightControl::run(state.clone())),
        tokio::spawn(ledStrip::run(state.clone())),
    ];

    let addr = format!("{}:{}", config.web.address, config.web.port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .with_context(|| format!("failed to bind web server to {addr}"))?;
    logs::log(&pool, Level::Info, &format!("Web interface on http://{addr}")).await;

    let result = axum::serve(listener, web::router(state.clone()))
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("web server failed");

    for task in tasks {
        task.abort();
    }
    // Wait until the lock is free so no task writes the relays after this point
    let _light = state.light.lock().await;
    let _led = state.led.lock().await;
    let uv_on: Vec<Relay> = {
        let hw = hw.lock();
        [Relay::Uv1, Relay::Uv2].into_iter().filter(|r| hw.relay(*r)).collect()
    };
    switch_everything_off(&hw);
    for relay in uv_on {
        if let Err(e) = storage::save_lamp_state(&pool, relay.name(), false, chrono::Utc::now()).await {
            eprintln!("Failed to store {} state: {e:#}", relay.name());
        }
    }
    logs::log(&pool, Level::Info, "Terrarium controller stopped, all outputs off").await;
    pool.close().await;
    result
}

fn switch_everything_off(hw: &gpio::SharedHardware) {
    let mut hw = hw.lock();
    for relay in [Relay::Uv1, Relay::Uv2, Relay::Heat] {
        if let Err(e) = hw.set_relay(relay, false) {
            eprintln!("Failed to switch {} off: {e:#}", relay.name());
        }
    }
    if let Err(e) = hw.write_leds(Rgbww::OFF) {
        eprintln!("Failed to switch LEDs off: {e:#}");
    }
    if let Err(e) = hw.set_relay(Relay::LedPsu, false) {
        eprintln!("Failed to switch LED PSU off: {e:#}");
    }
}

/// Resolves on Ctrl+C, and on Unix also on SIGTERM (sent by systemd).
async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut s) => {
                s.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
