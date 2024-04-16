use reqwest::{Client, Error};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

struct ResponseVectors {
    categories: Arc<Mutex<Vec<Category>>>,
}

#[derive(Deserialize, Debug)]
struct Category {
    id: i64,
    name: String,
    url: String,
}

struct FetchRequest {
    url: String,
}

enum FetchResponse {
    DataLoaded { has_next_page: bool },
    FetchFailed { error: String },
}

#[derive(Deserialize, Debug)]
struct CategoriesResponse {
    categories: Vec<Category>,
    next_page: Option<String>,
}

struct Fetcher {
    client: Client,
    rx: mpsc::Receiver<FetchRequest>,
    tx: mpsc::Sender<FetchResponse>,
    response_vectors: ResponseVectors,
}

impl Fetcher {
    pub fn new(
        rx: mpsc::Receiver<FetchRequest>,
        tx: mpsc::Sender<FetchResponse>,
        response_vectors: ResponseVectors,
    ) -> Self {
        Fetcher {
            client: Client::new(),
            rx,
            tx,
            response_vectors,
        }
    }

    pub async fn run(&mut self) {
        while let Some(request) = self.rx.recv().await {
            match self.fetch_data(&request.url).await {
                Ok(data) => {
                    match serde_json::from_str::<CategoriesResponse>(&data) {
                        Ok(categories_response) => {
                            // Properly await on the lock
                            let mut categories = self.response_vectors.categories.lock().await;
                            categories.extend(categories_response.categories);

                            // Determine if there's a next page
                            let has_next_page = categories_response.next_page.is_some();
                            // Send a response back about the load operation
                            if self
                                .tx
                                .send(FetchResponse::DataLoaded { has_next_page })
                                .await
                                .is_err()
                            {
                                eprintln!("Failed to send fetch response");
                            }
                        }
                        Err(_) => {
                            eprintln!("Response did not match expected CategoriesResponse");
                        }
                    }
                }
                Err(e) => {
                    let error_msg = format!("Failed to fetch data: {}", e);
                    // Handle the sending error more gracefully
                    if self
                        .tx
                        .send(FetchResponse::FetchFailed { error: error_msg })
                        .await
                        .is_err()
                    {
                        eprintln!("Failed to send fetch failure response");
                    }
                }
            }
        }
    }

    async fn fetch_data(&self, url: &str) -> Result<String, Error> {
        let response = self.client.get(url).send().await?;
        response.text().await
    }
}

fn main() {
    unimplemented!();
}
