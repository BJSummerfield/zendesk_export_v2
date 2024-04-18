use std::{env, error::Error};
use tokio::sync::broadcast;

mod events;
mod models;
mod utils;

use events::EventType;
use models::{
    app_state::AppState,
    categories::Categories,
    fetcher::{Fetcher, FetcherConfig},
    file_writer::FileWriter,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Setup channel communications
    let (tx, _) = broadcast::channel::<EventType>(100);
    //
    // Configuration from environment variables
    let config = FetcherConfig {
        email: env::var("ZENDESK_EMAIL")?,
        password: env::var("ZENDESK_PASSWORD")?,
        base_url: "https://nttsh.zendesk.com".to_string(),
        language: "en-001".to_string(),
    };

    let mut app_state = AppState::new(tx.clone(), tx.subscribe());
    let mut fetcher = Fetcher::new(config, tx.clone(), tx.subscribe());
    let mut file_writer = FileWriter::new(tx.clone(), tx.subscribe());
    let mut categories = Categories::new(tx.clone(), tx.subscribe());

    let state_handle = tokio::spawn(async move {
        app_state.monitor_state().await;
    });

    let fetcher_handle = tokio::spawn(async move {
        fetcher.run().await;
    });

    let file_writer_handle = tokio::spawn(async move {
        file_writer.run().await;
    });

    let categories_handle = tokio::spawn(async move {
        categories.run().await;
    });

    let _ = tokio::try_join!(
        state_handle,
        fetcher_handle,
        categories_handle,
        file_writer_handle
    )?;

    Ok(())
}
