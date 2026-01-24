#!/bin/bash
# Post-tool hook: Run linters after Write/Edit operations
# Provides warnings but doesn't block (informational)

set -e

# Parse JSON input from stdin
input=$(cat)
file_path=$(echo "$input" | jq -r '.tool_input.file_path // empty' 2>/dev/null)

# Exit silently if no file path
if [[ -z "$file_path" ]]; then
    exit 0
fi

# Get project root (look for Cargo.toml)
project_root="$CLAUDE_PROJECT_DIR"
if [[ -z "$project_root" ]]; then
    # Try to find it
    dir=$(dirname "$file_path")
    while [[ "$dir" != "/" ]]; do
        if [[ -f "$dir/Cargo.toml" ]] && [[ -d "$dir/crates" ]]; then
            project_root="$dir"
            break
        fi
        dir=$(dirname "$dir")
    done
fi

# Rust files: run clippy on the specific file's crate
if [[ "$file_path" =~ \.rs$ ]]; then
    # Find which crate this file belongs to
    crate_dir=$(dirname "$file_path")
    while [[ "$crate_dir" != "/" ]]; do
        if [[ -f "$crate_dir/Cargo.toml" ]]; then
            break
        fi
        crate_dir=$(dirname "$crate_dir")
    done

    if [[ -f "$crate_dir/Cargo.toml" ]]; then
        crate_name=$(grep -m1 '^name' "$crate_dir/Cargo.toml" | sed 's/.*= *"\([^"]*\)".*/\1/' 2>/dev/null || echo "")
        if [[ -n "$crate_name" ]]; then
            # Run clippy and capture warnings (non-blocking)
            cd "$project_root" 2>/dev/null || exit 0
            warnings=$(cargo clippy -p "$crate_name" --message-format=short 2>&1 | grep -E "^(warning|error)" | head -5 || true)
            if [[ -n "$warnings" ]]; then
                echo "Clippy notes for $crate_name:"
                echo "$warnings"
            fi
        fi
    fi
fi

# TypeScript/TSX files: run oxlint
if [[ "$file_path" =~ \.(ts|tsx)$ ]]; then
    ui_dir="$project_root/ui"
    if [[ -d "$ui_dir" ]] && [[ -f "$ui_dir/package.json" ]]; then
        cd "$ui_dir" 2>/dev/null || exit 0
        # Check if oxlint is available
        if [[ -f "node_modules/.bin/oxlint" ]]; then
            warnings=$(./node_modules/.bin/oxlint "$file_path" 2>&1 | head -10 || true)
            if [[ -n "$warnings" ]] && [[ ! "$warnings" =~ "Found 0" ]]; then
                echo "oxlint notes:"
                echo "$warnings"
            fi
        fi
    fi
fi

exit 0
