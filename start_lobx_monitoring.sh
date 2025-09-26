#!/bin/bash

# Start LOBX + Monitoring Stack
# This script starts everything you need for monitoring

echo "🚀 Starting LOBX + Monitoring Stack"
echo "===================================="
echo ""

# Start the monitoring stack first
echo "📊 Starting monitoring stack..."
docker-compose -f docker-compose.monitoring.yml up -d

# Wait a moment for services to start
echo "⏳ Waiting for services to start..."
sleep 5

# Check if monitoring is running
if docker-compose -f docker-compose.monitoring.yml ps | grep -q "Up"; then
    echo "✅ Monitoring stack is running"
else
    echo "❌ Failed to start monitoring stack"
    exit 1
fi

echo ""
echo "🎯 Starting LOBX application..."
echo "   (This will run in the foreground - use Ctrl+C to stop)"
echo ""

# Start LOBX application
cargo run --features metrics-exporter
