#!/bin/bash
# Database management script for SDRTrunk Transcriber

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
DB_CONTAINER="sdrtrunk-postgres"
DB_USER="sdrtrunk"
DB_PASSWORD="sdrtrunk_dev_password"
DB_NAME="sdrtrunk_transcriber"
DB_HOST="localhost"
DB_PORT="5432"

# Check if podman is available, otherwise use docker
if command -v podman &> /dev/null; then
    CONTAINER_CMD="podman"
    COMPOSE_CMD="podman-compose"
    echo -e "${GREEN}Using Podman${NC}"
else
    CONTAINER_CMD="docker"
    COMPOSE_CMD="docker-compose"
    echo -e "${GREEN}Using Docker${NC}"
fi

# Function to print usage
usage() {
    echo "Usage: $0 {start|stop|restart|status|logs|shell|migrate|reset|test}"
    echo ""
    echo "Commands:"
    echo "  start    - Start the PostgreSQL container"
    echo "  stop     - Stop the PostgreSQL container"
    echo "  restart  - Restart the PostgreSQL container"
    echo "  status   - Show container status"
    echo "  logs     - Show container logs"
    echo "  shell    - Open psql shell to database"
    echo "  migrate  - Run database migrations"
    echo "  reset    - Reset database (WARNING: destroys all data)"
    echo "  test     - Test database connection"
    exit 1
}

# Function to start database
start_db() {
    echo -e "${GREEN}Starting PostgreSQL container...${NC}"
    
    # Use appropriate compose file
    if [ "$CONTAINER_CMD" = "podman" ]; then
        $COMPOSE_CMD -f podman-compose.yml up -d postgres
    else
        $COMPOSE_CMD -f docker-compose.yml up -d postgres
    fi
    
    echo -e "${GREEN}Waiting for PostgreSQL to be ready...${NC}"
    sleep 3
    
    # Wait for database to be ready
    for i in {1..30}; do
        if $CONTAINER_CMD exec $DB_CONTAINER pg_isready -U $DB_USER -d $DB_NAME &> /dev/null; then
            echo -e "${GREEN}PostgreSQL is ready!${NC}"
            return 0
        fi
        echo -n "."
        sleep 1
    done
    
    echo -e "${RED}PostgreSQL failed to start${NC}"
    return 1
}

# Function to stop database
stop_db() {
    echo -e "${YELLOW}Stopping PostgreSQL container...${NC}"
    $CONTAINER_CMD stop $DB_CONTAINER || true
}

# Function to restart database
restart_db() {
    stop_db
    start_db
}

# Function to show status
show_status() {
    echo -e "${GREEN}Container Status:${NC}"
    $CONTAINER_CMD ps --filter name=$DB_CONTAINER
}

# Function to show logs
show_logs() {
    $CONTAINER_CMD logs -f $DB_CONTAINER
}

# Function to open database shell
db_shell() {
    echo -e "${GREEN}Opening PostgreSQL shell...${NC}"
    $CONTAINER_CMD exec -it $DB_CONTAINER psql -U $DB_USER -d $DB_NAME
}

# Function to run migrations
run_migrations() {
    echo -e "${GREEN}Running database migrations...${NC}"
    
    # Check if sqlx-cli is installed
    if ! command -v sqlx &> /dev/null; then
        echo -e "${YELLOW}sqlx-cli not found. Installing...${NC}"
        cargo install sqlx-cli --no-default-features --features postgres
    fi
    
    # Set database URL
    export DATABASE_URL="postgresql://$DB_USER:$DB_PASSWORD@$DB_HOST:$DB_PORT/$DB_NAME"
    
    # Run migrations
    cd crates/sdrtrunk-database
    sqlx migrate run
    cd ../..
    
    echo -e "${GREEN}Migrations completed!${NC}"
}

# Function to reset database
reset_db() {
    echo -e "${RED}WARNING: This will destroy all data in the database!${NC}"
    read -p "Are you sure? (y/N) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        echo "Cancelled"
        return 1
    fi
    
    echo -e "${YELLOW}Resetting database...${NC}"
    
    # Stop container
    stop_db
    
    # Remove volume
    if [ "$CONTAINER_CMD" = "podman" ]; then
        $CONTAINER_CMD volume rm postgres_data 2>/dev/null || true
    else
        $CONTAINER_CMD volume rm rs-sdrtrunk-transcriber_postgres_data 2>/dev/null || true
    fi
    
    # Start fresh
    start_db
    
    # Run migrations
    run_migrations
    
    echo -e "${GREEN}Database reset complete!${NC}"
}

# Function to test connection
test_connection() {
    echo -e "${GREEN}Testing database connection...${NC}"
    
    if $CONTAINER_CMD exec $DB_CONTAINER pg_isready -U $DB_USER -d $DB_NAME; then
        echo -e "${GREEN}Connection successful!${NC}"
        
        # Test with psql
        echo -e "${GREEN}Database version:${NC}"
        $CONTAINER_CMD exec $DB_CONTAINER psql -U $DB_USER -d $DB_NAME -c "SELECT version();"
        
        # Show tables
        echo -e "${GREEN}Database tables:${NC}"
        $CONTAINER_CMD exec $DB_CONTAINER psql -U $DB_USER -d $DB_NAME -c "\dt"
    else
        echo -e "${RED}Connection failed!${NC}"
        return 1
    fi
}

# Main script
case "$1" in
    start)
        start_db
        ;;
    stop)
        stop_db
        ;;
    restart)
        restart_db
        ;;
    status)
        show_status
        ;;
    logs)
        show_logs
        ;;
    shell)
        db_shell
        ;;
    migrate)
        run_migrations
        ;;
    reset)
        reset_db
        ;;
    test)
        test_connection
        ;;
    *)
        usage
        ;;
esac