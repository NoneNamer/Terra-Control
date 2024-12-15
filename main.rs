mod modules;
use tokio::task;
use std::sync::{Arc, Mutex};
use modules::config::Config;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {

    // Load the configuration from the config.toml file
    let config = Config::load()?;
    println!("{:?}", config); // Debugging: Print the loaded config

    // Start the web server
    let web_handle = {
        let state = state.clone();
        task::spawn(async move {
            modules::web::start_server(state, config.clone()).await;
        })
    };

    // Wait for tasks to finish
    let _ = tokio::join!(config, web_handle);
}

