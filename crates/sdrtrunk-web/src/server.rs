//! Web server setup and configuration

use crate::{routes::build_routes, state::AppState};
use axum::Router;
use sdrtrunk_core::Config;
use std::sync::Arc;

/// Build the complete web application with all routes and state
pub fn build_app(config: Config) -> Router {
    let state = Arc::new(AppState::new(config));

    build_routes().with_state(state)
}
