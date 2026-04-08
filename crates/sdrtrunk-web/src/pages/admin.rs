//! Administrative interface page
#![allow(unreachable_pub, clippy::too_many_lines)]

use leptos::prelude::*;
use serde::{Deserialize, Serialize};

/// API Key summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeySummary {
    /// Key identifier
    pub id: String,
    /// Human-readable description
    pub description: String,
    /// Creation timestamp
    pub created_at: String,
    /// Optional expiration timestamp
    pub expires_at: Option<String>,
    /// When the key was last used
    pub last_used: Option<String>,
    /// Whether the key is currently active
    pub active: bool,
}

/// Health check response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    /// Overall health status string
    pub status: String,
    /// Database connectivity details
    pub database: Option<DatabaseHealth>,
    /// Server uptime in seconds
    pub uptime_seconds: Option<i64>,
}

/// Database health
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseHealth {
    /// Whether the database connection is active
    pub connected: bool,
    /// Total connection pool size
    pub pool_size: u32,
    /// Number of idle connections in the pool
    pub idle_connections: u32,
}

/// Create API key request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateApiKeyRequest {
    /// Description for the new key
    pub description: String,
}

/// Create API key response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateApiKeyResponse {
    /// The generated API key value
    pub api_key: String,
    /// Key identifier
    pub id: String,
    /// Informational message
    pub message: String,
}

/// Admin panel page component
#[allow(unreachable_pub, clippy::too_many_lines)]
#[component]
pub fn AdminPanel() -> impl IntoView {
    // State for API key creation
    let (new_key_description, set_new_key_description) = signal(String::new());
    let (show_new_key, set_show_new_key) = signal(false);
    let (new_key_value, set_new_key_value) = signal(String::new());
    let (refetch_keys, set_refetch_keys) = signal(0_u32);

    // Resources
    let api_keys_resource = LocalResource::new(move || {
        // Track the refetch signal so we re-run when it changes
        let _version = refetch_keys.get();
        async { fetch_api_keys().await }
    });
    let health_resource = LocalResource::new(|| async { fetch_health().await });

    // Action for creating API key
    let create_key_action = Action::new_local(move |description: &String| {
        let desc = description.clone();
        async move {
            match create_api_key(&desc).await {
                Ok(response) => {
                    set_new_key_value.set(response.api_key);
                    set_show_new_key.set(true);
                    set_new_key_description.set(String::new());
                    set_refetch_keys.update(|v| *v += 1);
                    Ok(())
                }
                Err(e) => Err(e),
            }
        }
    });

    // Action for database cleanup
    let cleanup_action = Action::new_local(|(): &()| async move { run_cleanup().await });

    view! {
        <div class="admin-panel">
            <h2>Administration</h2>
            <div class="admin-grid">
                // API Keys Section
                <div class="admin-card">
                    <h3>API Keys</h3>
                    <div class="admin-section">
                        <div class="create-key-form">
                            <input
                                type="text"
                                placeholder="Key description..."
                                prop:value=move || new_key_description.get()
                                on:input=move |ev| set_new_key_description.set(event_target_value(&ev))
                            />
                            <button
                                class="btn btn-primary"
                                on:click=move |_| {
                                    let desc = new_key_description.get();
                                    if !desc.is_empty() {
                                        let _ = create_key_action.dispatch(desc);
                                    }
                                }
                                disabled=move || create_key_action.pending().get()
                            >
                                {move || if create_key_action.pending().get() {
                                    "Creating..."
                                } else {
                                    "Create New API Key"
                                }}
                            </button>
                        </div>

                        {move || {
                            if show_new_key.get() {
                                view! {
                                    <div class="new-key-alert">
                                        <p><strong>"New API Key Created!"</strong></p>
                                        <p>"Save this key - it will not be shown again:"</p>
                                        <code class="api-key-code">{new_key_value.get()}</code>
                                        <button
                                            class="btn btn-small"
                                            on:click=move |_| set_show_new_key.set(false)
                                        >
                                            Dismiss
                                        </button>
                                    </div>
                                }.into_any()
                            } else {
                                view! { <div></div> }.into_any()
                            }
                        }}

                        <Suspense fallback=move || view! { <p>"Loading API keys..."</p> }>
                            {move || {
                                api_keys_resource.get().map(|result| {
                                    match send_wrapper::SendWrapper::take(result) {
                                        Ok(keys) => {
                                            if keys.is_empty() {
                                                view! { <p>"No API keys yet"</p> }.into_any()
                                            } else {
                                                view! {
                                                    <ul class="api-key-list">
                                                        <For
                                                            each=move || keys.clone()
                                                            key=|key| key.id.clone()
                                                            children=move |key: ApiKeySummary| {
                                                                let status_text = if key.active { "Active" } else { "Inactive" };
                                                                view! {
                                                                    <li class="api-key-item">
                                                                        <div class="key-info">
                                                                            <strong>{key.description.clone()}</strong>
                                                                            <span class="key-status">
                                                                                {status_text}
                                                                            </span>
                                                                        </div>
                                                                        <div class="key-meta">
                                                                            <span>Created: {key.created_at}</span>
                                                                        </div>
                                                                    </li>
                                                                }
                                                            }
                                                        />
                                                    </ul>
                                                }.into_any()
                                            }
                                        },
                                        Err(e) => view! { <p class="error">"Error: " {e}</p> }.into_any(),
                                    }
                                })
                            }}
                        </Suspense>
                    </div>
                </div>

                // System Health Section
                <div class="admin-card">
                    <h3>System Health</h3>
                    <div class="admin-section">
                        <Suspense fallback=move || view! { <p>"Checking health..."</p> }>
                            {move || {
                                health_resource.get().map(|result| {
                                    match send_wrapper::SendWrapper::take(result) {
                                        Ok(health) => view! {
                                            <div class="health-info">
                                                <div class="health-item">
                                                    <span class="health-label">Status:</span>
                                                    <span class="health-value">{health.status.clone()}</span>
                                                </div>
                                                {health.uptime_seconds.map(|uptime| {
                                                    let hours = uptime / 3600;
                                                    let minutes = (uptime % 3600) / 60;
                                                    view! {
                                                        <div class="health-item">
                                                            <span class="health-label">Uptime:</span>
                                                            <span class="health-value">{hours}h {minutes}m</span>
                                                        </div>
                                                    }.into_any()
                                                })}
                                                {health.database.map(|db| {
                                                    let db_status = if db.connected { "Connected" } else { "Disconnected" };
                                                    view! {
                                                        <div class="health-item">
                                                            <span class="health-label">Database:</span>
                                                            <span class="health-value">
                                                                {db_status}
                                                            </span>
                                                        </div>
                                                        <div class="health-item">
                                                            <span class="health-label">Pool Size:</span>
                                                            <span class="health-value">{db.pool_size}</span>
                                                        </div>
                                                    }.into_any()
                                                })}
                                            </div>
                                        }.into_any(),
                                        Err(e) => view! { <p class="error">"Error: " {e}</p> }.into_any(),
                                    }
                                })
                            }}
                        </Suspense>
                    </div>
                </div>

                // Database Maintenance Section
                <div class="admin-card">
                    <h3>Database Maintenance</h3>
                    <div class="admin-section">
                        <p>Run cleanup operations to remove old or orphaned records</p>
                        <button
                            class="btn btn-warning"
                            on:click=move |_| { let _ = cleanup_action.dispatch(()); }
                            disabled=move || cleanup_action.pending().get()
                        >
                            {move || if cleanup_action.pending().get() {
                                "Running..."
                            } else {
                                "Run Cleanup"
                            }}
                        </button>
                        {move || {
                            cleanup_action.value().get().map(|result| {
                                match result {
                                    Ok(()) => view! {
                                        <p class="success">"Cleanup completed successfully"</p>
                                    }.into_any(),
                                    Err(e) => view! {
                                        <p class="error">"Error: " {e}</p>
                                    }.into_any(),
                                }
                            })
                        }}
                    </div>
                </div>

                // System Configuration Section
                <div class="admin-card">
                    <h3>System Configuration</h3>
                    <div class="admin-section">
                        <p>Configuration management coming soon</p>
                        <div class="config-info">
                            <div class="config-item">
                                <span>API Server: Running</span>
                            </div>
                            <div class="config-item">
                                <span>Web Server: Running</span>
                            </div>
                        </div>
                    </div>
                </div>
            </div>
        </div>
    }
}

