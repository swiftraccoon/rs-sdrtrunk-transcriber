//! System statistics page

use leptos::prelude::*;

/// System statistics page component
#[component]
pub fn SystemStats() -> impl IntoView {
    view! {
        <div class="stats-page">
            <h2>System Statistics</h2>
            <div class="stats-grid">
                <div class="stats-card">
                    <h3>Call Volume</h3>
                    <div class="chart-container">
                        <p>Call volume charts will appear here</p>
                        // TODO: Implement call volume charts
                    </div>
                </div>
                <div class="stats-card">
                    <h3>System Activity</h3>
                    <div class="chart-container">
                        <p>System activity charts will appear here</p>
                        // TODO: Implement system activity charts
                    </div>
                </div>
                <div class="stats-card">
                    <h3>Transcription Success Rate</h3>
                    <div class="chart-container">
                        <p>Transcription success rate will appear here</p>
                        // TODO: Implement transcription metrics
                    </div>
                </div>
                <div class="stats-card">
                    <h3>Top Talkgroups</h3>
                    <div class="top-list">
                        <p>Top talkgroups list will appear here</p>
                        // TODO: Implement top talkgroups list
                    </div>
                </div>
            </div>
        </div>
    }
}