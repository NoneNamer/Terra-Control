mod modules;

use tokio::task;
use std::sync::{Arc, Mutex};

#[tokio::main]
async fn main() {

    // Start the web server
    let web_handle = {
        let state = state.clone();
        task::spawn(async move {
            modules::web::start_server(state, config.clone()).await;
        })
    };

    // Wait for tasks to finish
    let _ = tokio::join!(web_handle, sensor_handle, logger_handle);
}