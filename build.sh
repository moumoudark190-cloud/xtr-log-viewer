#!/usr/bin/env bash
set -e
echo "═══════════════════════════════════════"
echo "  XTR Log Viewer — build"
echo "═══════════════════════════════════════"

if ! command -v cargo &>/dev/null; then
  echo ""
  echo "Rust not found. Installing via rustup..."
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
  source "$HOME/.cargo/env"
fi

cargo build --release

BINARY="./target/release/logviewer"
if [[ "$OSTYPE" == "msys"* || "$OSTYPE" == "cygwin"* ]]; then
  BINARY="./target/release/logviewer.exe"
fi

echo ""
echo "✓ Build complete: $BINARY"
echo ""
echo "Usage:"
echo "  $BINARY                  # launch empty viewer"
echo "  $BINARY myfile.log       # open a file directly"
echo ""
echo "Then drag & drop .log files, or use Ctrl+O"
