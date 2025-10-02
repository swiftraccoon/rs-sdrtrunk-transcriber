//! Call browser page for searching and viewing radio calls

use leptos::prelude::*;

/// Call browser page component
#[component]
pub fn CallBrowser() -> impl IntoView {
    view! {
        <div class="call-browser">
            <h2>Radio Calls</h2>
            <div class="call-browser-header">
                <div class="search-filters">
                    <input
                        type="text"
                        placeholder="Search transcriptions..."
                        class="search-input"
                    />
                    <select class="filter-select">
                        <option value="">All Systems</option>
                        // TODO: Populate with actual system IDs
                    </select>
                    <select class="filter-select">
                        <option value="">All Talkgroups</option>
                        // TODO: Populate with actual talkgroups
                    </select>
                    <input
                        type="date"
                        class="date-filter"
                        placeholder="From date"
                    />
                    <input
                        type="date"
                        class="date-filter"
                        placeholder="To date"
                    />
                </div>
            </div>
            <div class="call-list">
                <div class="call-list-header">
                    <div class="header-col">Time</div>
                    <div class="header-col">System</div>
                    <div class="header-col">Talkgroup</div>
                    <div class="header-col">Duration</div>
                    <div class="header-col">Status</div>
                    <div class="header-col">Transcription</div>
                    <div class="header-col">Actions</div>
                </div>
                <div class="call-list-body">
                    <p>Call list will appear here</p>
                    // TODO: Implement call list with pagination
                </div>
            </div>
            <div class="pagination">
                // TODO: Implement pagination controls
                <p>Pagination controls will appear here</p>
            </div>
        </div>
    }
}