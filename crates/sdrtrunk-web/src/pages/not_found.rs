//! 404 Not Found page

use leptos::prelude::*;
use leptos_router::components::*;

/// 404 Not Found page component
#[component]
pub fn NotFound() -> impl IntoView {
    view! {
        <div class="not-found">
            <h2>Page Not Found</h2>
            <p>The page you are looking for does not exist.</p>
            <A href="/" class="btn btn-primary">Return to Dashboard</A>
        </div>
    }
}