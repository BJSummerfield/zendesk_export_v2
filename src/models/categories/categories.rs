use rayon::prelude::*;
use serde::Deserialize;
use tokio::sync::broadcast;

use crate::events::{
    ActiveCount, EventType, FetcherRequest, FetcherResponse, FileRequest, RequestUrl, StateUpdate,
};
use crate::utils::Utils;

#[derive(Deserialize, Debug, Clone)]
struct Category {
    id: i64,
    name: String,
    url: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct CategoriesResponse {
    categories: Vec<Category>,
    next_page: Option<String>,
}

#[derive(Debug)]
pub struct Categories {
    sender: broadcast::Sender<EventType>,
    receiver: broadcast::Receiver<EventType>,
}

impl Categories {
    pub fn new(
        sender: broadcast::Sender<EventType>,
        receiver: broadcast::Receiver<EventType>,
    ) -> Self {
        Categories { sender, receiver }
    }

    pub async fn run(&mut self) {
        let initial_url = "categories.json".to_string();
        let request = FetcherRequest::Categories(RequestUrl { url: initial_url });
        let _ = self.sender.send(EventType::FetcherRequest(request));

        while let Ok(message) = self.receiver.recv().await {
            match message {
                EventType::FetcherResponse(response) => {
                    self.process_response(response).await;
                }
                EventType::Shutdown => {
                    println!("Categories service is shutting down.");
                    break;
                }
                _ => {}
            }
        }
    }

    async fn process_response(&mut self, response: FetcherResponse) {
        match response {
            FetcherResponse::Categories(res) => {
                let _ = self
                    .sender
                    .send(EventType::UpdateState(StateUpdate::Categories(
                        ActiveCount::Increment,
                    )));

                // Handle pagination
                if let Some(next_page) = res.next_page {
                    let next_page_url = next_page.split('/').last().unwrap_or("").to_string();
                    let request = FetcherRequest::Categories(RequestUrl { url: next_page_url });
                    let _ = self.sender.send(EventType::FetcherRequest(request));
                }

                res.categories.into_par_iter().for_each(|cat| {
                    let sanitized_name = Utils::sanitize_name(&cat.name);
                    let front_matter = Utils::create_front_matter(&cat.name);
                    let path = format!("{}/_index.md", sanitized_name);

                    // Send the file write request
                    let _ = self
                        .sender
                        .send(EventType::FileRequest(FileRequest::Markdown {
                            path,
                            data: front_matter,
                        }));
                });

                let _ = self
                    .sender
                    .send(EventType::UpdateState(StateUpdate::Categories(
                        ActiveCount::Decrement,
                    )));
            }
            FetcherResponse::FetchFailed { error } => {
                eprintln!("Fetch failed: {}", error);
            }
        }
    }
}
