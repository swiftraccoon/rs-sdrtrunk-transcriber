//! Loading component for displaying loading states

use leptos::prelude::*;

/// Loading spinner component
#[component]
pub fn Loading() -> impl IntoView {
    view! {
        <div class="loading">
            <div class="spinner"></div>
            <p>Loading...</p>
        </div>
    }
}