mod modules;

use modules::config::Config;
use modules::web; // Include the web module
use std::error::Error;
use std::sync::Arc;
use tokio::task;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Load the configuration from the config.toml file
    let config = Config::load()?;
    println!("{:?}", config); // Debugging: Print the loaded config

    // Initialize the web server
    let web_handle = task::spawn(async {
        let router = create_router(&config).await; // Initialize the Axum router
        let addr: SocketAddr = format!("{}:{}", config.web.address, config.web.port)
            .parse()
            .expect("Invalid address");
        axum::Server::bind(&addr)
            .serve(router.into_make_service())
            .await
            .expect("Failed to start server");
    });

    // Wait for all tasks to finish
    tokio::try_join!(web_handle)?;

    Ok(())
}