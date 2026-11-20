//! Shared application state used by the background tasks and the web handlers.

use crate::modules::config::Config;
use crate::modules::gpio::SharedHardware;
use crate::modules::ledStrip::LedController;
use crate::modules::lightControl::LightController;
use crate::modules::models::Readings;
use sqlx::SqlitePool;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{Mutex, RwLock};

/// Cheap to clone; all fields are shared handles.
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub pool: SqlitePool,
    pub hw: SharedHardware,
    pub light: Arc<Mutex<LightController>>,
    pub led: Arc<Mutex<LedController>>,
    /// Latest sensor values (None until the first measurement)
    pub readings: Arc<RwLock<Option<Readings>>>,
    pub started: Instant,
}
