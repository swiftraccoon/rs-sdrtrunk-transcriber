//! Main Leptos application component with routing

use leptos::prelude::*;
use leptos_router::{components::*, *};
use crate::pages::{dashboard::Dashboard, calls::CallBrowser, stats::SystemStats, admin::AdminPanel, not_found::NotFound};

/// Main application component
#[component]
pub fn App() -> impl IntoView {
    view! {
        <Router>
            <main class="app">
                <Header />
                <div class="content">
                    <Routes>
                        <Route path="/" view=Dashboard />
                        <Route path="/calls" view=CallBrowser />
                        <Route path="/stats" view=SystemStats />
                        <Route path="/admin" view=AdminPanel />
                        <Route path="/*any" view=NotFound />
                    </Routes>
                </div>
            </main>
        </Router>
    }
}

/// Application header with navigation
#[component]
fn Header() -> impl IntoView {
    view! {
        <header class="header">
            <div class="header-content">
                <h1 class="logo">
                    <A href="/">SDRTrunk Transcriber</A>
                </h1>
                <nav class="nav">
                    <A href="/" class="nav-link">Dashboard</A>
                    <A href="/calls" class="nav-link">Calls</A>
                    <A href="/stats" class="nav-link">Statistics</A>
                    <A href="/admin" class="nav-link">Admin</A>
                </nav>
            </div>
        </header>
    }
}

#[cfg(test)]
#[allow(clippy::missing_panics_doc)]
mod tests {
    use super::*;

    #[test]
    fn test_app_component_creation() {
        // Test that the App component can be created without panicking
        let _ = App();
    }
}