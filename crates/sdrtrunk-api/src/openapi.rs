//! `OpenAPI` 3.0 specification generator

use serde_json::json;

/// Generate comprehensive `OpenAPI` 3.0 specification
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn generate_openapi_spec() -> serde_json::Value {
    json!({
        "openapi": "3.0.3",
        "info": {
            "title": "SDRTrunk Transcriber API",
            "version": "0.1.0",
            "description": "REST API for SDRTrunk P25 radio call transcription and management with speaker diarization",
            "contact": {
                "name": "API Support",
                "url": "https://github.com/swiftraccoon/rs-sdrtrunk-transcriber"
            },
            "license": {
                "name": "GPL-3.0",
                "url": "https://www.gnu.org/licenses/gpl-3.0.html"
            }
        },
        "servers": [
            {
                "url": "http://localhost:9000",
                "description": "Development server"
            }
        ],
        "paths": {
            "/api/call-upload": {
                "post": {
                    "summary": "Upload radio call recording",
                    "description": "Upload audio files from SDRTrunk for processing and transcription. Rdio Scanner compatible.",
                    "tags": ["Uploads"],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "multipart/form-data": {
                                "schema": {
                                    "$ref": "#/components/schemas/CallUploadRequest"
                                }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Upload successful",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "$ref": "#/components/schemas/UploadResponse"
                                    }
                                }
                            }
                        },
                        "400": {
                            "description": "Invalid request",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "$ref": "#/components/schemas/ErrorResponse"
                                    }
                                }
                            }
                        }
                    }
                }
            },
            "/api/calls": {
                "get": {
                    "summary": "List radio calls",
                    "description": "Retrieve a paginated list of radio calls with optional filtering",
                    "tags": ["Calls"],
                    "parameters": [
                        {
                            "name": "system_id",
                            "in": "query",
                            "description": "Filter by system ID",
                            "schema": { "type": "string" }
                        },
                        {
                            "name": "talkgroup_id",
                            "in": "query",
                            "description": "Filter by talkgroup ID",
                            "schema": { "type": "integer" }
                        },
                        {
                            "name": "status",
                            "in": "query",
                            "description": "Filter by transcription status",
                            "schema": {
                                "type": "string",
                                "enum": ["pending", "processing", "completed", "failed"]
                            }
                        },
                        {
                            "name": "page",
                            "in": "query",
                            "description": "Page number (1-indexed)",
                            "schema": { "type": "integer", "default": 1, "minimum": 1 }
                        },
                        {
                            "name": "per_page",
                            "in": "query",
                            "description": "Results per page",
                            "schema": { "type": "integer", "default": 50, "minimum": 1, "maximum": 200 }
                        }
                    ],
                    "responses": {
                        "200": {
                            "description": "List of calls",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "$ref": "#/components/schemas/CallListResponse"
                                    }
                                }
                            }
                        }
                    }
                }
            },
            "/api/calls/{id}": {
                "get": {
                    "summary": "Get call details",
                    "description": "Retrieve detailed information for a specific call",
                    "tags": ["Calls"],
                    "parameters": [
                        {
                            "name": "id",
                            "in": "path",
                            "required": true,
                            "description": "Call UUID",
                            "schema": { "type": "string", "format": "uuid" }
                        }
                    ],
                    "responses": {
                        "200": {
                            "description": "Call details",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "$ref": "#/components/schemas/CallDetail"
                                    }
                                }
                            }
                        },
                        "404": {
                            "description": "Call not found"
                        }
                    }
                }
            },
            "/api/systems/{system_id}/stats": {
                "get": {
                    "summary": "Get system statistics",
                    "description": "Retrieve comprehensive statistics for a specific system",
                    "tags": ["Statistics"],
                    "parameters": [
                        {
                            "name": "system_id",
                            "in": "path",
                            "required": true,
                            "description": "System identifier",
                            "schema": { "type": "string" }
                        }
                    ],
                    "responses": {
                        "200": {
                            "description": "System statistics"
                        }
                    }
                }
            },
            "/api/stats/global": {
                "get": {
                    "summary": "Get global statistics",
                    "description": "Retrieve aggregated statistics across all systems",
                    "tags": ["Statistics"],
                    "responses": {
                        "200": {
                            "description": "Global statistics"
                        }
                    }
                }
            },
            "/api/ws": {
                "get": {
                    "summary": "WebSocket endpoint",
                    "description": "Real-time updates via WebSocket connection",
                    "tags": ["WebSocket"],
                    "responses": {
                        "101": {
                            "description": "Switching protocols to WebSocket"
                        }
                    }
                }
            },
            "/health": {
                "get": {
                    "summary": "Health check",
                    "description": "Basic health status",
                    "tags": ["Health"],
                    "responses": {
                        "200": {
                            "description": "Service is healthy"
                        }
                    }
                }
            },
            "/metrics": {
                "get": {
                    "summary": "Prometheus metrics",
                    "description": "Metrics in Prometheus exposition format",
                    "tags": ["Monitoring"],
                    "responses": {
                        "200": {
                            "description": "Prometheus metrics",
                            "content": {
                                "text/plain": {
                                    "schema": {
                                        "type": "string"
                                    }
                                }
                            }
                        }
                    }
                }
            },
            "/admin/api-keys": {
                "get": {
                    "summary": "List API keys",
                    "description": "List all active API keys (admin only)",
                    "tags": ["Admin"],
                    "responses": {
                        "200": {
                            "description": "List of API keys"
                        }
                    }
                },
                "post": {
                    "summary": "Create API key",
                    "description": "Generate a new API key (admin only)",
                    "tags": ["Admin"],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": {
                                    "$ref": "#/components/schemas/CreateApiKeyRequest"
                                }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "API key created"
                        }
                    }
                }
            }
        },
        "components": {
            "schemas": {
                "CallUploadRequest": {
                    "type": "object",
                    "properties": {
                        "audio": {
                            "type": "string",
                            "format": "binary",
                            "description": "Audio file (MP3, WAV, FLAC)"
                        },
                        "key": {
                            "type": "string",
                            "description": "Metadata JSON"
                        }
                    },
                    "required": ["audio"]
                },
                "UploadResponse": {
                    "type": "object",
                    "properties": {
                        "success": {
                            "type": "boolean"
                        },
                        "call_id": {
                            "type": "string",
                            "format": "uuid"
                        }
                    }
                },
                "ErrorResponse": {
                    "type": "object",
                    "properties": {
                        "error": {
                            "type": "string"
                        },
                        "code": {
                            "type": "string"
                        }
                    }
                },
                "CallListResponse": {
                    "type": "object",
                    "properties": {
                        "calls": {
                            "type": "array",
                            "items": {
                                "$ref": "#/components/schemas/CallSummary"
                            }
                        },
                        "pagination": {
                            "$ref": "#/components/schemas/Pagination"
                        }
                    }
                },
                "CallSummary": {
                    "type": "object",
                    "properties": {
                        "id": {
                            "type": "string",
                            "format": "uuid"
                        },
                        "system_id": {
                            "type": "string"
                        },
                        "talkgroup_id": {
                            "type": "integer"
                        },
                        "timestamp": {
                            "type": "string",
                            "format": "date-time"
                        },
                        "duration": {
                            "type": "number"
                        },
                        "transcription_status": {
                            "type": "string",
                            "enum": ["pending", "processing", "completed", "failed"]
                        }
                    }
                },
                "CallDetail": {
                    "type": "object",
                    "properties": {
                        "id": {
                            "type": "string",
                            "format": "uuid"
                        },
                        "system_id": {
                            "type": "string"
                        },
                        "talkgroup_id": {
                            "type": "integer"
                        },
                        "transcription_text": {
                            "type": "string"
                        },
                        "speaker_segments": {
                            "type": "array",
                            "items": {
                                "type": "object"
                            }
                        }
                    }
                },
                "Pagination": {
                    "type": "object",
                    "properties": {
                        "page": {
                            "type": "integer"
                        },
                        "per_page": {
                            "type": "integer"
                        },
                        "total": {
                            "type": "integer"
                        },
                        "total_pages": {
                            "type": "integer"
                        }
                    }
                },
                "CreateApiKeyRequest": {
                    "type": "object",
                    "properties": {
                        "description": {
                            "type": "string"
                        },
                        "expires_at": {
                            "type": "string",
                            "format": "date-time"
                        }
                    }
                }
            },
            "securitySchemes": {
                "ApiKeyAuth": {
                    "type": "apiKey",
                    "in": "header",
                    "name": "X-API-Key"
                }
            }
        },
        "tags": [
            {
                "name": "Uploads",
                "description": "Audio upload endpoints"
            },
            {
                "name": "Calls",
                "description": "Call management and retrieval"
            },
            {
                "name": "Statistics",
                "description": "System and global statistics"
            },
            {
                "name": "WebSocket",
                "description": "Real-time updates"
            },
            {
                "name": "Health",
                "description": "Health and readiness checks"
            },
            {
                "name": "Monitoring",
                "description": "Metrics and monitoring"
            },
            {
                "name": "Admin",
                "description": "Administrative functions"
            }
        ]
    })
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::cognitive_complexity,
    clippy::too_many_lines,
    clippy::unreadable_literal,
    clippy::redundant_clone,
    clippy::missing_panics_doc,
    clippy::missing_errors_doc,
    clippy::needless_pass_by_value,
    clippy::uninlined_format_args,
    unused_qualifications,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::items_after_statements,
    clippy::float_cmp,
    clippy::redundant_closure_for_method_calls,
    clippy::fn_params_excessive_bools,
    clippy::similar_names,
    clippy::map_unwrap_or,
    clippy::unused_async,
    clippy::case_sensitive_file_extension_comparisons,
    clippy::manual_string_new,
    clippy::no_effect_underscore_binding,
    clippy::option_if_let_else,
    clippy::single_char_pattern,
    clippy::ip_constant,
    clippy::or_fun_call,
    clippy::cast_lossless,
    clippy::needless_collect,
    clippy::single_match_else,
    clippy::needless_raw_string_hashes,
    clippy::match_same_arms
)]
mod tests {
    use super::*;

