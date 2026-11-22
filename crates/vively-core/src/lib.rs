pub mod api;
pub mod auth;
pub mod catalog;
pub mod client;
pub mod config;
pub mod downloads;
pub mod error;
pub mod library;
pub mod lyrics;
pub mod media;
pub mod models;
pub mod playback;
pub mod session;
pub mod settings;

pub use api::AudioService;
pub use client::VkClient;
pub use error::{Error, Result};
