#!/bin/bash
#
# Hone Model Training Script for Mac
#
# This script fetches training data from the Hone server (running on Pi via Tailscale)
# and runs MLX fine-tuning locally on Apple Silicon.
#
# Prerequisites:
#   - Python 3.10+ with mlx-lm installed: pip install mlx-lm
#   - Ollama installed and running
#   - Tailscale connected to the hone network
#   - Access to the Hone API server
#
# Usage:
#   ./train.sh --task classify_merchant --branch main
#   ./train.sh --task normalize_merchant --base-model gemma3:4b
#   ./train.sh --list  # Show available tasks and data counts
#
# Environment variables:
#   HONE_API_URL - Base URL of Hone server (e.g., http://192.168.1.x:3000)
#   HONE_API_KEY - API key for authentication (generate with: openssl rand -hex 32)
#   HONE_TRAINING_DIR - Directory for training artifacts (default: ~/.hone/training)

set -e

# Default configuration
HONE_API_URL="${HONE_API_URL:-}"
HONE_API_KEY="${HONE_API_KEY:-}"
HONE_TRAINING_DIR="${HONE_TRAINING_DIR:-$HOME/.hone/training}"
DEFAULT_BASE_MODEL="gemma3:12b"
TASK=""
BRANCH="main"
BASE_MODEL=""
LIST_ONLY=false
SKIP_TRAINING=false
SKIP_OLLAMA=false

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

print_usage() {
    cat << EOF
Hone Model Training Script

Usage: $0 [OPTIONS]

Options:
  --task TASK          Training task (classify_merchant, normalize_merchant, classify_subscription)
  --branch NAME        Experiment branch name (default: main)
  --base-model MODEL   Base Ollama model to fine-tune (default: gemma3:12b)
  --list               List available tasks and their training data counts
  --skip-training      Skip MLX training (use existing adapter)
  --skip-ollama        Skip Ollama model creation
  --api-url URL        Hone API URL (or set HONE_API_URL env var)
  --api-key KEY        API key for authentication (or set HONE_API_KEY env var)
  --help               Show this help message

Environment Variables:
  HONE_API_URL         Base URL of Hone server (e.g., http://192.168.1.x:3000)
  HONE_API_KEY         API key for authentication (generate: openssl rand -hex 32)
  HONE_TRAINING_DIR    Directory for training artifacts (default: ~/.hone/training)

Examples:
  $0 --list
  $0 --task classify_merchant --branch main
  $0 --task normalize_merchant --base-model gemma3:27b --branch experiment-1
EOF
}

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1" >&2
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --task)
            TASK="$2"
            shift 2
            ;;
        --branch)
            BRANCH="$2"
            shift 2
            ;;
        --base-model)
            BASE_MODEL="$2"
            shift 2
            ;;
        --list)
            LIST_ONLY=true
            shift
            ;;
        --skip-training)
            SKIP_TRAINING=true
            shift
            ;;
        --skip-ollama)
            SKIP_OLLAMA=true
            shift
            ;;
        --api-url)
            HONE_API_URL="$2"
            shift 2
            ;;
        --api-key)
            HONE_API_KEY="$2"
            shift 2
            ;;
        --help|-h)
            print_usage
            exit 0
            ;;
        *)
            log_error "Unknown option: $1"
            print_usage
            exit 1
            ;;
    esac
done

