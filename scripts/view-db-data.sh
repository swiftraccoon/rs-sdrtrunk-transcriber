#!/bin/bash

# Database connection settings
DB_HOST="${DB_HOST:-localhost}"
DB_PORT="${DB_PORT:-5432}"
DB_NAME="${DB_NAME:-sdrtrunk_transcriber}"
DB_USER="${DB_USER:-sdrtrunk}"
DB_PASS="${DB_PASS:-sdrtrunk_dev_password}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
MAGENTA='\033[0;35m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color
BOLD='\033[1m'

# Database URL
DATABASE_URL="postgresql://${DB_USER}:${DB_PASS}@${DB_HOST}:${DB_PORT}/${DB_NAME}"

# Function to run SQL query
run_query() {
    local query="$1"
    local title="$2"
    
    echo -e "\n${BOLD}${CYAN}═══════════════════════════════════════════════════════════════════════════${NC}"
    echo -e "${BOLD}${YELLOW}$title${NC}"
    echo -e "${CYAN}═══════════════════════════════════════════════════════════════════════════${NC}\n"
    
    # Check if running in container or local
    if command -v psql &> /dev/null; then
        PGPASSWORD="$DB_PASS" psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -c "$query"
    else
        # Use podman/docker if psql not available locally
        if command -v podman &> /dev/null; then
            echo "$query" | podman exec -i sdrtrunk-postgres-prod psql -U "$DB_USER" -d "$DB_NAME"
        elif command -v docker &> /dev/null; then
            echo "$query" | docker exec -i sdrtrunk-postgres-prod psql -U "$DB_USER" -d "$DB_NAME"
        else
            echo -e "${RED}Error: psql, podman, or docker not found${NC}"
            exit 1
        fi
    fi
}

# Parse command line arguments
LIMIT=20
VIEW_TYPE="all"
SYSTEM_FILTER=""
TALKGROUP_FILTER=""
DATE_FILTER="TODAY"

while [[ $# -gt 0 ]]; do
    case $1 in
        -l|--limit)
            LIMIT="$2"
            shift 2
            ;;
        -t|--type)
            VIEW_TYPE="$2"
            shift 2
            ;;
        -s|--system)
            SYSTEM_FILTER="$2"
            shift 2
            ;;
        -g|--talkgroup)
            TALKGROUP_FILTER="$2"
            shift 2
            ;;
        -d|--date)
            DATE_FILTER="$2"
            shift 2
            ;;
        -h|--help)
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  -l, --limit NUM       Number of records to show (default: 20)"
            echo "  -t, --type TYPE       View type: all, summary, recent, stats, raw, transcriptions, speakers (default: all)"
            echo "  -s, --system ID       Filter by system ID"
            echo "  -g, --talkgroup ID    Filter by talkgroup ID"
            echo "  -d, --date DATE       Date filter: TODAY, YESTERDAY, WEEK, ALL (default: TODAY)"
            echo "  -h, --help           Show this help message"
            echo ""
            echo "Examples:"
            echo "  $0                    # Show all views with today's data"
            echo "  $0 -t raw -l 50       # Show 50 raw records"
            echo "  $0 -t stats           # Show only statistics"
            echo "  $0 -s 123 -g 41001    # Filter by system and talkgroup"
            echo "  $0 -t transcriptions  # Show transcription texts"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

# Build date filter clause
case $DATE_FILTER in
    TODAY)
        DATE_WHERE="WHERE DATE(call_timestamp) = CURRENT_DATE"
        ;;
    YESTERDAY)
        DATE_WHERE="WHERE DATE(call_timestamp) = CURRENT_DATE - INTERVAL '1 day'"
        ;;
    WEEK)
        DATE_WHERE="WHERE call_timestamp >= CURRENT_DATE - INTERVAL '7 days'"
        ;;
    ALL)
        DATE_WHERE=""
        ;;
    *)
        DATE_WHERE="WHERE DATE(call_timestamp) = '$DATE_FILTER'"
        ;;
esac

