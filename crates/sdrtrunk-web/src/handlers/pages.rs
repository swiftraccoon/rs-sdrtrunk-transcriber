//! Page handlers for serving HTML templates

use axum::response::Html;

/// Dashboard page
pub async fn dashboard() -> Html<&'static str> {
    Html(include_str!("../../templates/dashboard.html"))
}

/// Calls browser page
pub async fn calls_page() -> Html<&'static str> {
    Html(include_str!("../../templates/calls.html"))
}

/// Statistics page
pub async fn stats_page() -> Html<&'static str> {
    Html(include_str!("../../templates/stats.html"))
}

/// Admin page
pub async fn admin_page() -> Html<&'static str> {
    Html(include_str!("../../templates/admin.html"))
}
