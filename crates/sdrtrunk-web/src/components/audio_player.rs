//! Audio player component for playing radio call recordings

use leptos::prelude::*;

/// Audio player component for call recordings
#[component]
pub fn AudioPlayer(
    /// URL of the audio file to play
    audio_url: String,
    /// Call ID for the audio
    call_id: uuid::Uuid,
) -> impl IntoView {
    view! {
        <div class="audio-player">
            <audio controls>
                <source src={audio_url} type="audio/mpeg" />
                <p>Your browser does not support the audio element.</p>
            </audio>
            <div class="audio-controls">
                <span class="call-id">Call: {call_id.to_string()}</span>
                // TODO: Add additional controls (download, share, etc.)
            </div>
        </div>
    }
}