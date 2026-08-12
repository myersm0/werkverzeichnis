#!/bin/sh
# Install script for wv (werkverzeichnis CLI)
# Usage: curl -fsSL https://raw.githubusercontent.com/myersm0/werkverzeichnis/main/install.sh | sh

set -eu

REPO="myersm0/werkverzeichnis"
BINARY="wv"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"

OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

case "$OS" in
	linux)
		case "$ARCH" in
			x86_64) TARGET="wv-linux-x86_64" ;;
			*) echo "Unsupported architecture: $ARCH"; exit 1 ;;
		esac
		DEFAULT_DATA_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/werkverzeichnis"
		;;
	darwin)
		case "$ARCH" in
			x86_64) TARGET="wv-macos-x86_64" ;;
			arm64)  TARGET="wv-macos-arm64" ;;
			*) echo "Unsupported architecture: $ARCH"; exit 1 ;;
		esac
		DEFAULT_DATA_DIR="$HOME/Library/Application Support/werkverzeichnis"
		;;
	*)
		echo "Unsupported OS: $OS"
		echo "For Windows, download from: https://github.com/$REPO/releases"
		exit 1
		;;
esac

DATA_DIR="${WV_INSTALL_DATA_DIR:-$DEFAULT_DATA_DIR}"
EXT="tar.gz"

echo "Fetching latest release..."
LATEST=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name"' | sed -E 's/.*"([^"]+)".*/\1/')

if [ -z "$LATEST" ]; then
	echo "Failed to fetch latest release"
	exit 1
fi

echo "Installing $BINARY $LATEST for $OS/$ARCH..."

URL="https://github.com/$REPO/releases/download/$LATEST/$TARGET.$EXT"
TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT INT TERM
cd "$TMPDIR"

echo "Downloading $URL..."
curl -fsSL "$URL" -o "$TARGET.$EXT"
tar -xzf "$TARGET.$EXT"

if [ ! -x "$BINARY" ] || [ ! -d data/compositions ]; then
	echo "Release archive is missing the binary or dataset"
	exit 1
fi

mkdir -p "$INSTALL_DIR" "$DATA_DIR"
mv "$BINARY" "$INSTALL_DIR/$BINARY"
chmod +x "$INSTALL_DIR/$BINARY"

for dir in catalogs collections composers compositions schemas; do
	rm -rf "$DATA_DIR/$dir"
	mv "data/$dir" "$DATA_DIR/$dir"
done

if [ -f data/LICENSE.md ]; then
	mv data/LICENSE.md "$DATA_DIR/LICENSE.md"
fi

rm -rf "$DATA_DIR/.indexes"
"$INSTALL_DIR/$BINARY" index --data-dir "$DATA_DIR"

echo ""
echo "Installed $BINARY to $INSTALL_DIR/$BINARY"
echo "Installed data to $DATA_DIR"
echo ""

case ":$PATH:" in
	*:"$INSTALL_DIR":*) ;;
	*)
		echo "Add $INSTALL_DIR to your PATH:"
		echo ""
		echo "  export PATH=\"\$PATH:$INSTALL_DIR\""
		echo ""
		echo "Add this line to your ~/.bashrc or ~/.zshrc"
		;;
esac
