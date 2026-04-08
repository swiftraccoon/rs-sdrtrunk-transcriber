//! Call browser page for searching and viewing radio calls
#![allow(unreachable_pub, clippy::too_many_lines)]

use leptos::prelude::*;
use serde::{Deserialize, Serialize};

/// Call summary from API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallSummary {
    /// Unique call identifier
    pub id: String,
    /// System identifier
    pub system_id: String,
    /// Optional talkgroup identifier
    pub talkgroup_id: Option<i32>,
    /// Timestamp string
    pub timestamp: String,
    /// Duration in seconds
    pub duration: Option<f64>,
    /// Transcription processing status
    pub transcription_status: Option<String>,
    /// Transcription output text
    pub transcription_text: Option<String>,
}

/// Pagination info from API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginationInfo {
    /// Current page number
    pub page: i32,
    /// Results per page
    pub per_page: i32,
    /// Total result count
    pub total: i64,
    /// Total number of pages
    pub total_pages: i32,
}

/// API response for list calls
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListCallsResponse {
    /// List of calls
    pub calls: Vec<CallSummary>,
    /// Pagination metadata
    pub pagination: PaginationInfo,
}

/// Format duration in seconds to MM:SS
#[allow(clippy::cast_possible_truncation)]
fn format_duration(seconds: Option<f64>) -> String {
    seconds.map_or_else(
        || "--:--".to_string(),
        |s| {
            let minutes = (s / 60.0).floor() as i32;
            let secs = (s % 60.0).round() as i32;
            format!("{minutes:02}:{secs:02}")
        },
    )
}

/// Format transcription status into a display string
fn format_status(status: Option<&str>) -> String {
    match status {
        Some("completed") => "Completed".to_string(),
        Some("processing") => "Processing".to_string(),
        Some("failed") => "Failed".to_string(),
        Some("pending") => "Pending".to_string(),
        _ => "Unknown".to_string(),
    }
}

/// Call browser page component
#[allow(unreachable_pub, clippy::too_many_lines)]
#[component]
pub fn CallBrowser() -> impl IntoView {
    // State for filters
    let (search_query, set_search_query) = signal(String::new());
    let (system_filter, set_system_filter) = signal(String::new());
    let (talkgroup_filter, set_talkgroup_filter) = signal(String::new());
    let (page, set_page) = signal(1_i32);

    // Fetch calls from API
    let calls_resource = LocalResource::new(move || {
        let current_page = page.get();
        let query = search_query.get();
        let system = system_filter.get();
        let talkgroup = talkgroup_filter.get();
        async move { fetch_calls(current_page, &query, &system, &talkgroup).await }
    });

    view! {
        <div class="call-browser">
            <h2>Radio Calls</h2>
            <div class="call-browser-header">
                <div class="search-filters">
                    <input
                        type="text"
                        placeholder="Search transcriptions..."
                        class="search-input"
                        prop:value=move || search_query.get()
                        on:input=move |ev| set_search_query.set(event_target_value(&ev))
                    />
                    <select
                        class="filter-select"
                        on:change=move |ev| set_system_filter.set(event_target_value(&ev))
                    >
                        <option value="">All Systems</option>
                        <option value="metro_pd">Metro PD</option>
                        <option value="county_fire">County Fire</option>
                    </select>
                    <select
                        class="filter-select"
                        on:change=move |ev| set_talkgroup_filter.set(event_target_value(&ev))
                    >
                        <option value="">All Talkgroups</option>
                    </select>
                </div>
            </div>

            <Suspense fallback=move || view! { <p>"Loading calls..."</p> }>
                {move || {
                    calls_resource.get().map(|result| {
                        match send_wrapper::SendWrapper::take(result) {
                            Ok(response) => view! {
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
                                        <For
                                            each=move || response.calls.clone()
                                            key=|call| call.id.clone()
                                            children=move |call: CallSummary| {
                                                let status_text = format_status(call.transcription_status.as_deref());
                                                let duration_text = format_duration(call.duration);
                                                let transcription = call.transcription_text.clone().unwrap_or_else(|| "--".to_string());
                                                view! {
                                                    <div class="call-row">
                                                        <div class="call-col">{call.timestamp.clone()}</div>
                                                        <div class="call-col">{call.system_id.clone()}</div>
                                                        <div class="call-col">{call.talkgroup_id.map_or("N/A".to_string(), |tg| tg.to_string())}</div>
                                                        <div class="call-col">{duration_text}</div>
                                                        <div class="call-col">{status_text}</div>
                                                        <div class="call-col transcription-text">
                                                            {transcription}
                                                        </div>
                                                        <div class="call-col">
                                                            <button class="btn-small">View</button>
                                                            <button class="btn-small">Play</button>
                                                        </div>
                                                    </div>
                                                }
                                            }
                                        />
                                    </div>
                                </div>
                                <div class="pagination">
                                    <button
                                        disabled=move || page.get() <= 1
                                        on:click=move |_| set_page.update(|p| *p -= 1)
                                    >
                                        Previous
                                    </button>
                                    <span>
                                        Page {move || page.get()} of {response.pagination.total_pages}
                                    </span>
                                    <button
                                        disabled=move || page.get() >= response.pagination.total_pages
                                        on:click=move |_| set_page.update(|p| *p += 1)
                                    >
                                        Next
                                    </button>
                                </div>
                            }.into_any(),
                            Err(e) => view! {
                                <div class="error">
                                    <p>"Error loading calls: " {e}</p>
                                </div>
                            }.into_any(),
                        }
                    })
                }}
            </Suspense>
        </div>
    }
}

/// Fetch calls from API
///
/// # Errors
///
/// Returns an error string if the HTTP request fails or the response cannot be parsed.
#[allow(clippy::future_not_send)]
async fn fetch_calls(
    page: i32,
    _search: &str,
    system: &str,
    talkgroup: &str,
) -> Result<ListCallsResponse, String> {
    use std::fmt::Write as _;

    // Build query parameters
    let mut url = format!("/api/calls?page={page}&per_page=50");

    if !system.is_empty() {
        let _ = write!(url, "&system_id={system}");
    }

    if !talkgroup.is_empty() {
        let _ = write!(url, "&talkgroup_id={talkgroup}");
    }

    // Fetch from API
    let response = gloo_net::http::Request::get(&url)
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?;

    if !response.ok() {
        return Err(format!("API error: {}", response.status()));
    }

    response
        .json::<ListCallsResponse>()
        .await
        .map_err(|e| format!("Failed to parse response: {e}"))
}
