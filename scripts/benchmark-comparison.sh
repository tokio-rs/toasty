#!/bin/bash

# Benchmark Comparison Script
set -e

echo "🔍 Association Performance Comparison"
echo "======================================"

RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OPTIMIZED_DIR="$(dirname "$SCRIPT_DIR")"
BASELINE_DIR="$(dirname "$OPTIMIZED_DIR")/toasty-baseline"

FEATURE=${1:-sqlite}

echo -e "${BLUE}Testing with feature: ${FEATURE}${NC}"
echo ""

run_benchmark() {
    local dir=$1
    local label=$2
    
    echo -e "${YELLOW}Running ${label} benchmark...${NC}"
    cd "$dir"
    
    local output=$(cargo bench --features "$FEATURE" 2>&1)
    
    local timings=$(echo "$output" | grep -E "time:\s+\[.*\]" | sed 's/.*time: *\[\([^]]*\)\].*/\1/')
    
    echo "$timings"
}

echo -e "${RED}📊 BASELINE (Before Optimization)${NC}"
echo "Commit: 362c388 (improve infer_ty coverage)"
baseline_results=$(run_benchmark "$BASELINE_DIR" "BASELINE")
echo "$baseline_results"
echo ""

echo -e "${GREEN}🚀 OPTIMIZED (After Optimization)${NC}"
echo "Commit: $(cd "$OPTIMIZED_DIR" && git rev-parse --short HEAD) ($(cd "$OPTIMIZED_DIR" && git log -1 --pretty=format:'%s'))"
optimized_results=$(run_benchmark "$OPTIMIZED_DIR" "OPTIMIZED")
echo "$optimized_results"
echo ""

echo ""
echo -e "${YELLOW}To run with different database:${NC}"
echo "  ./scripts/benchmark-comparison.sh postgresql"
echo "  ./scripts/benchmark-comparison.sh mysql"
echo "  ./scripts/benchmark-comparison.sh dynamodb"
echo ""
echo -e "${BLUE}Setup:${NC}"
echo "  1. Run 'git worktree add ../toasty-baseline 362c388' to create baseline"
echo "  2. Copy benches/ directory to baseline: 'cp -r benches ../toasty-baseline/'"
echo "  3. Add 'benches' to workspace members in ../toasty-baseline/Cargo.toml"
