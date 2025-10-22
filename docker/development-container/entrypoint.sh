#!/bin/bash
set -euo pipefail

# Ensure HOME is set
if [ -z "${HOME:-}" ]; then
  export HOME="/home/builder"
fi

# Source cargo env if it exists (rustup creates this)
if [ -f "$HOME/.cargo/env" ]; then
  source "$HOME/.cargo/env"
elif [ -d "$HOME/.cargo/bin" ]; then
  # Fallback: add cargo bin to PATH if the directory exists
  export PATH="$HOME/.cargo/bin:$PATH"
fi

# If the workspace is owned by root because of a mounted volume from root, try to chown for the builder user if chown permitted
if [ "$(id -u)" -eq 0 ]; then
  # Running as root inside container: drop to builder (non-login to avoid .cargo/env sourcing issues)
  exec su builder -c "export PATH=/home/builder/.cargo/bin:\$PATH; $*"
fi

# If no arguments provided, open a bash shell
if [ $# -eq 0 ]; then
  exec bash
fi

# Run provided command
exec "$@"
