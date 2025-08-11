#!/bin/bash
# Test script for SDRTrunk API server

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

API_URL="http://localhost:8080"

echo -e "${GREEN}Testing SDRTrunk API Server${NC}"
echo "================================"

# Test 1: Health check
echo -e "\n${YELLOW}Test 1: Health Check${NC}"
response=$(curl -s -w "\n%{http_code}" $API_URL/health)
http_code=$(echo "$response" | tail -1)
body=$(echo "$response" | sed '$d')

if [ "$http_code" = "200" ]; then
    echo -e "${GREEN}✓ Health check passed${NC}"
    echo "$body" | jq -r '.status, .database.connected' | while read -r line; do
        echo "  - $line"
    done
else
    echo -e "${RED}✗ Health check failed (HTTP $http_code)${NC}"
fi

# Test 2: List calls (should be empty)
echo -e "\n${YELLOW}Test 2: List Calls${NC}"
response=$(curl -s -w "\n%{http_code}" $API_URL/api/calls)
http_code=$(echo "$response" | tail -n1)
body=$(echo "$response" | head -n-1)

if [ "$http_code" = "200" ]; then
    total=$(echo "$body" | jq -r '.total')
    echo -e "${GREEN}✓ List calls successful${NC}"
    echo "  - Total calls: $total"
else
    echo -e "${RED}✗ List calls failed (HTTP $http_code)${NC}"
fi

# Test 3: Get system stats (should be empty)
echo -e "\n${YELLOW}Test 3: System Statistics${NC}"
response=$(curl -s -w "\n%{http_code}" $API_URL/api/systems/test_system/stats)
http_code=$(echo "$response" | tail -n1)

if [ "$http_code" = "404" ]; then
    echo -e "${GREEN}✓ System stats correctly returns 404 for non-existent system${NC}"
else
    echo -e "${RED}✗ Unexpected response (HTTP $http_code)${NC}"
fi

# Test 4: Ready check
echo -e "\n${YELLOW}Test 4: Readiness Check${NC}"
response=$(curl -s -w "\n%{http_code}" $API_URL/ready)
http_code=$(echo "$response" | tail -n1)
body=$(echo "$response" | head -n-1)

if [ "$http_code" = "200" ]; then
    echo -e "${GREEN}✓ Readiness check passed${NC}"
    echo "$body" | jq -r '.database, .storage' | while read -r line; do
        echo "  - $line"
    done
else
    echo -e "${RED}✗ Readiness check failed (HTTP $http_code)${NC}"
fi

# Test 5: Database connection
echo -e "\n${YELLOW}Test 5: Database Connection${NC}"
if podman exec sdrtrunk-postgres psql -U sdrtrunk -d sdrtrunk_transcriber -c "SELECT COUNT(*) FROM radio_calls;" > /dev/null 2>&1; then
    echo -e "${GREEN}✓ Database connection successful${NC}"
    count=$(podman exec sdrtrunk-postgres psql -U sdrtrunk -d sdrtrunk_transcriber -t -c "SELECT COUNT(*) FROM radio_calls;" | tr -d ' ')
    echo "  - Radio calls in database: $count"
else
    echo -e "${RED}✗ Database connection failed${NC}"
fi

echo -e "\n${GREEN}================================${NC}"
echo -e "${GREEN}API Server Test Complete!${NC}"
echo -e "${GREEN}================================${NC}"

# Summary
echo -e "\n${YELLOW}Summary:${NC}"
echo "  - API Server: Running on $API_URL"
echo "  - Database: PostgreSQL 17 on localhost:5432"
echo "  - Ready for file uploads and monitoring"

echo -e "\n${YELLOW}Next Steps:${NC}"
echo "  1. Create sample MP3 files in /tmp/sdrtrunk/watch"
echo "  2. Run the file monitor: cargo run --bin sdrtrunk-monitor"
echo "  3. Upload files via API: curl -X POST $API_URL/api/call-upload -F audio=@file.mp3"