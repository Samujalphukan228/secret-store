#!/bin/sh
set -e

REPO="https://github.com/Samujalphukan228/secret-store"
BIN_NAME="secret"
BIN_DIR="$HOME/.local/bin"

# ── helpers ────────────────────────────────────────────────────────────────────

info()    { printf "\033[1;36m==>\033[0m %s\n" "$1"; }
success() { printf "\033[1;32m  ✓\033[0m %s\n" "$1"; }
warn()    { printf "\033[1;33m  !\033[0m %s\n" "$1"; }
die()     { printf "\033[1;31merror:\033[0m %s\n" "$1" >&2; exit 1; }

# ── detect shell rc ────────────────────────────────────────────────────────────

detect_shell_rc() {
    case "$SHELL" in
        */zsh)  echo "$HOME/.zshrc" ;;
        */bash) echo "$HOME/.bashrc" ;;
        *)      echo "$HOME/.bashrc" ;;
    esac
}

# ── step 1: install rust if missing ───────────────────────────────────────────

install_rust_if_needed() {
    if command -v cargo >/dev/null 2>&1; then
        success "Rust already installed ($(cargo --version))"
        return
    fi

    info "Rust not found — installing via rustup"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path
    success "Rust installed"

    . "$HOME/.cargo/env"
}

# ── step 2: check for xclip (linux) ───────────────────────────────────────────

check_xclip() {
    if [ "$(uname)" = "Linux" ]; then
        if ! command -v xclip >/dev/null 2>&1; then
            warn "xclip not found (needed for clipboard)"
            warn "Install with: sudo pacman -S xclip (Arch) or sudo apt install xclip (Debian)"
        fi
    fi
}

# ── step 3: build ──────────────────────────────────────────────────────────────

build() {
    command -v cargo >/dev/null 2>&1 || . "$HOME/.cargo/env"
    command -v git   >/dev/null 2>&1 || die "git not found — please install git first"

    info "Cloning secret-store"
    TMP_DIR=$(mktemp -d)
    git clone --depth 1 "$REPO" "$TMP_DIR/secret-store" || die "git clone failed"
    success "Cloned"

    info "Building (this may take a moment on first run)"
    cd "$TMP_DIR/secret-store"
    cargo build --release --quiet
    success "Build complete"

    mkdir -p "$BIN_DIR"
    cp "target/release/$BIN_NAME" "$BIN_DIR/$BIN_NAME"
    chmod +x "$BIN_DIR/$BIN_NAME"
    success "Binary installed → $BIN_DIR/$BIN_NAME"

    cd /
    rm -rf "$TMP_DIR"
}

# ── step 4: ensure ~/.local/bin is in PATH ────────────────────────────────────

ensure_path() {
    RC=$(detect_shell_rc)

    case ":$PATH:" in
        *":$BIN_DIR:"*) ;;
        *)
            if ! grep -q "$BIN_DIR" "$RC" 2>/dev/null; then
                printf '\nexport PATH="%s:$PATH"\n' "$BIN_DIR" >> "$RC"
                success "$BIN_DIR added to PATH"
            fi
            ;;
    esac
}

# ── main ───────────────────────────────────────────────────────────────────────

printf "\n\033[1msecret-store installer\033[0m\n\n"

install_rust_if_needed
check_xclip
build
ensure_path

export PATH="$BIN_DIR:$PATH"

printf "\n\033[1;32mAll done!\033[0m\n\n"
printf "Initialize:\n\n"
printf "  \033[1msecret init\033[0m\n\n"
printf "And try:\n\n"
printf "  \033[1mss test 'hello'\033[0m\n"
printf "  \033[1msg t\033[0m\n\n"