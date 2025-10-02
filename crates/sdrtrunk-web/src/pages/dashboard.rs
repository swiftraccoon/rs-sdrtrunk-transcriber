//! Dashboard page showing live call activity and system status

use leptos::prelude::*;

/// Main dashboard page component
#[component]
pub fn Dashboard() -> impl IntoView {
    view! {
        <div class="dashboard">
            <h2>Live Dashboard</h2>
            <div class="dashboard-grid">
                <div class="dashboard-card">
                    <h3>Live Calls</h3>
                    <div class="call-feed">
                        <p>Real-time call feed will appear here</p>
                        // TODO: Implement live call feed with WebSocket updates
                    </div>
                </div>
                <div class="dashboard-card">
                    <h3>System Status</h3>
                    <div class="system-status">
                        <p>System health indicators will appear here</p>
                        // TODO: Implement system status monitoring
                    </div>
                </div>
                <div class="dashboard-card">
                    <h3>Recent Activity</h3>
                    <div class="recent-activity">
                        <p>Recent transcription activity will appear here</p>
                        // TODO: Implement recent activity feed
                    </div>
                </div>
                <div class="dashboard-card">
                    <h3>Quick Stats</h3>
                    <div class="quick-stats">
                        <p>Key metrics will appear here</p>
                        // TODO: Implement quick statistics display
                    </div>
                </div>
            </div>
        </div>
    }
}