use rayon::prelude::*;
use reqwest::{Client, Error as ReqwestError};
use serde::Deserialize;
use std::{collections::HashMap, env, error::Error};
use tokio::{sync::mpsc, task};

#[derive(Deserialize, Debug)]
struct Category {
    id: i64,
    name: String,
    url: String,
}

#[derive(Debug, Deserialize, Clone)]
struct CategoryDetail {
    name: String,
    url: String,
}

#[derive(Deserialize, Debug)]
struct CategoriesResponse {
    categories: Vec<Category>,
    next_page: Option<String>,
}

struct Categories {
    categories_hash: HashMap<i64, CategoryDetail>,
    sender: mpsc::Sender<FetchRequest>,
    receiver: mpsc::Receiver<FetchResponse>,
}

impl Categories {
    pub fn new(
        sender: mpsc::Sender<FetchRequest>,
        receiver: mpsc::Receiver<FetchResponse>,
    ) -> Self {
        Categories {
            categories_hash: HashMap::new(),
            sender,
            receiver,
        }
    }

    pub async fn run(&mut self) {
        let initial_url = "catagories.json".to_string();
        // Send the initial fetch request
        self.sender
            .send(FetchRequest {
                url: initial_url.to_string(),
            })
            .await
            .unwrap();

        // Listen for responses
        while let Some(response) = self.receiver.recv().await {
            match response {
                FetchResponse::Categories(res) => {
                    self.categories_hash
                        .par_extend(res.categories.into_par_iter().map(|cat| {
                            (
                                cat.id,
                                CategoryDetail {
                                    name: cat.name,
                                    url: cat.url,
                                },
                            )
                        }));
                }
                FetchResponse::FetchFailed { error } => {
                    eprintln!("Fetch failed: {}", error);
                }
            }
        }
    }
}

struct FetchRequest {
    url: String,
}

enum FetchResponse {
    Categories(CategoriesResponse),
    FetchFailed { error: String },
}

struct Fetcher {
    client: Client,
    rx: mpsc::Receiver<FetchRequest>,
    tx: mpsc::Sender<FetchResponse>,
    config: FetcherConfig,
}

#[derive(Clone)]
struct FetcherConfig {
    base_url: String,
    language: String,
    email: String,
    password: String,
}

impl Fetcher {
    pub fn new(
        config: FetcherConfig,
        rx: mpsc::Receiver<FetchRequest>,
        tx: mpsc::Sender<FetchResponse>,
    ) -> Self {
        Fetcher {
            client: Client::new(),
            rx,
            tx,
            config,
        }
    }

    pub async fn run(&mut self) {
        while let Some(request) = self.rx.recv().await {
            let client = self.client.clone();
            let tx = self.tx.clone();
            let url = request.url.clone();
            let config = self.config.clone();

            task::spawn(async move {
                let response = Fetcher::fetch_data(&client, &config, &url).await;
                match response {
                    Ok(data) => {
                        if let Ok(categories_response) =
                            serde_json::from_str::<CategoriesResponse>(&data)
                        {
                            if tx
                                .send(FetchResponse::Categories(categories_response))
                                .await
                                .is_err()
                            {
                                eprintln!("Failed to send categories response");
                            }
                        } else {
                            eprintln!("Response did not match expected CategoriesResponse");
                        }
                    }
                    Err(e) => {
                        let error_msg = format!("Failed to fetch data: {}", e);
                        if tx
                            .send(FetchResponse::FetchFailed { error: error_msg })
                            .await
                            .is_err()
                        {
                            eprintln!("Failed to send fetch failure response");
                        }
                    }
                }
            });
        }
    }

    async fn fetch_data(
        client: &Client,
        config: &FetcherConfig,
        url: &str,
    ) -> Result<String, ReqwestError> {
        let endpoint = format!(
            "{}/api/v2/help_center/{}/{}",
            config.base_url, config.language, url
        );

        let response = client
            .get(&endpoint)
            .basic_auth(&config.email, Some(&config.password))
            .send()
            .await?
            .json()
            .await?;

        Ok(response)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Setup channel communications
    let (fetcher_tx, fetcher_rx) = mpsc::channel(32);
    let (categories_tx, categories_rx) = mpsc::channel(32);

    // Configuration from environment variables
    let config = FetcherConfig {
        email: env::var("ZENDESK_EMAIL")?,
        password: env::var("ZENDESK_PASSWORD")?,
        base_url: "https://nttsh.zendesk.com".to_string(),
        language: "en-001".to_string(),
    };

    // Initialize Fetcher and Categories
    let mut fetcher = Fetcher::new(config, fetcher_rx, categories_tx.clone());
    let mut categories = Categories::new(fetcher_tx, categories_rx);

    // Run Fetcher and Categories concurrently
    let fetcher_handle = tokio::spawn(async move {
        fetcher.run().await;
    });

    let categories_handle = tokio::spawn(async move {
        categories.run().await;
    });

    // Wait for both tasks to complete
    let _ = tokio::try_join!(fetcher_handle, categories_handle)?;

    Ok(())
}
