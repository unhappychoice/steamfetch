#!/bin/bash
set -e

REPO="unhappychoice/steamfetch"
BINARY_NAME="steamfetch"

get_latest_release() {
    curl -s "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/'
}

check_glibc_version() {
    if ! command -v ldd &> /dev/null; then
        return
    fi

    GLIBC_VERSION=$(ldd --version 2>&1 | head -n1 | grep -oE '[0-9]+\.[0-9]+' | head -1)
    REQUIRED_VERSION="2.35"

    if [ -n "$GLIBC_VERSION" ]; then
        if [ "$(printf '%s\n' "$REQUIRED_VERSION" "$GLIBC_VERSION" | sort -V | head -n1)" != "$REQUIRED_VERSION" ]; then
            echo ""
            echo "WARNING: Your glibc version ($GLIBC_VERSION) is older than $REQUIRED_VERSION."
            echo "The pre-built binary may not work on your system."
            echo ""
            echo "Options:"
            echo "  1. Upgrade your OS to get a newer glibc"
            echo "  2. Build from source: cargo install steamfetch"
            echo "  3. Continue anyway and see if it works"
            echo ""
            read -p "Continue with installation? [y/N] " -n 1 -r
            echo
            if [[ ! $REPLY =~ ^[Yy]$ ]]; then
                echo "Installation cancelled."
                exit 0
            fi
        fi
    fi
}

detect_platform() {
    OS="$(uname -s)"
    ARCH="$(uname -m)"

    case "$OS" in
        Linux*)
            check_glibc_version
            case "$ARCH" in
                x86_64)
                    echo "x86_64-unknown-linux-gnu"
                    ;;
                *)
                    echo "Unsupported architecture: $ARCH (Steam only supports x86_64 on Linux)" >&2
                    exit 1
                    ;;
            esac
            ;;
        Darwin*)
            case "$ARCH" in
                x86_64)
                    echo "x86_64-apple-darwin"
                    ;;
                arm64)
                    echo "aarch64-apple-darwin"
                    ;;
                *)
                    echo "Unsupported architecture: $ARCH" >&2
                    exit 1
                    ;;
            esac
            ;;
        MINGW*|MSYS*|CYGWIN*)
            echo "x86_64-pc-windows-msvc"
            ;;
        *)
            echo "Unsupported OS: $OS" >&2
            exit 1
            ;;
    esac
}

main() {
    VERSION="${1:-$(get_latest_release)}"
    PLATFORM="$(detect_platform)"

    if [ -z "$VERSION" ]; then
        echo "Failed to detect latest version" >&2
        exit 1
    fi

    echo "Installing $BINARY_NAME $VERSION for $PLATFORM..."

    TEMP_DIR="$(mktemp -d)"
    trap 'rm -rf "$TEMP_DIR"' EXIT

    if [[ "$PLATFORM" == *"windows"* ]]; then
        DOWNLOAD_URL="https://github.com/$REPO/releases/download/$VERSION/${BINARY_NAME}-${VERSION}-${PLATFORM}.zip"
        echo "Downloading from $DOWNLOAD_URL..."
        curl -sL "$DOWNLOAD_URL" -o "$TEMP_DIR/steamfetch.zip"
        unzip -q "$TEMP_DIR/steamfetch.zip" -d "$TEMP_DIR"
        rm "$TEMP_DIR/steamfetch.zip"
        INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"
    else
        DOWNLOAD_URL="https://github.com/$REPO/releases/download/$VERSION/${BINARY_NAME}-${VERSION}-${PLATFORM}.tar.gz"
        echo "Downloading from $DOWNLOAD_URL..."
        curl -sL "$DOWNLOAD_URL" | tar xz -C "$TEMP_DIR"
        INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"
    fi

    mkdir -p "$INSTALL_DIR"

    # Install binary and Steam API library
    mv "$TEMP_DIR"/* "$INSTALL_DIR/"
    
    if [[ "$PLATFORM" != *"windows"* ]]; then
        chmod +x "$INSTALL_DIR/$BINARY_NAME"
    fi

    echo "Successfully installed $BINARY_NAME to $INSTALL_DIR/"
    echo ""
    echo "Make sure $INSTALL_DIR is in your PATH:"
    if [[ "$PLATFORM" == *"windows"* ]]; then
        echo "  Add $INSTALL_DIR to your PATH environment variable"
    else
        echo "  export PATH=\"\$PATH:$INSTALL_DIR\""
    fi
    echo ""
    echo "Setup Steam API key:"
    echo "  1. Get your API key from: https://steamcommunity.com/dev/apikey"
    echo "  2. Find your Steam ID from: https://steamid.io"
    echo "  3. Set environment variables:"
    if [[ "$PLATFORM" == *"windows"* ]]; then
        echo "     set STEAM_API_KEY=your_api_key"
        echo "     set STEAM_ID=your_steam_id"
    else
        echo "     export STEAM_API_KEY=\"your_api_key\""
        echo "     export STEAM_ID=\"your_steam_id\""
    fi
}

main "$@"
