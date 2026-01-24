#!/bin/bash
# Post-tool hook: Auto-format Rust files after Write/Edit operations

set -e

# Parse JSON input from stdin
input=$(cat)
file_path=$(echo "$input" | jq -r '.tool_input.file_path // empty' 2>/dev/null)

# Exit silently if no file path
if [[ -z "$file_path" ]]; then
    exit 0
fi

# Only process Rust files
if [[ ! "$file_path" =~ \.rs$ ]]; then
    exit 0
fi

# Check if file exists
if [[ ! -f "$file_path" ]]; then
    exit 0
fi

# Run rustfmt if available
if command -v rustfmt &> /dev/null; then
    if rustfmt --edition 2021 "$file_path" 2>/dev/null; then
        echo "Formatted: $file_path"
    fi
fi

exit 0
