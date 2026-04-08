//! Call list component for displaying radio calls
#![allow(unreachable_pub)]

use crate::api_client::CallSummary;
use leptos::prelude::*;

/// Call list component
#[allow(unreachable_pub, dead_code)]
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
#[allow(unreachable_pub)]
#[component]
fn CallRow(call: CallSummary) -> impl IntoView {
    let system_display = call
        .system_label
        .clone()
        .unwrap_or_else(|| call.system_id.to_string());
    let talkgroup_display = call
        .talkgroup_label
        .clone()
        .or_else(|| call.talkgroup_id.map(|id| id.to_string()))
        .unwrap_or_else(|| "Unknown".to_string());

    view! {
        <div class="call-row">
            <div class="call-col">
                {call.call_timestamp.format("%Y-%m-%d %H:%M:%S").to_string()}
            </div>
            <div class="call-col">
                {system_display}
            </div>
            <div class="call-col">
                {talkgroup_display}
            </div>
            <div class="call-col">
                <button class="btn btn-sm">Play</button>
                <button class="btn btn-sm">Details</button>
            </div>
        </div>
    }
}