/// Fetch API keys from server
///
/// # Errors
///
/// Returns an error string if the HTTP request fails or the response cannot be parsed.
#[allow(clippy::future_not_send)]
async fn fetch_api_keys() -> Result<Vec<ApiKeySummary>, String> {
    let response = gloo_net::http::Request::get("/admin/api-keys")
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?;

    if !response.ok() {
        return Err(format!("API error: {}", response.status()));
    }

    let keys: Vec<ApiKeySummary> = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {e}"))?;

    Ok(keys)
}

/// Fetch system health
///
/// # Errors
///
/// Returns an error string if the HTTP request fails or the response cannot be parsed.
#[allow(clippy::future_not_send)]
async fn fetch_health() -> Result<HealthResponse, String> {
    let response = gloo_net::http::Request::get("/health")
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?;

    if !response.ok() {
        return Err(format!("API error: {}", response.status()));
    }

    response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {e}"))
}

/// Create a new API key
///
/// # Errors
///
/// Returns an error string if the HTTP request fails or the response cannot be parsed.
#[allow(clippy::future_not_send)]
async fn create_api_key(description: &str) -> Result<CreateApiKeyResponse, String> {
    let request_body = CreateApiKeyRequest {
        description: description.to_string(),
    };

    let response = gloo_net::http::Request::post("/admin/api-keys")
        .json(&request_body)
        .map_err(|e| format!("Failed to serialize request: {e}"))?
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?;

    if !response.ok() {
        return Err(format!("API error: {}", response.status()));
    }

    response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {e}"))
}

/// Run database cleanup
///
/// # Errors
///
/// Returns an error string if the HTTP request fails.
#[allow(clippy::future_not_send)]
async fn run_cleanup() -> Result<(), String> {
    let response = gloo_net::http::Request::post("/admin/cleanup")
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?;

    if !response.ok() {
        return Err(format!("API error: {}", response.status()));
    }

    Ok(())
}
