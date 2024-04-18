use rayon::prelude::*;
use serde::Deserialize;
use std::collections::HashMap;
use tokio::sync::broadcast;

use crate::events::{
    ActiveCount, EventType, FetcherRequest, FetcherResponse, RequestUrl, StateUpdate,
};

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
pub struct CategoriesResponse {
    categories: Vec<Category>,
    next_page: Option<String>,
}

#[derive(Debug)]
pub struct Categories {
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

        // Receive responses and handle shutdown
        while let Ok(message) = self.receiver.recv().await {
            match message {
                EventType::FetcherResponse(response) => {
                    match response {
                        FetcherResponse::Categories(res) => {
                            // Increment active count before processing
                            println!("{}", res.next_page.unwrap_or("None".to_string()));
                            let _ =
                                self.sender
                                    .send(EventType::UpdateState(StateUpdate::Categories(
                                        ActiveCount::Increment,
                                    )));

                            // Process categories
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

                            // Decrement active count after processing
                            let _ =
                                self.sender
                                    .send(EventType::UpdateState(StateUpdate::Categories(
                                        ActiveCount::Decrement,
                                    )));
                        }
                        FetcherResponse::FetchFailed { error } => {
                            eprintln!("Fetch failed: {}", error);
                        }
                    }
                    self.print_categories();
                }
                EventType::Shutdown => {
                    println!("Categories service is shutting down.");
                    break; // Exit the loop and end the task
                }
                _ => {} // Ignore other event types
            }
        }
    }
}
