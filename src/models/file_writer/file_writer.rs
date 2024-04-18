use crate::events::{ActiveCount, EventType, FileRequest, StateUpdate};
use std::path::Path;
use tokio::{fs, sync::broadcast, task};

pub struct FileWriter {
    sender: broadcast::Sender<EventType>,
    receiver: broadcast::Receiver<EventType>,
    base_path: String,
}

impl FileWriter {
    pub fn new(
        sender: broadcast::Sender<EventType>,
        receiver: broadcast::Receiver<EventType>,
    ) -> Self {
        FileWriter {
            sender,
            receiver,
            base_path: "data".to_string(),
        }
    }

    pub async fn run(&mut self) {
        while let Ok(event) = self.receiver.recv().await {
            match event {
                EventType::FileRequest(file_request) => {
                    let _ = self
                        .sender
                        .send(EventType::UpdateState(StateUpdate::FileWriter(
                            ActiveCount::Increment,
                        )));
                    match file_request {
                        FileRequest::Markdown { path, data } => {
                            let file_path = format!("{}/{}", self.base_path, path);
                            handle_file_write(&file_path, data.into()).await;
                        }
                        FileRequest::Image { path, data } => {
                            let file_path = format!("{}/{}", self.base_path, path);
                            handle_file_write(&file_path, data).await;
                        }
                    }
                    let _ = self
                        .sender
                        .send(EventType::UpdateState(StateUpdate::FileWriter(
                            ActiveCount::Decrement,
                        )));
                }
                EventType::Shutdown => break,
                _ => {} // Handle other EventType variants if necessary
            }
        }
    }
}

async fn handle_file_write(path: &str, data: Vec<u8>) {
    let path = Path::new(path);
    if let Some(dir) = path.parent() {
        if !dir.exists() {
            if let Err(e) = fs::create_dir_all(dir).await {
                eprintln!("Failed to create directory: {}", e);
                return;
            }
        }
    }

    match fs::write(path, &data).await {
        Ok(_) => println!("File written successfully: {}", path.display()),
        Err(e) => eprintln!("Failed to write file: {}", e),
    }
}
