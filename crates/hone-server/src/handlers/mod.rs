//! HTTP request handlers organized by domain
//!
//! Each submodule contains handlers for a specific API area.

pub mod accounts;
pub mod alerts;
pub mod audit;
pub mod auth;
pub mod backup;
pub mod detection;
pub mod entities;
pub mod explore;
pub mod export;
pub mod feedback;
pub mod import_history;
pub mod insights;
pub mod locations;
pub mod mileage;
pub mod ollama;
pub mod receipts;
pub mod reports;
pub mod splits;
pub mod subscriptions;
pub mod suggestions;
pub mod tags;
pub mod training;
pub mod transactions;
pub mod trips;

// Re-export all handlers for use in router
pub use accounts::*;
pub use alerts::*;
pub use audit::*;
pub use auth::*;
pub use backup::*;
pub use detection::*;
pub use entities::*;
pub use explore::*;
pub use export::*;
pub use feedback::*;
pub use import_history::*;
pub use insights::*;
pub use locations::*;
pub use mileage::*;
pub use ollama::*;
pub use receipts::*;
pub use reports::*;
pub use splits::*;
pub use subscriptions::*;
pub use suggestions::*;
pub use tags::*;
pub use training::*;
pub use transactions::*;
pub use trips::*;
