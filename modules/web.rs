use warp::Filter;
use std::sync::{Arc, Mutex};

use crate::control::State;
use crate::config::Config;

pub async fn start_server(state: Arc<Mutex<State>>, config: Config) {
    let state_filter = warp::any().map(move || state.clone());

    // Endpoints
    let get_data = warp::path("get_data")
        .and(state_filter.clone())
        .map(|state: Arc<Mutex<State>>| {
            let state = state.lock().unwrap();
            warp::reply::json(&*state)
        });

    let update_config = warp::path("update_config")
        .and(warp::body::json())
        .map(move |new_config: Config| {
            let _ = crate::config::save_config(&new_config);
            warp::reply::with_status("Updated config", warp::http::StatusCode::OK)
        });

    warp::serve(get_data.or(update_config))
        .run(([0, 0, 0, 0], 8080))
        .await;
}