#![forbid(unsafe_code)]

//! kira-cli library — exposes modules for integration tests.

#[cfg(test)]
mod agent_props;
pub mod app;
pub mod backoff;
pub mod commands;
pub mod config;
pub mod core_client;
#[cfg(test)]
mod core_client_props;
pub mod health;
pub mod input;
pub mod layers;
pub mod messages;
pub mod models;
pub mod polling;
pub mod queue;
pub mod retry;
pub mod sse;
pub mod ui;
pub mod views;
