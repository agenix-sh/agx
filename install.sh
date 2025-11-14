#!/usr/bin/env sh
# This file is intended to be served via GitHub Pages at:
#   https://agenix.sh/install.sh
# It simply fetches the canonical installer from the main AGX repository
# and executes it.
set -e

REPO_RAW="https://raw.githubusercontent.com/agenix-sh/agx/main/scripts/install.sh"

if command -v curl >/dev/null 2>&1; then
  curl -fsSL "$REPO_RAW" | sh
elif command -v wget >/dev/null 2>&1; then
  wget -qO- "$REPO_RAW" | sh
else
  echo "[agx-install] ERROR: Neither curl nor wget is available." >&2
  exit 1
fi
