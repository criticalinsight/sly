#!/bin/bash
export SLY_MODEL="qwen3.5:latest"
export SLY_OPENAI_URL="http://localhost:11434/v1/chat/completions"
mkdir -p /tmp/sly_test_flask_app
cd /tmp/sly_test_flask_app
rm -rf *
echo "Build a Python Flask web app with an HTML/JS frontend and SQLite backend for a real-time chat room. You must create the virtual environment (python3 -m venv venv), install requirements (Flask), write app.py, models.py, and templates/index.html. Run the server in a background job." | /Users/brixelectronics/Downloads/sly/target/release/sly
