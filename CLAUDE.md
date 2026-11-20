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

The code **does not compile yet** (phase 0 done, phase 1 next). It was written module-by-module and never
built as a whole. Known inconsistencies to resolve rather than work around:

- Three DB approaches are mixed: `storage.rs`/`web.rs` use `sqlx::SqlitePool`, `getData.rs` uses
  `PgPool`, `lightControl.rs`/`ledStrip.rs` use `rusqlite::Connection`. **Decision: sqlx + SQLite only.**
- SQL in `lightControl.rs`/`ledStrip.rs` queries `week_start`/`week_end` and `led_start`/`led_end`,
  which don't exist in the `schedule` table created in `storage.rs` (keyed by `week_number`).
- Sensor functions (`read_ds18b20`, …), `History`, `ConfigError`, `config.thresholds` don't exist.
- `web.rs` declares `mod handlers` twice and does not serve `static/` (that was only done by the
  removed camera server).
- `config.rs` field names differ from `config.toml` (`debug` vs `setup_mode`), and `Config::load`
  takes a path but `main.rs` calls it without one.
- `RelayController` (gpio.rs) and `LightController` both claim the same GPIO pins.
- GPIO numbers in `config.toml` do not match the fabricated PCB — see the pin table in `readme.md`.
- `static/api.js` calls `/api/current` and `/api/data/{date}`; the backend serves `/api/values`
  and `/api/graph/*`.

## Commands

```bash
cargo build                 # binary: terra-control (entry point main.rs, not src/)
cargo test                  # no tests yet
cargo test <test_name>      # single test
```

- The binary reads `config.toml` and creates `data.db`, `logs/`, `temp/` relative to the **working
  directory**, so run it from this folder.
- `sqlx::query!`/`query_as!` macros check SQL at compile time and need `DATABASE_URL=sqlite:data.db`
  (or offline data via `cargo sqlx prepare`).
- `rppal` only works on the Pi. The plan (phase 1) is a hardware trait with a simulation backend so
  the app builds and runs on Windows; the real build is cross-compiled for the Pi 3A+ in Docker
  (phase 5, Dockerfile not written yet; `Dockerfile` is gitignored). Target is 64-bit Pi OS:
  `aarch64-unknown-linux-gnu`. `rppal` is already a Linux-only dependency in `Cargo.toml`.

## Architecture

- **`main.rs`** builds shared state as `Arc<tokio::sync::Mutex<…>>` (relay, light and LED
  controllers, `CurrentReadings`) plus `Arc<Config>` and the DB pool, then spawns long-running
  tasks: sensor collection (`getData::start_data_collection`, interval from config), light update
  loop (30 s), LED update loop (30 s) and the Axum web server (axum 0.8, `axum::serve`).
  The camera code was deleted in phase 0.
- **Data flow:** sensors → `CurrentReadings` (shared, read by the web API) → basking temperature is
  pushed into `LightController::update_temperature` for overheat protection → readings are stored in SQLite.
- **Schedule model:** one row per ISO calendar week (KW 1–53) with UV1/UV2/heat on/off times and LED
  on/off times + base colour. Only the LED colour is interpolated *within a day*
  (morning → noon → evening presets in `[led]` of `config.toml`, mixed with the weekly colour via
  `season_weight`). Defaults come from `[db]` in `config.toml` when no week row exists.
- **Layering (decided):** all hardware access (relays, WS2805 strip, 1-Wire, I²C) lives in `gpio.rs`;
  `lightControl.rs`/`ledStrip.rs`/`getData.rs` contain logic only and call into it.
- **Web (`web.rs`):** `AppState` holds the shared handles; handlers are inline submodules
  (`handlers::{schedule, led, monitoring, system}`), routes are grouped per area under
  `/api/...`, and `static/` is served as plain HTML/JS (no build step).
- **Logging (`logs.rs`):** `logs::log` writes both to the `logs` DB table and to `logs/<date>.log`.

## Hardware constraints that affect code

- Loads: UV1 = metal-halide lamp on an HID ballast, UV2 = T5 tube, Heat = 230 V halogen spot.
  **UV1/UV2 need ≥ 15 min re-strike pause after switching off**, and this must survive restarts.
- Relays are active-high, 3 channels only (Q1/Q2/Q3 = GPIO25/24/23). The mapping to UV1/UV2/Heat
  is unconfirmed → keep it configurable (default Q1/Q2/Q3 = UV1/UV2/Heat). An LED-PSU relay will
  be retrofitted later → optional in config/code, pin still unknown.
- Each LED strip has DI + BI (backup data): drive GPIO18 (strip 1) and GPIO13 (strip 2);
  GPIO17/19 are the BI lines. GPIO4 (`Display_On`) and J7 (W5500 Ethernet, SPI0) are unused in v1.
- 1-Wire is on GPIO26 (DS18B20 ×2 in the enclosure + MAX31820 on the board, fixed IDs in config);
  I²C (GPIO2/3) carries the DS3231 RTC and the planned SHT3x. SPI0 is wired to connector J7, so the
  WS2805 LED data must use GPIO18/13 (PWM), not SPI.
- Out of scope for v1: camera, OLED display, UV sensors (VEML6075), DHT22, remote access/HTTPS.
- KiCad projects for the boards live in `../terrarium_pi` and `../terrarium_psu`.
