#!/bin/bash

echo "🚀 Starting Ultra-Fast Trading Demo (Release + LTO)"
echo "=================================================="

# Kill any existing processes
pkill -f "lobx-rs" || true

# Start the demo with maximum optimizations
cargo run --profile release-lto --features metrics-exporter -- --unified-demo
