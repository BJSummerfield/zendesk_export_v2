use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::{broadcast, Mutex};

use crate::events::{ActiveCount, EventType, StateUpdate};

#[derive(Debug, PartialEq)]
enum CurrentState {
    Initialized,
    Active,
    Inactive,
}

struct State {
    active_count: AtomicUsize,
    current_state: Mutex<CurrentState>,
}

pub struct AppState {
    categories: State,
    fetcher: State,
    file_writer: State,
    tx: broadcast::Sender<EventType>,
    rx: broadcast::Receiver<EventType>,
}

impl AppState {
    pub fn new(tx: broadcast::Sender<EventType>, rx: broadcast::Receiver<EventType>) -> Self {
        AppState {
            categories: State {
                active_count: AtomicUsize::new(0),
                current_state: CurrentState::Initialized.into(),
            },
            fetcher: State {
                active_count: AtomicUsize::new(0),
                current_state: CurrentState::Initialized.into(),
            },
            file_writer: State {
                active_count: AtomicUsize::new(0),
                current_state: CurrentState::Initialized.into(),
            },
            tx,
            rx,
        }
    }

    pub async fn monitor_state(&mut self) {
        while let Ok(update) = self.rx.recv().await {
            match update {
                EventType::UpdateState(state_update) => match state_update {
                    StateUpdate::Categories(count_action) => {
                        self.update_service_state(&self.categories, count_action)
                            .await;
                    }
                    StateUpdate::Fetcher(count_action) => {
                        self.update_service_state(&self.fetcher, count_action).await;
                    }
                    StateUpdate::FileWriter(count_action) => {
                        self.update_service_state(&self.file_writer, count_action)
                            .await;
                    }
                },
                EventType::Shutdown => {
                    println!("AppState service is shutting down.");
                    break;
                }
                _ => {} // Handle other EventType variants if necessary
            }
            if self.check_all_services_inactive().await {
                println!("All services are now inactive.");
                let _ = self.tx.send(EventType::Shutdown);
            }
        }
    }

    async fn update_service_state(&self, service_state: &State, action: ActiveCount) {
        match action {
            ActiveCount::Increment => {
                let current_count = service_state.active_count.fetch_add(1, Ordering::SeqCst) + 1;
                let mut state = service_state.current_state.lock().await;
                if current_count == 1 {
                    *state = CurrentState::Active;
                }
            }
            ActiveCount::Decrement => {
                let current_count = service_state.active_count.fetch_sub(1, Ordering::SeqCst) - 1;
                let mut state = service_state.current_state.lock().await;
                if current_count == 0 {
                    *state = CurrentState::Inactive;
                }
            }
        }
    }

    async fn check_all_services_inactive(&self) -> bool {
        let categories_state = self.categories.current_state.lock().await;
        let fetcher_state = self.fetcher.current_state.lock().await;

        *categories_state == CurrentState::Inactive && *fetcher_state == CurrentState::Inactive
    }
}
