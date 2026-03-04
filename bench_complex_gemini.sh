#!/bin/bash
export SLY_MODEL="gemini-2.5-flash"
unset SLY_OPENAI_URL
mkdir -p /tmp/sly_test_rust_cli
cd /tmp/sly_test_rust_cli
rm -rf *
echo "Build a Rust CLI tool using clap and reqwest. It should accept a markdown file, parse it to extract all HTTP/HTTPS URLs (you can use regex), ping them asynchronously to check if they are alive (200 OK), and print a summary. Initialize the cargo project first (e.g. cargo init), run 'cargo build' to verify everything works." | /Users/brixelectronics/Downloads/sly/target/release/sly
