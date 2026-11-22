//! Headless Vively application core shared by the desktop (gpui) and Android
//! (UniFFI) front-ends. Nothing in this crate may depend on a UI toolkit.

pub mod account;
pub mod actions;
pub mod app;
pub mod bootstrap;
pub mod auth;
pub mod captcha;
pub mod downloads;
pub mod editor;
pub mod fmt;
pub mod library;
pub mod media;
pub mod models;
pub mod pages;
pub mod palette;
pub mod playback;
pub mod runtime;
pub mod search;

pub use app::{AppCore, CoreDeps, CoreEvent, Dirty, Host, Spawner};
pub use auth::{AuthField, AuthState};
pub use bootstrap::{CoreConfig, CorePaths, FileSessionPersistence, build_core};
pub use runtime::TokioBridge;