    #[test]
    fn test_openapi_spec_structure() {
        let spec = generate_openapi_spec();

        assert_eq!(spec["openapi"], "3.0.3");
        assert_eq!(spec["info"]["title"], "SDRTrunk Transcriber API");
        assert_eq!(spec["info"]["version"], "0.1.0");

        // Check paths exist
        assert!(spec["paths"].is_object());
        assert!(spec["paths"]["/api/call-upload"].is_object());
        assert!(spec["paths"]["/api/calls"].is_object());
        assert!(spec["paths"]["/health"].is_object());
        assert!(spec["paths"]["/metrics"].is_object());
    }

    #[test]
    fn test_openapi_components() {
        let spec = generate_openapi_spec();

        // Check schemas exist
        assert!(spec["components"]["schemas"]["CallUploadRequest"].is_object());
        assert!(spec["components"]["schemas"]["ErrorResponse"].is_object());
        assert!(spec["components"]["schemas"]["CallListResponse"].is_object());

        // Check security schemes
        assert!(spec["components"]["securitySchemes"]["ApiKeyAuth"].is_object());
    }

    #[test]
    fn test_openapi_tags() {
        let spec = generate_openapi_spec();

        let tags = spec["tags"].as_array().unwrap();
        assert!(tags.len() >= 7);

        let tag_names: Vec<&str> = tags.iter().map(|t| t["name"].as_str().unwrap()).collect();

        assert!(tag_names.contains(&"Uploads"));
        assert!(tag_names.contains(&"Calls"));
        assert!(tag_names.contains(&"Statistics"));
        assert!(tag_names.contains(&"WebSocket"));
    }
}
