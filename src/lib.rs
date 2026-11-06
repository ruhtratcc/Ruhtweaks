pub mod app_ui;
pub mod atlas;
pub mod browser;
pub mod catalog;
pub mod drivers;
pub mod installer;
pub mod security;
pub mod state;
pub mod system_info;
pub mod tweaks;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Message(String),
    #[error("Erro de arquivo: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON inválido: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Falha de rede: {0}")]
    Network(#[from] reqwest::Error),
}
pub type Result<T> = std::result::Result<T, Error>;
pub fn fail(message: impl Into<String>) -> Error {
    Error::Message(message.into())
}