# Add system/talkgroup filters
if [ -n "$SYSTEM_FILTER" ]; then
    if [ -n "$DATE_WHERE" ]; then
        DATE_WHERE="$DATE_WHERE AND system_id = '$SYSTEM_FILTER'"
    else
        DATE_WHERE="WHERE system_id = '$SYSTEM_FILTER'"
    fi
fi

if [ -n "$TALKGROUP_FILTER" ]; then
    if [ -n "$DATE_WHERE" ]; then
        DATE_WHERE="$DATE_WHERE AND talkgroup_id = $TALKGROUP_FILTER"
    else
        DATE_WHERE="WHERE talkgroup_id = $TALKGROUP_FILTER"
    fi
fi

echo -e "${BOLD}${GREEN}╔═══════════════════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BOLD}${GREEN}║              SDRTrunk Transcriber Database Viewer                        ║${NC}"
echo -e "${BOLD}${GREEN}╚═══════════════════════════════════════════════════════════════════════════╝${NC}"

# Show summary statistics
if [[ "$VIEW_TYPE" == "all" ]] || [[ "$VIEW_TYPE" == "summary" ]]; then
    run_query "
    SELECT 
        COUNT(*) as total_records,
        COUNT(DISTINCT system_id) as systems,
        COUNT(DISTINCT talkgroup_id) as talkgroups,
        TO_CHAR(MIN(call_timestamp), 'YYYY-MM-DD HH24:MI:SS') as earliest,
        TO_CHAR(MAX(call_timestamp), 'YYYY-MM-DD HH24:MI:SS') as latest,
        COUNT(CASE WHEN transcription_status = 'completed' THEN 1 END) as transcribed,
        COUNT(CASE WHEN transcription_status = 'pending' THEN 1 END) as pending,
        COUNT(CASE WHEN transcription_status = 'failed' THEN 1 END) as failed,
        COUNT(CASE WHEN speaker_count > 1 THEN 1 END) as multi_speaker,
        ROUND(AVG(CASE WHEN speaker_count IS NOT NULL THEN speaker_count END), 2) as avg_speakers
    FROM radio_calls
    $DATE_WHERE;" \
    "📊 SUMMARY STATISTICS"
fi

# Show recent records
if [[ "$VIEW_TYPE" == "all" ]] || [[ "$VIEW_TYPE" == "recent" ]]; then
    run_query "
    SELECT 
        SUBSTRING(id::text, 1, 8) as id,
        system_id as sys,
        talkgroup_id as tg_id,
        SUBSTRING(talkgroup_label, 1, 15) as tg_label,
        frequency/1000000.0 as freq_mhz,
        TO_CHAR(call_timestamp, 'MM-DD HH24:MI:SS') as call_time,
        COALESCE(duration_seconds::text, '-') as dur_sec,
        transcription_status as status,
        SUBSTRING(audio_filename, 1, 30) as audio_file
    FROM radio_calls
    $DATE_WHERE
    ORDER BY call_timestamp DESC
    LIMIT $LIMIT;" \
    "📻 RECENT RADIO CALLS (Last $LIMIT)"
fi

# Show talkgroup statistics
if [[ "$VIEW_TYPE" == "all" ]] || [[ "$VIEW_TYPE" == "stats" ]]; then
    run_query "
    SELECT 
        system_id as sys,
        talkgroup_id as tg_id,
        SUBSTRING(talkgroup_label, 1, 20) as talkgroup,
        COUNT(*) as calls,
        COALESCE(SUM(duration_seconds), 0)::numeric(10,1) as total_sec,
        COALESCE(AVG(duration_seconds), 0)::numeric(10,1) as avg_sec,
        TO_CHAR(MAX(call_timestamp), 'HH24:MI:SS') as last_call
    FROM radio_calls
    $DATE_WHERE
    GROUP BY system_id, talkgroup_id, talkgroup_label
    ORDER BY calls DESC
    LIMIT 15;" \
    "📈 TALKGROUP ACTIVITY"
fi

