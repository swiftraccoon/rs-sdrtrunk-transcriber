//! System statistics page
#![allow(unreachable_pub, clippy::too_many_lines)]

use leptos::prelude::*;
use serde::{Deserialize, Serialize};

/// Global statistics response from API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalStatsResponse {
    /// Total number of calls processed
    pub total_calls: i64,
    /// Number of distinct systems
    pub total_systems: i64,
    /// Number of calls in the last 24 hours
    pub calls_last_24h: i64,
    /// Average call duration in seconds
    pub avg_call_duration: Option<f64>,
    /// Total transcriptions completed
    pub total_transcriptions: i64,
    /// Fraction of transcriptions that succeeded
    pub transcription_success_rate: Option<f64>,
    /// Ranked list of most active systems
    pub top_systems: Vec<SystemSummary>,
    /// Total storage used in bytes
    pub storage_used_bytes: Option<i64>,
}

/// System summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemSummary {
    /// System identifier string
    pub system_id: String,
    /// Human-readable system label
    pub system_label: Option<String>,
    /// Total calls for this system
    pub total_calls: i64,
    /// Number of recent calls
    pub recent_calls: i64,
}

/// Format bytes to human-readable format
#[allow(clippy::cast_precision_loss)]
fn format_bytes(bytes: Option<i64>) -> String {
    bytes.map_or_else(
        || "N/A".to_string(),
        |b| {
            let bf = b as f64;
            if b < 1024 {
                format!("{b} B")
            } else if b < 1024 * 1024 {
                format!("{:.2} KB", bf / 1024.0)
            } else if b < 1024 * 1024 * 1024 {
                format!("{:.2} MB", bf / (1024.0 * 1024.0))
            } else {
                format!("{:.2} GB", bf / (1024.0 * 1024.0 * 1024.0))
            }
        },
    )
}

/// Format percentage
fn format_percentage(rate: Option<f64>) -> String {
    rate.map_or_else(|| "N/A".to_string(), |r| format!("{:.1}%", r * 100.0))
}

/// Format duration
#[allow(clippy::cast_possible_truncation)]
fn format_duration(seconds: Option<f64>) -> String {
    seconds.map_or_else(
        || "N/A".to_string(),
        |s| {
            if s < 60.0 {
                format!("{s:.1}s")
            } else {
                let minutes = (s / 60.0).floor() as i32;
                let secs = (s % 60.0).round() as i32;
                format!("{minutes}m {secs}s")
            }
        },
    )
}

/// System statistics page component
#[allow(unreachable_pub, clippy::too_many_lines)]
#[component]
pub fn SystemStats() -> impl IntoView {
    // Fetch global stats from API
    let stats_resource = LocalResource::new(|| async { fetch_global_stats().await });

    view! {
        <div class="stats-page">
            <h2>System Statistics</h2>

            <Suspense fallback=move || view! { <p>"Loading statistics..."</p> }>
                {move || {
                    stats_resource.get().map(|result| {
                        match send_wrapper::SendWrapper::take(result) {
                            Ok(stats) => view! {
                                <div class="stats-grid">
                                    <div class="stats-card">
                                        <h3>Call Volume</h3>
                                        <div class="stats-content">
                                            <div class="stat-item">
                                                <span class="stat-label">Total Calls:</span>
                                                <span class="stat-value">{stats.total_calls}</span>
                                            </div>
                                            <div class="stat-item">
                                                <span class="stat-label">Last 24 Hours:</span>
                                                <span class="stat-value">{stats.calls_last_24h}</span>
                                            </div>
                                            <div class="stat-item">
                                                <span class="stat-label">Avg Duration:</span>
                                                <span class="stat-value">{format_duration(stats.avg_call_duration)}</span>
                                            </div>
                                        </div>
                                    </div>

                                    <div class="stats-card">
                                        <h3>System Activity</h3>
                                        <div class="stats-content">
                                            <div class="stat-item">
                                                <span class="stat-label">Total Systems:</span>
                                                <span class="stat-value">{stats.total_systems}</span>
                                            </div>
                                            <div class="stat-item">
                                                <span class="stat-label">Storage Used:</span>
                                                <span class="stat-value">{format_bytes(stats.storage_used_bytes)}</span>
                                            </div>
                                        </div>
                                    </div>

                                    <div class="stats-card">
                                        <h3>Transcription Metrics</h3>
                                        <div class="stats-content">
                                            <div class="stat-item">
                                                <span class="stat-label">Total Transcriptions:</span>
                                                <span class="stat-value">{stats.total_transcriptions}</span>
                                            </div>
                                            <div class="stat-item">
                                                <span class="stat-label">Success Rate:</span>
                                                <span class="stat-value">{format_percentage(stats.transcription_success_rate)}</span>
                                            </div>
                                        </div>
                                    </div>

                                    <div class="stats-card">
                                        <h3>Top Systems</h3>
                                        <div class="top-list">
                                            {if stats.top_systems.is_empty() {
                                                view! { <p>"No systems yet"</p> }.into_any()
                                            } else {
                                                view! {
                                                    <ul class="system-list">
                                                        <For
                                                            each=move || stats.top_systems.clone()
                                                            key=|sys| sys.system_id.clone()
                                                            children=move |sys: SystemSummary| {
                                                                let name = sys.system_label.unwrap_or_else(|| sys.system_id.clone());
                                                                view! {
                                                                    <li class="system-item">
                                                                        <span class="system-name">
                                                                            {name}
                                                                        </span>
                                                                        <span class="system-calls">
                                                                            {sys.total_calls} " calls"
                                                                        </span>
                                                                    </li>
                                                                }
                                                            }
                                                        />
                                                    </ul>
                                                }.into_any()
                                            }}
                                        </div>
                                    </div>
                                </div>
                            }.into_any(),
                            Err(e) => view! {
                                <div class="error">
                                    <p>"Error loading statistics: " {e}</p>
                                </div>
                            }.into_any(),
                        }
                    })
                }}
            </Suspense>
        </div>
    }
}

/// Fetch global statistics from API
///
/// # Errors
///
/// Returns an error string if the HTTP request fails or the response cannot be parsed.
#[allow(clippy::future_not_send)]
async fn fetch_global_stats() -> Result<GlobalStatsResponse, String> {
    let response = gloo_net::http::Request::get("/api/stats/global")
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?;

    if !response.ok() {
        return Err(format!("API error: {}", response.status()));
    }

    response
        .json::<GlobalStatsResponse>()
        .await
        .map_err(|e| format!("Failed to parse response: {e}"))
}
