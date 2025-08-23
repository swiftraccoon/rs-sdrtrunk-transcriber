//! `SDRTrunk` API server library

pub mod handlers;
pub mod routes;
pub mod state;
// pub mod middleware; // Disabled for minimal build
// pub mod extractors; // Disabled for minimal build

pub use state::AppState;

use axum::Router;
use sdrtrunk_core::Config;
use sdrtrunk_core::context_error::Result;
use sdrtrunk_database::PgPool;
use std::sync::Arc;

/// Build the API router with all routes and middleware
///
/// # Errors
///
/// Returns an error if the application state validation fails.
pub fn build_router(config: Config, pool: PgPool) -> Result<Router> {
    let state = Arc::new(AppState::new(config, pool)?);

    // Validate the application state
    state.validate()?;

    // Build the complete router with all routes
    let app = routes::build_router().with_state(state);

    Ok(app)
}

/// Build a minimal router for testing (without authentication)
///
/// # Errors
///
/// Returns an error if the application state creation fails.
#[cfg(test)]
pub fn build_test_router(config: Config, pool: PgPool) -> Result<Router> {
    let state = Arc::new(AppState::new(config, pool)?);

    // Build a simplified router for testing
    let app = Router::new()
        .merge(routes::health_routes())
        .merge(routes::docs_routes())
        .with_state(state);

    Ok(app)
}