# Show raw data dump
if [[ "$VIEW_TYPE" == "raw" ]]; then
    run_query "
    SELECT * FROM radio_calls
    $DATE_WHERE
    ORDER BY call_timestamp DESC
    LIMIT $LIMIT;" \
    "🗄️ RAW DATA DUMP (Last $LIMIT records)"
fi

# Show transcriptions
if [[ "$VIEW_TYPE" == "transcriptions" ]]; then
    run_query "
    SELECT
        TO_CHAR(call_timestamp, 'YYYY-MM-DD HH24:MI:SS') as timestamp,
        system_id,
        talkgroup_id,
        SUBSTRING(talkgroup_label, 1, 20) as talkgroup,
        COALESCE(duration_seconds::text, '-') as duration,
        COALESCE(speaker_count::text, '1') as speakers,
        CASE
            WHEN speaker_count > 1 THEN '🎙️'
            ELSE '👤'
        END as type,
        transcription_text
    FROM radio_calls
    WHERE transcription_text IS NOT NULL
        AND transcription_text != ''
        $(if [ -n "$DATE_WHERE" ]; then echo "AND (${DATE_WHERE#WHERE})"; fi)
    ORDER BY call_timestamp DESC
    LIMIT $LIMIT;" \
    "📝 TRANSCRIPTIONS (Last $LIMIT)"
fi

# Show speaker statistics
if [[ "$VIEW_TYPE" == "speakers" ]]; then
    run_query "
    SELECT
        speaker_count,
        COUNT(*) as calls,
        ROUND(COUNT(*) * 100.0 / SUM(COUNT(*)) OVER(), 2) as percentage,
        COUNT(DISTINCT talkgroup_id) as unique_talkgroups,
        ROUND(AVG(duration_seconds), 1) as avg_duration
    FROM radio_calls
    WHERE transcription_status = 'completed'
        AND transcription_text IS NOT NULL
        AND transcription_text != ''
        $(if [ -n "$DATE_WHERE" ]; then echo "AND (${DATE_WHERE#WHERE})"; fi)
    GROUP BY speaker_count
    ORDER BY speaker_count NULLS FIRST;" \
    "🎙️ SPEAKER DISTRIBUTION"

    # Show multi-speaker examples
    run_query "
    SELECT
        TO_CHAR(call_timestamp, 'MM-DD HH24:MI') as time,
        system_id,
        talkgroup_id,
        SUBSTRING(talkgroup_label, 1, 15) as talkgroup,
        speaker_count as spkrs,
        COALESCE(duration_seconds::text, '-') as dur,
        SUBSTRING(transcription_text, 1, 80) as sample_text
    FROM radio_calls
    WHERE speaker_count > 1
        AND transcription_text IS NOT NULL
        $(if [ -n "$DATE_WHERE" ]; then echo "AND (${DATE_WHERE#WHERE})"; fi)
    ORDER BY call_timestamp DESC
    LIMIT 10;" \
    "🗣️ RECENT MULTI-SPEAKER CALLS"
fi

# Show hourly distribution
if [[ "$VIEW_TYPE" == "all" ]] || [[ "$VIEW_TYPE" == "stats" ]]; then
    run_query "
    SELECT 
        EXTRACT(HOUR FROM call_timestamp) as hour,
        COUNT(*) as calls,
        COUNT(DISTINCT talkgroup_id) as active_tgs,
        COALESCE(SUM(duration_seconds), 0)::numeric(10,1) as total_duration
    FROM radio_calls
    $DATE_WHERE
    GROUP BY EXTRACT(HOUR FROM call_timestamp)
    ORDER BY hour;" \
    "⏰ HOURLY DISTRIBUTION"
fi

echo -e "\n${BOLD}${GREEN}═══════════════════════════════════════════════════════════════════════════${NC}"
echo -e "${BOLD}${CYAN}Query completed at: $(date '+%Y-%m-%d %H:%M:%S')${NC}"
echo -e "${GREEN}═══════════════════════════════════════════════════════════════════════════${NC}\n"