# Check required tools
check_requirements() {
    local missing=()

    if ! command -v curl &> /dev/null; then
        missing+=("curl")
    fi

    if ! command -v jq &> /dev/null; then
        missing+=("jq")
    fi

    if ! $LIST_ONLY && ! $SKIP_TRAINING; then
        if ! command -v python3 &> /dev/null; then
            missing+=("python3")
        fi

        if ! python3 -c "import mlx_lm" 2>/dev/null; then
            log_warn "mlx-lm not installed. Install with: pip install mlx-lm"
            missing+=("mlx-lm")
        fi
    fi

    if ! $LIST_ONLY && ! $SKIP_OLLAMA; then
        if ! command -v ollama &> /dev/null; then
            missing+=("ollama")
        fi
    fi

    if [ ${#missing[@]} -ne 0 ]; then
        log_error "Missing required tools: ${missing[*]}"
        exit 1
    fi
}

# Authenticated curl wrapper
api_curl() {
    local url="$1"
    shift
    if [ -n "$HONE_API_KEY" ]; then
        curl -s -H "Authorization: Bearer $HONE_API_KEY" "$url" "$@"
    else
        curl -s "$url" "$@"
    fi
}

# Validate API URL and authentication
check_api_url() {
    if [ -z "$HONE_API_URL" ]; then
        log_error "HONE_API_URL not set. Use --api-url or set the environment variable."
        log_info "Example: export HONE_API_URL='http://192.168.1.x:3000'"
        exit 1
    fi

    log_info "Checking API connection to $HONE_API_URL..."

    local response
    local http_code
    http_code=$(api_curl "${HONE_API_URL}/api/training/tasks" -w "%{http_code}" -o /dev/null 2>/dev/null)

    if [ "$http_code" = "401" ]; then
        log_error "Authentication failed (401 Unauthorized)"
        log_info "Set HONE_API_KEY or use --api-key with a valid API key"
        log_info "Generate a key on the server: openssl rand -hex 32"
        exit 1
    elif [ "$http_code" != "200" ]; then
        log_error "Cannot connect to Hone API at $HONE_API_URL (HTTP $http_code)"
        log_info "Make sure:"
        log_info "  1. Hone server is running"
        log_info "  2. The URL is correct"
        log_info "  3. Network connectivity is working"
        exit 1
    fi

    log_success "API connection OK"
    if [ -n "$HONE_API_KEY" ]; then
        log_info "  Authenticated via API key"
    fi
}

# List available training tasks
list_tasks() {
    log_info "Fetching training tasks from $HONE_API_URL..."

    local response
    response=$(api_curl "${HONE_API_URL}/api/training/tasks")

    if [ $? -ne 0 ]; then
        log_error "Failed to fetch tasks"
        exit 1
    fi

    echo ""
    echo "Available Training Tasks"
    echo "========================"
    echo ""

    echo "$response" | jq -r '.tasks[] |
        if .ready then "✅" else "⚠️" end + " " + .task +
        "\n   Total: " + (.total_examples | tostring) + " examples" +
        "\n   User corrections: " + (.user_corrections | tostring) +
        "\n   Ollama confirmed: " + (.ollama_confirmed | tostring) +
        "\n   Ready: " + (if .ready then "Yes" else "No (need " + (.min_required | tostring) + ")" end) +
        "\n"'

    echo ""
    log_info "Use --task <task_name> to start training"
}

# Fetch training data
fetch_training_data() {
    local task=$1
    local output_dir=$2

    log_info "Fetching training data for task: $task"

    mkdir -p "$output_dir"
    local data_file="$output_dir/training_data.jsonl"

    api_curl "${HONE_API_URL}/api/training/export?task=${task}" -o "$data_file"

    if [ ! -s "$data_file" ]; then
        log_error "No training data returned. Check if the task has enough examples."
        exit 1
    fi

    local count=$(wc -l < "$data_file" | tr -d ' ')
    log_success "Downloaded $count training examples to $data_file"

    echo "$data_file"
}

# Run MLX fine-tuning
run_mlx_training() {
    local data_file=$1
    local base_model=$2
    local output_dir=$3

    log_info "Starting MLX fine-tuning..."
    log_info "  Base model: $base_model"
    log_info "  Training data: $data_file"
    log_info "  Output: $output_dir/adapters"

    # Create adapter output directory
    mkdir -p "$output_dir/adapters"

    # MLX fine-tuning command
    # Uses LoRA with default parameters, suitable for small datasets
    python3 -m mlx_lm.lora \
        --model "$base_model" \
        --train \
        --data "$data_file" \
        --adapter-path "$output_dir/adapters" \
        --iters 100 \
        --batch-size 4 \
        --lora-layers 8

    if [ $? -ne 0 ]; then
        log_error "MLX training failed"
        exit 1
    fi

    log_success "Training completed. Adapter saved to $output_dir/adapters"
}

# Create Ollama model from adapter
create_ollama_model() {
    local task=$1
    local branch=$2
    local base_model=$3
    local adapter_dir=$4

    # Generate model name
    local timestamp=$(date +%Y%m%d_%H%M%S)
    local model_name="hone-${task//_/-}-${branch}-${timestamp}"

    log_info "Creating Ollama model: $model_name"

    # Create Modelfile
    local modelfile="$adapter_dir/Modelfile"
    cat > "$modelfile" << EOF
FROM $base_model
ADAPTER $adapter_dir/adapters
EOF

    # Create the model
    ollama create "$model_name" -f "$modelfile"

    if [ $? -ne 0 ]; then
        log_error "Failed to create Ollama model"
        exit 1
    fi

    log_success "Created Ollama model: $model_name"
    echo ""
    echo "To use this model:"
    echo "  export OLLAMA_MODEL=$model_name"
    echo ""
    echo "To test:"
    echo "  ollama run $model_name \"Classify: TRADER JOE'S #456 SEATTLE WA\""

    echo "$model_name"
}

# Main execution
main() {
    check_requirements
    check_api_url

    if $LIST_ONLY; then
        list_tasks
        exit 0
    fi

    # Validate task
    if [ -z "$TASK" ]; then
        log_error "No task specified. Use --task or --list to see available tasks."
        print_usage
        exit 1
    fi

    # Set default base model if not specified
    if [ -z "$BASE_MODEL" ]; then
        BASE_MODEL="$DEFAULT_BASE_MODEL"
    fi

    # Create experiment directory
    local timestamp=$(date +%Y%m%d_%H%M%S)
    local exp_dir="${HONE_TRAINING_DIR}/${TASK}/${BRANCH}/${timestamp}"
    mkdir -p "$exp_dir"

    log_info "Experiment directory: $exp_dir"

    # Fetch training data
    local data_file
    data_file=$(fetch_training_data "$TASK" "$exp_dir")

    if ! $SKIP_TRAINING; then
        # Run training
        run_mlx_training "$data_file" "$BASE_MODEL" "$exp_dir"
    else
        log_info "Skipping training (--skip-training)"
    fi

    if ! $SKIP_OLLAMA; then
        # Create Ollama model
        local model_name
        model_name=$(create_ollama_model "$TASK" "$BRANCH" "$BASE_MODEL" "$exp_dir")

        # Save experiment metadata
        cat > "$exp_dir/experiment.json" << EOF
{
    "task": "$TASK",
    "branch": "$BRANCH",
    "base_model": "$BASE_MODEL",
    "model_name": "$model_name",
    "created_at": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
    "training_data": "$data_file",
    "adapter_path": "$exp_dir/adapters"
}
EOF

        log_success "Experiment complete!"
        log_info "Metadata saved to $exp_dir/experiment.json"
    else
        log_info "Skipping Ollama model creation (--skip-ollama)"
    fi
}

main
