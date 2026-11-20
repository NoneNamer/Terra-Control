# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

The user communicates in German; reply in German. `readme.md` (German) is the source of truth for
project decisions, the board pinout, the Bearded-Dragon reference values and the phased roadmap —
read it before making design changes and keep its roadmap checkboxes up to date.

## Conventions

- Identifiers (variables, functions, types) and code comments are written in **English**, even
  though conversation and `readme.md` are German.
- When requirements, hardware details or design choices are unclear, **ask the user** instead of
  assuming — e.g. the open points listed at the end of `readme.md`.

## Current state

Phase 1 is done: the app builds without warnings and runs on Windows with simulated hardware
(web UI on port 80). Still open:

- `PiHardware` (Linux-only `cfg`) has never been compiled; WS2805 output and SHT3x are stubs (phase 2).
- `static/api.js`, `schedule.html`, `data.html` still use the old API shape (phase 4). The current
  API: `GET /api/values`, `/api/graph/{today,yesterday}`, `/api/data/download?start&end`,
  `GET /api/schedule`, `GET|PUT|DELETE /api/schedule/{week}`, `/api/schedule/current`,
  `/api/led/{status,power,color,natural,brightness,presets}`, `/api/system/status`, `/api/logs`,
  `/api/logs/download`, `POST /api/relays/{uv1|uv2|heat}` (setup mode only).
- UV re-strike pause is configured (`restrike_pause_secs`) but not enforced yet (phase 3).

## Commands

```bash
cargo build                 # binary: terra-control (entry point main.rs, not src/)
cargo test                  # so far: config.toml loads and validates
cargo test <test_name>      # single test
```

- The binary reads `config.toml` and creates `data.db`, `logs/`, `temp/` relative to the **working
  directory**, so run it from this folder.
- SQL uses runtime-checked `sqlx::query`/`query_as::<_, T>` (decided: no `query!` macros, so no
  `DATABASE_URL` is needed). Schema changes go into a new file in `migrations/`; `sqlx::migrate!`
  embeds them and they run on startup. Never edit an applied migration.
- Run on the PC: `cargo run`, then open http://localhost/. `config.toml` has `simulate = true`.
- `rppal` only works on the Pi; non-Linux builds always use `SimulatedHardware`. The real build is cross-compiled for the Pi 3A+ in Docker
  (phase 5, Dockerfile not written yet; `Dockerfile` is gitignored). Target is 64-bit Pi OS:
  `aarch64-unknown-linux-gnu`. `rppal` is already a Linux-only dependency in `Cargo.toml`.

## Architecture

- **`main.rs`** loads `config.toml`, opens the DB (runs migrations), creates the hardware backend,
  builds `state::AppState` (cloneable: config, pool, `SharedHardware`, `Mutex<LightController>`,
  `Mutex<LedController>`, `RwLock<Option<Readings>>`) and spawns `getData::run`,
  `lightControl::run`, `ledStrip::run` (30 s) plus the Axum server. On Ctrl+C/SIGTERM it switches
  all outputs off.
- **Hardware:** `gpio::Hardware` trait (blocking calls; use `spawn_blocking` for sensor reads),
  shared as `SharedHardware` (`Arc<std::sync::Mutex<Box<dyn Hardware>>>`). Never hold that lock
  across an `.await`. Lock order: tokio controller mutex first, then the hardware lock.
- **Data flow:** `getData` reads sensors → `AppState.readings` → `LightController::update_temperature`
  + immediate `apply` (overheat) → threshold warnings (logged on change) → `readings` table.
- **Heat safety:** `LightController::heat_block` keeps the heat spot off with no reading yet, on
  sensor failure / stale value (> 3 intervals), at/above `overheat_temp`, and during the cooldown.
  Relay changes go through `lightControl::apply_and_log`; web handlers call the same functions.
- **Schedule model:** one row per ISO calendar week (KW 1–53) with UV1/UV2/heat on/off times and LED
  on/off times + base colour. Only the LED colour is interpolated *within a day*
  (morning → noon → evening presets in `[led]` of `config.toml`, mixed with the weekly colour via
  `season_weight`). Defaults come from `[db]` in `config.toml` when no week row exists.
- **Layering (decided):** all hardware access (relays, WS2805 strip, 1-Wire, I²C) lives in `gpio.rs`;
  `lightControl.rs`/`ledStrip.rs`/`getData.rs` contain logic only and call into it.
- **Web (`web.rs`):** handlers are inline submodules (`handlers::{monitoring, schedule, led, system}`),
  each with its own `routes()`; errors via `ApiError` (`From<anyhow::Error>`). `static/` is the
  fallback service (plain HTML/JS, no build step).
- **Logging (`logs.rs`):** `logs::log(pool, Level, msg)` writes to stdout, `logs/<date>.log` and the
  `logs` table; it never fails the caller. Errors elsewhere use `anyhow`.

## Hardware constraints that affect code

- Loads: UV1 = metal-halide lamp on an HID ballast, UV2 = T5 tube, Heat = 230 V halogen spot.
  **UV1/UV2 need ≥ 15 min re-strike pause after switching off**, and this must survive restarts.
- Relays are active-high, 3 channels only (Q1/Q2/Q3 = GPIO25/24/23). The mapping to UV1/UV2/Heat
  is unconfirmed → configurable in `[relays]` (default Q1/Q2/Q3 = UV1/UV2/Heat). An LED-PSU relay
  will be retrofitted later → optional `relays.led_psu`, pin still unknown.
- Each LED strip has DI + BI (backup data): drive GPIO18 (strip 1) and GPIO13 (strip 2);
  GPIO17/19 are the BI lines. GPIO4 (`Display_On`) and J7 (W5500 Ethernet, SPI0) are unused in v1.
- 1-Wire is on GPIO26 (DS18B20 ×2 in the enclosure + MAX31820 on the board, fixed IDs in config);
  I²C (GPIO2/3) carries the DS3231 RTC and the planned SHT3x. SPI0 is wired to connector J7, so the
  WS2805 LED data must use GPIO18/13 (PWM), not SPI.
- Out of scope for v1: camera, OLED display, UV sensors (VEML6075), DHT22, remote access/HTTPS.
- KiCad projects for the boards live in `../terrarium_pi` and `../terrarium_psu`.
