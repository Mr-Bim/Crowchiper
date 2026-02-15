#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
WASM_DIR="$SCRIPT_DIR/wasm"
mkdir -p "$WASM_DIR"

echo "Building all test plugins..."
(cd "$SCRIPT_DIR" && cargo build --examples --release --target wasm32-wasip2)

EXAMPLES_DIR="$SCRIPT_DIR/target/wasm32-wasip2/release/examples"
OUTPUT=$(find examples -maxdepth 1 -type f -name "*.rs" -exec basename {} .rs \;)
for name in $OUTPUT; do
    dest_name="${name//_/-}"
    cp "$EXAMPLES_DIR/${name}.wasm" "$WASM_DIR/${dest_name}.wasm"
done

echo "All plugins built → $WASM_DIR/"
