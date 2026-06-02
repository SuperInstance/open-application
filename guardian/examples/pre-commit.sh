#!/bin/bash
# Pre-commit hook: check binary size before committing.
# Install: cp examples/pre-commit.sh .git/hooks/pre-commit
#
# Or add to your project's package.json:
#   "scripts": { "size-check": "guardian check ./target/release/my-app --budget guardian-budget.toml" }

set -e

if ! command -v guardian &> /dev/null; then
    echo "⚠️  guardian not found. Install with: cargo install --path ./guardian"
    exit 0
fi

# Only run if a budget file exists
if [ ! -f "guardian-budget.toml" ]; then
    exit 0
fi

# Check if there's a release binary to analyze
BINARY=""
for candidate in target/release/my-app target/release/app target/release/guardian; do
    if [ -f "$candidate" ]; then
        BINARY="$candidate"
        break
    fi
done

if [ -z "$BINARY" ]; then
    echo "ℹ️  No release binary found. Skipping size check."
    exit 0
fi

echo "🔍 Checking binary size..."
guardian check "$BINARY" --budget guardian-budget.toml --save
echo "✅ Size check passed"
