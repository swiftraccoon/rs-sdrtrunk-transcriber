//! Main Leptos application component with routing
// Leptos #[component] macro generates pub items that trigger unreachable_pub
#![allow(unreachable_pub)]

use crate::pages::{
    admin::AdminPanel, calls::CallBrowser, dashboard::Dashboard, not_found::NotFound,
    stats::SystemStats,
};
use leptos::prelude::*;
use leptos_router::{
    components::{A, Route, Router, Routes},
    path,
};

/// Main application component
#[allow(unreachable_pub)]
#[component]
pub fn App() -> impl IntoView {
    view! {
        <Router>
            <main class="app">
                <Header />
                <div class="content">
                    <Routes fallback=|| view! { <NotFound /> }>
                        <Route path=path!("/") view=Dashboard />
                        <Route path=path!("/calls") view=CallBrowser />
                        <Route path=path!("/stats") view=SystemStats />
                        <Route path=path!("/admin") view=AdminPanel />
                    </Routes>
                </div>
            </main>
        </Router>
    }
}

/// Application header with navigation
#[allow(unreachable_pub)]
#[component]
fn Header() -> impl IntoView {
    view! {
        <header class="header">
            <div class="header-content">
                <h1 class="logo">
                    <A href="/">SDRTrunk Transcriber</A>
                </h1>
                <nav class="nav">
                    <A href="/">Dashboard</A>
                    <A href="/calls">Calls</A>
                    <A href="/stats">Statistics</A>
                    <A href="/admin">Admin</A>
                </nav>
            </div>
        </header>
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
#[allow(clippy::missing_panics_doc)]
mod tests {
    use super::*;

    #[test]
    fn test_app_component_creation() {
        // Test that the App component can be created without panicking
        // This test only works on wasm32 because Leptos Router requires js-sys.
        let _ = App();
    }
}
