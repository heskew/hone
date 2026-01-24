#!/bin/bash
# Seed test database with sample data for UX tests
# This script initializes a fresh test database with realistic data

set -e

# Configuration
TEST_DB="${TEST_DB:-/tmp/hone-ux-test.db}"
HONE_BIN="${HONE_BIN:-cargo run --}"
PROJECT_ROOT="${PROJECT_ROOT:-$(dirname "$0")/../..}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${YELLOW}Seeding UX test database: $TEST_DB${NC}"

# Remove existing test database
if [ -f "$TEST_DB" ]; then
    echo "Removing existing test database..."
    rm "$TEST_DB"
fi

cd "$PROJECT_ROOT"

# Initialize database
echo -e "${GREEN}Initializing database...${NC}"
$HONE_BIN --no-encrypt --db "$TEST_DB" init

# Import test data (this runs tagging and detection automatically)
echo -e "${GREEN}Importing test transactions (with tagging and detection)...${NC}"
$HONE_BIN --no-encrypt --db "$TEST_DB" import --file samples/test_data.csv --bank chase

# Verify data was loaded
echo -e "${GREEN}Verifying seeded data...${NC}"
$HONE_BIN --no-encrypt --db "$TEST_DB" status

echo -e "${GREEN}Database seeded successfully!${NC}"
echo -e "Test DB location: ${YELLOW}$TEST_DB${NC}"
