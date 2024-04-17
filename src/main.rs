use rayon::prelude::*;
use reqwest::{Client, Error as ReqwestError};
use serde::Deserialize;
use std::{
    collections::HashMap,
    env,
    error::Error,
    sync::atomic::{AtomicUsize, Ordering},
};
use tokio::{sync::broadcast, task};

#[derive(Deserialize, Debug, Clone)]
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

#[derive(Deserialize, Debug, Clone)]
struct CategoriesResponse {
    categories: Vec<Category>,
    next_page: Option<String>,
}

#[derive(Debug)]
struct Categories {
    categories_hash: HashMap<i64, CategoryDetail>,
    sender: broadcast::Sender<EventType>,
    receiver: broadcast::Receiver<EventType>,
}

impl Categories {
    pub fn new(
        sender: broadcast::Sender<EventType>,
        receiver: broadcast::Receiver<EventType>,
    ) -> Self {
        Categories {
            categories_hash: HashMap::new(),
            sender,
            receiver,
        }
    }

    pub fn print_categories(&self) {
        for (id, category) in &self.categories_hash {
            println!("ID: {}, Name: {}, URL: {}", id, category.name, category.url);
        }
    }

    pub async fn run(&mut self) {
        let initial_url = "categories.json".to_string();
        let request = FetcherRequest::Categories(RequestUrl { url: initial_url });

        // Send the initial request
        let _ = self.sender.send(EventType::FetcherRequest(request)); // Ignoring errors, which occur if no subscribers are present

        // Receive responses
        while let Ok(message) = self.receiver.recv().await {
            if let EventType::FetcherResponse(response) = message {
                match response {
                    FetcherResponse::Categories(res) => {
                        let _ = self.sender.send(EventType::IncrementActive);
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
                        let _ = self.sender.send(EventType::DecrementActive);
                    }
                    FetcherResponse::FetchFailed { error } => {
                        eprintln!("Fetch failed: {}", error);
                    }
                }
                self.print_categories()
            }
        }
    }
}

#[derive(Debug, Clone)]
enum EventType {
    FetcherRequest(FetcherRequest),
    FetcherResponse(FetcherResponse),
    IncrementActive,
    DecrementActive,
}

#[derive(Debug, Clone)]
enum FetcherRequest {
    Categories(RequestUrl),
}

#[derive(Debug, Clone)]
struct RequestUrl {
    url: String,
}

#[derive(Debug, Clone)]
enum FetcherResponse {
    Categories(CategoriesResponse),
    FetchFailed { error: String },
}

struct Fetcher {
    client: Client,
    sender: broadcast::Sender<EventType>,
    receiver: broadcast::Receiver<EventType>,
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
        sender: broadcast::Sender<EventType>,
        receiver: broadcast::Receiver<EventType>,
    ) -> Self {
        Fetcher {
            client: Client::new(),
            sender,
            receiver,
            config,
        }
    }

    pub async fn run(&mut self) {
        while let Ok(event) = self.receiver.recv().await {
            match event {
                EventType::FetcherRequest(fetcher_request) => {
                    match fetcher_request {
                        FetcherRequest::Categories(request_url) => {
                            let client = self.client.clone();
                            let sender = self.sender.clone();
                            let url = request_url.url.clone();
                            let config = self.config.clone();

                            let _ = sender.send(EventType::IncrementActive);
                            task::spawn(async move {
                                let response = Fetcher::fetch_data(&client, &config, &url).await;
                                match response {
                                    Ok(data) => {
                                        match serde_json::from_str::<CategoriesResponse>(&data) {
                                        Ok(categories_response) => {
                                            let response_event = EventType::FetcherResponse(
                                                FetcherResponse::Categories(categories_response)
                                            );
                                            if sender.send(response_event).is_err() {
                                                eprintln!("Failed to send categories response");
                                            }
                                        },
                                        Err(_) => eprintln!("Response did not match expected CategoriesResponse")
                                    }
                                    }
                                    Err(e) => {
                                        let error_msg = format!("Failed to fetch data: {}", e);
                                        let failure_event = EventType::FetcherResponse(
                                            FetcherResponse::FetchFailed { error: error_msg },
                                        );
                                        if sender.send(failure_event).is_err() {
                                            eprintln!("Failed to send fetch failure response");
                                        }
                                    }
                                }
                                let _ = sender.send(EventType::DecrementActive);
                            });
                        } // Handle other fetcher requests similarly
                    }
                }
                _ => {} // Handle other event types or ignore
            }
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
            .await?;

        if response.status().is_success() {
            let data = response.text().await?;
            Ok(data)
        } else {
            Err(ReqwestError::from(response.error_for_status().unwrap_err()))
        }
    }
}

struct AppState {
    active_count: AtomicUsize,
    tx: broadcast::Sender<EventType>,
    rx: broadcast::Receiver<EventType>,
}

impl AppState {
    fn new(tx: broadcast::Sender<EventType>, rx: broadcast::Receiver<EventType>) -> Self {
        AppState {
            active_count: AtomicUsize::new(0),
            tx,
            rx,
        }
    }

    async fn monitor_state(&mut self) {
        while let Ok(update) = self.rx.recv().await {
            match update {
                EventType::IncrementActive => {
                    self.active_count.fetch_add(1, Ordering::SeqCst);
                }
                EventType::DecrementActive => {
                    self.active_count.fetch_sub(1, Ordering::SeqCst);
                }
                _ => {} // Handle other EventType variants if necessary
            }
            // Check the current active count after update
            let current_active_count = self.active_count.load(Ordering::SeqCst);
            println!("Current active tasks: {}", current_active_count);

            // If the active count is zero, print "no more active tasks"
            if current_active_count == 0 {
                println!("No more active tasks.");
            }
        }
    }
}

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
    let mut categories = Categories::new(tx.clone(), tx.subscribe());

    let state_handle = tokio::spawn(async move {
        app_state.monitor_state().await;
    });

    let fetcher_handle = tokio::spawn(async move {
        fetcher.run().await;
    });

    let categories_handle = tokio::spawn(async move {
        categories.run().await;
    });

    let _ = tokio::try_join!(state_handle, fetcher_handle, categories_handle)?;

    Ok(())
}
