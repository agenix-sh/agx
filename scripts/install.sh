#!/usr/bin/env sh
set -e

REPO="agenix-sh/agx"
PROJECT_NAME="agx"

info() {
  printf '%s\n' "[agx-install] $*"
}

warn() {
  printf '%s\n' "[agx-install] WARNING: $*" >&2
}

fail() {
  printf '%s\n' "[agx-install] ERROR: $*" >&2
  exit 1
}

detect_os() {
  case "$(uname -s)" in
    Linux) echo "unknown-linux-gnu" ;;
    Darwin) echo "apple-darwin" ;;
    *)
      echo "unknown"
      ;;
  esac
}

detect_arch() {
  case "$(uname -m)" in
    x86_64) echo "x86_64" ;;
    arm64|aarch64) echo "aarch64" ;;
    *)
      echo "unknown"
      ;;
  esac
}

detect_target() {
  os="$(detect_os)"
  arch="$(detect_arch)"

  if [ "$os" = "unknown" ] || [ "$arch" = "unknown" ]; then
    echo "unknown"
  else
    echo "${arch}-${os}"
  fi
}

detect_downloader() {
  if command -v curl >/dev/null 2>&1; then
    echo "curl"
  elif command -v wget >/dev/null 2>&1; then
    echo "wget"
  else
    echo "none"
  fi
}

download_file() {
  url="$1"
  dest="$2"

  downloader="$(detect_downloader)"

  case "$downloader" in
    curl)
      curl -fsSL "$url" -o "$dest" ;;
    wget)
      wget -qO "$dest" "$url" ;;
    *)
      fail "Neither curl nor wget is available; please install one of them."
      ;;
  esac
}

choose_install_dir() {
  if [ -w /usr/local/bin ]; then
    echo "/usr/local/bin"
    return
  fi

  if [ -d "$HOME/.local/bin" ] || mkdir -p "$HOME/.local/bin" 2>/dev/null; then
    echo "$HOME/.local/bin"
    return
  fi

  if [ -d "$HOME/bin" ] || mkdir -p "$HOME/bin" 2>/dev/null; then
    echo "$HOME/bin"
    return
  fi

  fail "Could not find a writable install directory."
}

ensure_on_path_message() {
  dir="$1"

  case ":$PATH:" in
    *:"$dir":*)
      ;;
    *)
      warn "Directory '$dir' is not on your PATH."
      warn "Add the following line to your shell configuration (e.g. ~/.bashrc or ~/.zshrc):"
      warn "  export PATH=\"$dir:\$PATH\""
      ;;
  esac
}

install_from_binary() {
  target="$(detect_target)"

  if [ "$target" = "unknown" ]; then
    warn "Unsupported OS/architecture combination; falling back to source install if available."
    return 1
  fi

  version="${AGX_VERSION:-latest}"

  if [ "$version" = "latest" ]; then
    url="https://github.com/${REPO}/releases/latest/download/${PROJECT_NAME}-${target}"
  else
    url="https://github.com/${REPO}/releases/download/${version}/${PROJECT_NAME}-${target}"
  fi

  info "Attempting to download binary for target '$target' from:"
  info "  $url"

  tmpfile="$(mktemp "/tmp/${PROJECT_NAME}.XXXXXXXX")"

  if ! download_file "$url" "$tmpfile"; then
    warn "Failed to download binary for target '$target'."
    rm -f "$tmpfile"
    return 1
  fi

  chmod +x "$tmpfile"

  instdir="$(choose_install_dir)"
  info "Installing ${PROJECT_NAME} to: $instdir"

  install_path="${instdir}/${PROJECT_NAME}"

  mv "$tmpfile" "$install_path"

  ensure_on_path_message "$instdir"

  info "Installed ${PROJECT_NAME} to '$install_path'."
  info "You can update later by re-running this installer."
}

install_from_source() {
  if ! command -v cargo >/dev/null 2>&1; then
    fail "Rust (cargo) is not installed and no prebuilt binary was available."
  fi

  info "Installing from source using cargo (this may take a while)..."

  if cargo install "$PROJECT_NAME" >/dev/null 2>&1; then
    :
  else
    cargo install --git "https://github.com/${REPO}.git" --locked "$PROJECT_NAME"
  fi

  info "Source installation complete. Make sure cargo's bin directory is on your PATH:"
  info "  export PATH=\"\$HOME/.cargo/bin:\$PATH\""
}

check_recommended_tools() {
  if ! command -v jq >/dev/null 2>&1; then
    warn "jq is not installed. Some JSON-related plans may fail."
    if command -v apt-get >/dev/null 2>&1; then
      warn "On Ubuntu, you can install it with: sudo apt-get install jq"
    elif command -v brew >/dev/null 2>&1; then
      warn "On macOS with Homebrew, you can install it with: brew install jq"
    fi
  fi

  if ! command -v ollama >/dev/null 2>&1; then
    warn "ollama is not installed or not on PATH. The planner will not work until a model runtime is available."
    warn "Visit https://ollama.com/download for installation instructions, or configure another backend when available."
  fi
}

main() {
  info "Starting ${PROJECT_NAME} installer..."

  if ! install_from_binary; then
    info "Falling back to installation from source."
    install_from_source
  fi

  check_recommended_tools

  info "Installation finished."
}

main "$@"

