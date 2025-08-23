-- Initial schema for SDRTrunk transcriber

-- Radio calls table
CREATE TABLE IF NOT EXISTS radio_calls (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    call_timestamp TIMESTAMPTZ NOT NULL,
    
    -- System information
    system_id VARCHAR(50) NOT NULL,
    system_label VARCHAR(255),
    
    -- Radio metadata
    frequency BIGINT,
    talkgroup_id INTEGER,
    talkgroup_label VARCHAR(255),
    talkgroup_group VARCHAR(255),
    talkgroup_tag VARCHAR(255),
    
    -- Source information
    source_radio_id INTEGER,
    talker_alias VARCHAR(255),
    
    -- Audio file information
    audio_filename VARCHAR(255),
    audio_file_path VARCHAR(500),
    audio_size_bytes BIGINT,
    audio_content_type VARCHAR(100),
    duration_seconds DECIMAL(10,3),
    
    -- Transcription results (for future use)
    transcription_text TEXT,
    transcription_confidence DECIMAL(5,4),
    transcription_language VARCHAR(10),
    transcription_status VARCHAR(20) DEFAULT 'pending',
    transcription_started_at TIMESTAMPTZ,
    transcription_completed_at TIMESTAMPTZ,
    speaker_segments JSONB,
    speaker_count INTEGER,
    
    -- Additional metadata
    patches TEXT,
    frequencies TEXT,
    sources TEXT,
    
    -- Upload tracking
    upload_ip INET,
    upload_timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    upload_api_key_id VARCHAR(100)
);

-- Indexes for performance
CREATE INDEX idx_radio_calls_timestamp ON radio_calls (call_timestamp DESC);
CREATE INDEX idx_radio_calls_system ON radio_calls (system_id, call_timestamp DESC);
CREATE INDEX idx_radio_calls_talkgroup ON radio_calls (talkgroup_id, call_timestamp DESC);
CREATE INDEX idx_radio_calls_transcription_status ON radio_calls (transcription_status);
CREATE INDEX idx_radio_calls_created_at ON radio_calls (created_at DESC);

-- Upload logs table for tracking and debugging
CREATE TABLE IF NOT EXISTS upload_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    client_ip INET NOT NULL,
    user_agent TEXT,
    api_key_used VARCHAR(100),
    system_id VARCHAR(50),
    success BOOLEAN NOT NULL DEFAULT TRUE,
    error_message TEXT,
    filename VARCHAR(255),
    file_size BIGINT,
    content_type VARCHAR(100),
    response_code INTEGER,
    processing_time_ms DECIMAL(10,3)
);

CREATE INDEX idx_upload_logs_timestamp ON upload_logs (timestamp DESC);
CREATE INDEX idx_upload_logs_ip ON upload_logs (client_ip);
CREATE INDEX idx_upload_logs_success ON upload_logs (success);

-- System statistics table
CREATE TABLE IF NOT EXISTS system_stats (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    system_id VARCHAR(50) UNIQUE NOT NULL,
    system_label VARCHAR(255),
    total_calls INTEGER DEFAULT 0,
    calls_today INTEGER DEFAULT 0,
    calls_this_hour INTEGER DEFAULT 0,
    first_seen TIMESTAMPTZ,
    last_seen TIMESTAMPTZ,
    top_talkgroups JSONB,
    upload_sources JSONB,
    last_updated TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_system_stats_system ON system_stats (system_id);
CREATE INDEX idx_system_stats_updated ON system_stats (last_updated);

-- API keys table
CREATE TABLE IF NOT EXISTS api_keys (
    id VARCHAR(100) PRIMARY KEY,
    key_hash VARCHAR(255) NOT NULL,
    description VARCHAR(255),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ,
    allowed_ips TEXT[],
    allowed_systems TEXT[],
    active BOOLEAN NOT NULL DEFAULT TRUE,
    last_used TIMESTAMPTZ,
    total_requests INTEGER DEFAULT 0
);

CREATE INDEX idx_api_keys_active ON api_keys (active);
CREATE INDEX idx_api_keys_expires ON api_keys (expires_at);