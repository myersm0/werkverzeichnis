#!/bin/sh
# Install script for wv (werkverzeichnis CLI)
# Usage: curl -fsSL https://raw.githubusercontent.com/myersm0/werkverzeichnis/main/install.sh | sh

set -e

REPO="myersm0/werkverzeichnis"
BINARY="wv"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"

# Detect OS and architecture
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

case "$OS" in
	linux)
		case "$ARCH" in
			x86_64) TARGET="wv-linux-x86_64" ;;
			*) echo "Unsupported architecture: $ARCH"; exit 1 ;;
		esac
		EXT="tar.gz"
		;;
	darwin)
		case "$ARCH" in
			x86_64) TARGET="wv-macos-x86_64" ;;
			arm64)  TARGET="wv-macos-arm64" ;;
			*) echo "Unsupported architecture: $ARCH"; exit 1 ;;
		esac
		EXT="tar.gz"
		;;
	*)
		echo "Unsupported OS: $OS"
		echo "For Windows, download from: https://github.com/$REPO/releases"
		exit 1
		;;
esac

# Get latest release tag
echo "Fetching latest release..."
LATEST=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name"' | sed -E 's/.*"([^"]+)".*/\1/')

if [ -z "$LATEST" ]; then
	echo "Failed to fetch latest release"
	exit 1
fi

echo "Installing $BINARY $LATEST for $OS/$ARCH..."

# Download
URL="https://github.com/$REPO/releases/download/$LATEST/$TARGET.$EXT"
TMPDIR=$(mktemp -d)
cd "$TMPDIR"

echo "Downloading $URL..."
curl -fsSL "$URL" -o "$TARGET.$EXT"

# Extract
tar -xzf "$TARGET.$EXT"

# Install
mkdir -p "$INSTALL_DIR"
mv "$BINARY" "$INSTALL_DIR/"
chmod +x "$INSTALL_DIR/$BINARY"

# Cleanup
cd /
rm -rf "$TMPDIR"

echo ""
echo "Installed $BINARY to $INSTALL_DIR/$BINARY"
echo ""

# Check if in PATH
if ! echo "$PATH" | grep -q "$INSTALL_DIR"; then
	echo "Add $INSTALL_DIR to your PATH:"
	echo ""
	echo "  export PATH=\"\$PATH:$INSTALL_DIR\""
	echo ""
	echo "Add this line to your ~/.bashrc or ~/.zshrc"
fi
