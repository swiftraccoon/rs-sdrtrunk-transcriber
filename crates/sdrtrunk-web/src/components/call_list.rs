//! Call list component for displaying radio calls

use leptos::prelude::*;
use crate::api_client::CallSummary;

/// Call list component
#[component]
pub fn CallList(
    /// List of calls to display
    calls: Vec<CallSummary>,
) -> impl IntoView {
    view! {
        <div class="call-list">
            <div class="call-list-header">
                <div class="header-col">Time</div>
                <div class="header-col">System</div>
                <div class="header-col">Talkgroup</div>
                <div class="header-col">Actions</div>
            </div>
            <div class="call-list-body">
                {calls.into_iter().map(|call| {
                    view! {
                        <CallRow call />
                    }
                }).collect::<Vec<_>>()}
            </div>
        </div>
    }
}

/// Individual call row component
#[component]
fn CallRow(call: CallSummary) -> impl IntoView {
    view! {
        <div class="call-row">
            <div class="call-col">
                {call.call_timestamp.format("%Y-%m-%d %H:%M:%S").to_string()}
            </div>
            <div class="call-col">
                {call.system_label.unwrap_or(call.system_id)}
            </div>
            <div class="call-col">
                {call.talkgroup_label.or(call.talkgroup_id.map(|id| id.to_string())).unwrap_or_else(|| "Unknown".to_string())}
            </div>
            <div class="call-col">
                <button class="btn btn-sm">Play</button>
                <button class="btn btn-sm">Details</button>
            </div>
        </div>
    }
}