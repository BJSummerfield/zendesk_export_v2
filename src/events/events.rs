use crate::models::categories::CategoriesResponse;

#[derive(Debug, Clone)]
pub enum EventType {
    FetcherRequest(FetcherRequest),
    FetcherResponse(FetcherResponse),
    FileRequest(FileRequest),
    UpdateState(StateUpdate),
    Shutdown,
}

#[derive(Debug, Clone)]
pub enum FileRequest {
    Markdown { path: String, data: String },
    Image { path: String, data: Vec<u8> },
}

#[derive(Debug, Clone)]
pub enum StateUpdate {
    Categories(ActiveCount),
    Fetcher(ActiveCount),
    FileWriter(ActiveCount),
}

#[derive(Debug, Clone)]
pub enum ActiveCount {
    Increment,
    Decrement,
}

#[derive(Debug, Clone)]
pub enum FetcherRequest {
    Categories(RequestUrl),
}

#[derive(Debug, Clone)]
pub struct RequestUrl {
    pub url: String,
}

#[derive(Debug, Clone)]
pub enum FetcherResponse {
    Categories(CategoriesResponse),
    FetchFailed { error: String },
}
