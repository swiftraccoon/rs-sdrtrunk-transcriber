#!/bin/bash
# Main script to run local CI/CD tests

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}
╔══════════════════════════════════════════╗
║     Local CI/CD Test Environment         ║
║     SDRTrunk Transcriber Project         ║
╚══════════════════════════════════════════╝
${NC}"

# Parse command line arguments
QUICK_MODE=false
COVERAGE=false
BENCHMARKS=false
MIRI=false
CLEAN=false

while [[ $# -gt 0 ]]; do
    case $1 in
        --quick|-q)
            QUICK_MODE=true
            shift
            ;;
        --coverage|-c)
            COVERAGE=true
            shift
            ;;
        --benchmarks|-b)
            BENCHMARKS=true
            shift
            ;;
        --miri|-m)
            MIRI=true
            shift
            ;;
        --clean)
            CLEAN=true
            shift
            ;;
        --help|-h)
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  -q, --quick       Run only essential checks (format, clippy, test)"
            echo "  -c, --coverage    Include code coverage analysis"
            echo "  -b, --benchmarks  Run benchmarks"
            echo "  -m, --miri        Run Miri memory safety checks (slow)"
            echo "  --clean           Clean up containers and volumes before running"
            echo "  -h, --help        Show this help message"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            echo "Use --help for usage information"
            exit 1
            ;;
    esac
done

# Check if podman is installed
if ! command -v podman &> /dev/null; then
    echo -e "${RED}Error: Podman is not installed${NC}"
    echo "Please install podman first: https://podman.io/getting-started/installation"
    exit 1
fi

# Use podman compose (which wraps docker-compose or podman-compose)
COMPOSE_CMD="podman compose"

# Change to script directory
cd "$(dirname "$0")"

# Clean up if requested
if [ "$CLEAN" = true ]; then
    echo -e "${YELLOW}Cleaning up existing containers and volumes...${NC}"
    $COMPOSE_CMD down -v || true
    podman system prune -f || true
fi

# Export environment variables
export RUN_COVERAGE=$COVERAGE
export RUN_BENCHMARKS=$BENCHMARKS
export RUN_MIRI=$MIRI

# Build containers
echo -e "${YELLOW}Building CI containers...${NC}"
$COMPOSE_CMD build

# Start services
echo -e "${YELLOW}Starting CI environment...${NC}"
$COMPOSE_CMD up --abort-on-container-exit --exit-code-from ci-runner

# Capture exit code
EXIT_CODE=$?

# Clean up
echo -e "${YELLOW}Cleaning up...${NC}"
$COMPOSE_CMD down

# Report results
echo ""
if [ $EXIT_CODE -eq 0 ]; then
    echo -e "${GREEN}
╔══════════════════════════════════════════╗
║          All CI Checks Passed! ✓         ║
║      Your code is ready to commit.       ║
╚══════════════════════════════════════════╝
${NC}"
else
    echo -e "${RED}
╔══════════════════════════════════════════╗
║         CI Checks Failed! ✗              ║
║    Please fix issues before committing.  ║
╚══════════════════════════════════════════╝
${NC}"
fi

exit $EXIT_CODE