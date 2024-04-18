use reqwest::{Client, Error as ReqwestError};
use tokio::{sync::broadcast, task};

use crate::events::{ActiveCount, EventType, FetcherRequest, FetcherResponse, StateUpdate};
use crate::models::categories::CategoriesResponse;

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
                    match fetcher_request {
                        FetcherRequest::Categories(request_url) => {
                            let client = self.client.clone();
                            let sender = self.sender.clone();
                            let url = request_url.url.clone();
                            let config = self.config.clone();

                            let _ = sender.send(EventType::UpdateState(StateUpdate::Fetcher(
                                ActiveCount::Increment,
                            )));
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
                                let _ = sender.send(EventType::UpdateState(StateUpdate::Fetcher(
                                    ActiveCount::Decrement,
                                )));
                            });
                        } // Handle other fetcher requests similarly
                    }
                }
                EventType::Shutdown => break,
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
