#![warn(clippy::all)]

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate prometheus;

pub(crate) mod auth;
pub(crate) mod channel;
pub mod environment;
pub(crate) mod error;
pub mod filters;
pub(crate) mod handlers;
pub(crate) mod jwt;
pub mod metrics;
pub(crate) mod pool;
pub mod problem;
pub mod settings;
