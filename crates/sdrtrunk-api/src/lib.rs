//! SDRTrunk API server library

pub mod handlers;
pub mod routes;
pub mod state;
// pub mod middleware; // Disabled for minimal build
// pub mod extractors; // Disabled for minimal build

pub use state::AppState;

use sdrtrunk_core::context_error::Result;
use axum::Router;
use sdrtrunk_core::Config;
use sdrtrunk_database::PgPool;
use std::sync::Arc;

/// Build the API router with all routes and middleware
pub async fn build_router(config: Config, pool: PgPool) -> Result<Router> {
    let state = Arc::new(AppState::new(config, pool)?);

    // Validate the application state
    state.validate()?;

    // Build the complete router with all routes
    let app = routes::build_router().with_state(state);

    Ok(app)
}

/// Build a minimal router for testing (without authentication)
#[cfg(test)]
pub async fn build_test_router(config: Config, pool: PgPool) -> Result<Router> {
    let state = Arc::new(AppState::new(config, pool)?);

    // Build a simplified router for testing
    let app = Router::new()
        .merge(routes::health_routes())
        .merge(routes::docs_routes())
        .with_state(state);

    Ok(app)
}
