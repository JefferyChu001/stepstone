#!/bin/bash

# GreptimeDB Stepstone Self-Test Tool Demo
# This script demonstrates the capabilities of the stepstone tool

# set -e  # Don't exit on errors so we can see all test results

echo "🚀 GreptimeDB Stepstone Self-Test Tool Demo"
echo "============================================="
echo

# Build the tool if not already built
if [ ! -f "./target/release/stepstone" ]; then
    echo "📦 Building stepstone tool..."
    cargo build --release
    echo "✅ Build complete"
    echo
fi

echo "🔍 Testing Metasrv Configuration..."
echo "-----------------------------------"
./target/release/stepstone metasrv -c examples/metasrv-etcd.toml
echo

echo "🔍 Testing Datanode Configuration (File Storage)..."
echo "---------------------------------------------------"
./target/release/stepstone datanode -c examples/datanode-file.toml
echo

echo "🔍 Testing Frontend Configuration..."
echo "-----------------------------------"
./target/release/stepstone frontend -c examples/frontend.toml
echo

echo "🔍 Testing Datanode Configuration (S3 Storage)..."
echo "-------------------------------------------------"
./target/release/stepstone datanode -c examples/datanode-s3.toml
echo

echo "📊 Testing JSON Output Format..."
echo "-------------------------------"
./target/release/stepstone datanode -c examples/datanode-file.toml --output json
echo

echo "⚡ Testing Performance Benchmarks..."
echo "-----------------------------------"
./target/release/stepstone datanode -c examples/datanode-file.toml --include-performance
echo

echo "🎉 Demo Complete!"
echo "================"
echo
echo "The stepstone tool provides comprehensive health checks for:"
echo "• Metasrv: etcd connectivity and operations"
echo "• Datanode: metasrv connectivity, storage validation, and performance tests"
echo "• Frontend: metasrv connectivity and server configuration validation"
echo
echo "Output formats:"
echo "• Human-readable (default)"
echo "• JSON (--output json)"
echo
echo "Additional features:"
echo "• Performance benchmarks (--include-performance)"
echo "• Detailed error messages with suggestions"
echo "• Duration tracking for all operations"
