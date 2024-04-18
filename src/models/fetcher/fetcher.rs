use crate::events::{ActiveCount, EventType, FetcherRequest, FetcherResponse, StateUpdate};
use crate::models::categories::CategoriesResponse;
use reqwest::{Client, Error as ReqwestError};
use tokio::sync::broadcast;

pub struct Fetcher {
    client: Client,
    sender: broadcast::Sender<EventType>,
    receiver: broadcast::Receiver<EventType>,
    config: FetcherConfig,
}

#[derive(Clone, Debug)]
pub struct FetcherConfig {
    pub base_url: String,
    pub language: String,
    pub email: String,
    pub password: String,
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
                    let _ = self
                        .sender
                        .send(EventType::UpdateState(StateUpdate::Fetcher(
                            ActiveCount::Increment,
                        )));
                    let response = self.handle_request(fetcher_request).await;
                    if let Err(e) = self.sender.send(response) {
                        eprintln!("Failed to communicate with event system: {}", e);
                    }
                    let _ = self
                        .sender
                        .send(EventType::UpdateState(StateUpdate::Fetcher(
                            ActiveCount::Decrement,
                        )));
                }
                EventType::Shutdown => {
                    println!("Fetcher service is shutting down.");
                    break;
                }
                _ => {} // Handle other event types or ignore
            }
        }
    }

    async fn handle_request(&self, fetcher_request: FetcherRequest) -> EventType {
        match fetcher_request {
            FetcherRequest::Categories(request_url) => {
                let url = request_url.url;
                match Fetcher::fetch_data(&self.client, &self.config, &url).await {
                    Ok(data) => match serde_json::from_str::<CategoriesResponse>(&data) {
                        Ok(categories_response) => EventType::FetcherResponse(
                            FetcherResponse::Categories(categories_response),
                        ),
                        Err(_) => EventType::FetcherResponse(FetcherResponse::FetchFailed {
                            error: "Invalid response format".to_string(),
                        }),
                    },
                    Err(e) => EventType::FetcherResponse(FetcherResponse::FetchFailed {
                        error: format!("Failed to fetch data: {}", e),
                    }),
                }
            } // Add other FetcherRequest cases here
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

        response.text().await.map_err(ReqwestError::from)
    }
}
