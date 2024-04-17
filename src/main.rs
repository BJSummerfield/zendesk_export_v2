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
        println!("Initializing Categories struct...");
        Categories {
            categories_hash: HashMap::new(),
            sender,
            receiver,
        }
    }

    pub async fn run(&mut self) {
        let initial_url = "categories.json".to_string();
        println!("Sending initial fetch request for URL: {}", initial_url);
        self.sender
            .send(FetchRequest { url: initial_url })
            .await
            .unwrap();

        println!("Listening for responses...");
        while let Some(response) = self.receiver.recv().await {
            println!("Received response: {:?}", response);
            match response {
                FetchResponse::Categories(res) => {
                    println!("Processing categories...");
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
                    println!("Categories processed and added.");
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

#[derive(Debug)]
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

#[derive(Clone, Debug)]
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
        println!("Initializing Fetcher with config: {:?}", config);
        Fetcher {
            client: Client::new(),
            rx,
            tx,
            config,
        }
    }

    pub async fn run(&mut self) {
        println!("Fetcher is running...");
        while let Some(request) = self.rx.recv().await {
            println!("Received fetch request for URL: {}", request.url);
            let client = self.client.clone();
            let tx = self.tx.clone();
            let url = request.url.clone();
            let config = self.config.clone();

            task::spawn(async move {
                println!("Spawning task to fetch data from URL: {}", url);
                let response = Fetcher::fetch_data(&client, &config, &url).await;
                match response {
                    Ok(data) => {
                        println!("Data fetched successfully, parsing...");
                        if let Ok(categories_response) =
                            serde_json::from_str::<CategoriesResponse>(&data)
                        {
                            println!("Data parsed successfully, sending response...");
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
        println!("Fetching data from endpoint: {}", url);
        let endpoint = format!(
            "{}/api/v2/help_center/{}/{}",
            config.base_url, config.language, url
        );

        let response = client
            .get(&endpoint)
            .basic_auth(&config.email, Some(&config.password))
            .send()
            .await?;

        if response.status().is_success() {
            let data = response.text().await?;
            println!("Data successfully retrieved from: {}", url);
            Ok(data)
        } else {
            let err = format!("HTTP Error: {}", response.status());
            println!("{}", err);
            Err(ReqwestError::from(response.error_for_status().unwrap_err()))
        }
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
    let mut fetcher = Fetcher::new(config.clone(), fetcher_rx, categories_tx.clone());
    let mut categories = Categories::new(fetcher_tx, categories_rx);

    println!("Starting Fetcher and Categories...");
    // Run Fetcher and Categories concurrently
    let fetcher_handle = tokio::spawn(async move {
        fetcher.run().await;
    });

    let categories_handle = tokio::spawn(async move {
        categories.run().await;
    });

    // Wait for both tasks to complete
    let _ = tokio::try_join!(fetcher_handle, categories_handle)?;

    println!("All tasks completed.");
    Ok(())
}
