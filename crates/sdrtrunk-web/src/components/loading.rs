//! Loading component for displaying loading states
#![allow(unreachable_pub)]

use leptos::prelude::*;

/// Loading spinner component
#[allow(unreachable_pub)]
#[component]
pub fn Loading() -> impl IntoView {
    view! {
        <div class="loading">
            <div class="spinner"></div>
            <p>Loading...</p>
        </div>
    }
}
