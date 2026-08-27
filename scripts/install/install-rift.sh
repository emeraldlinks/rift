#!/bin/sh
set -eu

REPO="emeraldlinks/rift"
BINARY_NAME="rift"
INSTALL_DIR="${RIFT_INSTALL_DIR:-$HOME/.local/bin}"
GITHUB_API="https://api.github.com/repos/$REPO"

step() {
  printf '\033[1;36m==>\033[0m %s\n' "$1"
}

warn() {
  printf '\033[1;33mWARNING:\033[0m %s\n' "$1" >&2
}

die() {
  printf '\033[1;31mERROR:\033[0m %s\n' "$1" >&2
  exit 1
}

detect_platform() {
  os="$(uname -s)"
  arch="$(uname -m)"

  case "$os" in
    Linux)
      case "$arch" in
        x86_64)  echo "x86_64-unknown-linux-musl" ;;
        aarch64) echo "aarch64-unknown-linux-musl" ;;
        *) die "Unsupported architecture: $arch" ;;
      esac
      ;;
    Darwin)
      case "$arch" in
        x86_64)  echo "x86_64-apple-darwin" ;;
        arm64)   echo "aarch64-apple-darwin" ;;
        *) die "Unsupported architecture: $arch" ;;
      esac
      ;;
    *) die "Unsupported OS: $os (use install.ps1 for Windows)" ;;
  esac
}

fetch_latest_version() {
  curl -fsSL "$GITHUB_API/releases/latest" | grep '"tag_name"' | sed -E 's/.*"tag_name": *"([^"]+)".*/\1/' | head -1
}

fetch_version() {
  tag="$1"
  if [ "$tag" = "latest" ]; then
    fetch_latest_version
  else
    printf '%s' "$tag"
  fi
}

download_and_install() {
  version="$1"
  platform="$2"
  asset="rift-${platform}.tar.gz"
  url="https://github.com/$REPO/releases/download/${version}/${asset}"

  tmp_dir="$(mktemp -d)"
  trap 'rm -rf "$tmp_dir"' EXIT

  step "Downloading $url"
  if ! curl -fsSL "$url" -o "$tmp_dir/$asset"; then
    die "Failed to download $url"
  fi

  step "Extracting to $INSTALL_DIR"
  mkdir -p "$INSTALL_DIR"
  tar xzf "$tmp_dir/$asset" -C "$INSTALL_DIR"
  chmod +x "$INSTALL_DIR/$BINARY_NAME"

  step "Installed $BINARY_NAME to $INSTALL_DIR/$BINARY_NAME"
}

check_path() {
  case ":$PATH:" in
    *":$INSTALL_DIR:"*) return 0 ;;
    *) return 1 ;;
  esac
}

main() {
  version="${1:-latest}"
  platform="$(detect_platform)"

  step "Detected platform: $platform"
  version="$(fetch_version "$version")"
  step "Resolved version: $version"

  download_and_install "$version" "$platform"

  if ! check_path; then
    warn "$INSTALL_DIR is not in your PATH"
    printf '\nAdd it to your shell profile:\n'
    printf '  export PATH="%s:$PATH"\n\n' "$INSTALL_DIR"
    printf 'Or run now:\n'
    printf '  export PATH="%s:$PATH" && %s --version\n\n' "$INSTALL_DIR" "$BINARY_NAME"
  else
    step "Verifying installation"
    "$INSTALL_DIR/$BINARY_NAME" --version || true
  fi
}

main "$@"
