//! Pagination component for navigating through data

use leptos::prelude::*;

/// Pagination component
#[component]
pub fn Pagination(
    /// Current page (1-based)
    current_page: u32,
    /// Total number of pages
    total_pages: u32,
    /// Callback when page changes
    on_page_change: Callback<u32>,
) -> impl IntoView {
    let has_prev = current_page > 1;
    let has_next = current_page < total_pages;

    view! {
        <div class="pagination">
            <button
                class="pagination-btn"
                disabled={!has_prev}
                on:click=move |_| {
                    if has_prev {
                        on_page_change(current_page - 1);
                    }
                }
            >
                "Previous"
            </button>

            <span class="pagination-info">
                "Page " {current_page} " of " {total_pages}
            </span>

            <button
                class="pagination-btn"
                disabled={!has_next}
                on:click=move |_| {
                    if has_next {
                        on_page_change(current_page + 1);
                    }
                }
            >
                "Next"
            </button>
        </div>
    }
}