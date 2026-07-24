#!/bin/bash
#
# musictag installer
# Detects distro and installs dependencies automatically
#

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

INSTALL_DIR="/usr/local/bin"
BUILD_DIR="/tmp/musictag-build"

info()  { echo -e "${CYAN}[*]${NC} $1"; }
ok()    { echo -e "${GREEN}[+]${NC} $1"; }
warn()  { echo -e "${YELLOW}[!]${NC} $1"; }
err()   { echo -e "${RED}[-]${NC} $1"; }

# ── Detect distro ──────────────────────────────────────────────

detect_distro() {
    if [ -f /etc/os-release ]; then
        . /etc/os-release
        ID_LOWER="${ID,,}"
        ID_LIKE_LOWER="${ID_LIKE,,}"

        case "$ID_LOWER" in
            arch|manjaro|endeavouros|artix|garuda|hyperos|blendos|cachyos)
                DISTRO="arch"
                ;;
            fedora|nobara|bluefin|universal-blue)
                DISTRO="fedora"
                ;;
            ubuntu|linuxmint|pop|elementary|zorin|kde-neon|regolith|pop_os)
                DISTRO="debian"
                ;;
            debian|ubuntu|kali|deepin|raspbian|devuan|pureos)
                DISTRO="debian"
                ;;
            void)
                DISTRO="void"
                ;;
            alpine)
                DISTRO="alpine"
                ;;
            gentoo|funtoo)
                DISTRO="gentoo"
                ;;
            opensuse*|suse*)
                DISTRO="opensuse"
                ;;
            centos|rhel|rocky|alma)
                DISTRO="rhel"
                ;;
            nixos)
                DISTRO="nixos"
                ;;
            *)
                if echo "$ID_LIKE_LOWER" | grep -q "arch"; then
                    DISTRO="arch"
                elif echo "$ID_LIKE_LOWER" | grep -q "debian\|ubuntu"; then
                    DISTRO="debian"
                elif echo "$ID_LIKE_LOWER" | grep -q "fedora"; then
                    DISTRO="fedora"
                elif echo "$ID_LIKE_LOWER" | grep -q "rhel\|centos"; then
                    DISTRO="rhel"
                else
                    DISTRO="unknown"
                fi
                ;;
        esac
    elif [ -f /etc/arch-release ]; then
        DISTRO="arch"
    elif [ -f /etc/debian_version ]; then
        DISTRO="debian"
    elif [ -f /etc/fedora-release ]; then
        DISTRO="fedora"
    elif [ -f /etc/gentoo-release ]; then
        DISTRO="gentoo"
    elif [ -f /etc/alpine-release ]; then
        DISTRO="alpine"
    elif [ -d /run/current-system ]; then
        DISTRO="nixos"
    else
        DISTRO="unknown"
    fi
}

# ── Check if command exists ────────────────────────────────────

has() {
    command -v "$1" &>/dev/null
}

# ── Install dependencies ──────────────────────────────────────

install_deps() {
    info "Detected distro: ${CYAN}$DISTRO${NC}"
    echo ""

    local to_install=()
    local already_ok=0
    local total=0

    # Check rust
    total=$((total + 1))
    if has rustc && has cargo; then
        ok "rustc    $(rustc --version 2>/dev/null | head -1)"
        ok "cargo    $(cargo --version 2>/dev/null | head -1)"
        already_ok=$((already_ok + 1))
    else
        warn "rustc    not found"
        warn "cargo    not found"
        to_install+=("rust")
    fi

    # Check pkg-config (needed by lofty)
    total=$((total + 1))
    if has pkg-config; then
        ok "pkg-config found"
        already_ok=$((already_ok + 1))
    else
        warn "pkg-config not found"
        case "$DISTRO" in
            arch)     to_install+=("pkgconf") ;;
            debian)   to_install+=("pkg-config") ;;
            fedora)   to_install+=("pkgconf-pkg-config") ;;
            alpine)   to_install+=("pkgconf") ;;
            void)     to_install+=("pkg-config") ;;
            gentoo)   to_install+=("virtual/pkgconf") ;;
            opensuse) to_install+=("pkg-config") ;;
            *)        to_install+=("pkg-config") ;;
        esac
    fi

    # Check git
    total=$((total + 1))
    if has git; then
        ok "git      $(git --version 2>/dev/null | awk '{print $3}')"
        already_ok=$((already_ok + 1))
    else
        warn "git      not found"
        to_install+=("git")
    fi

    # Check for nerd font
    total=$((total + 1))
    if fc-list 2>/dev/null | grep -qi "nerd"; then
        ok "Nerd Font detected"
        already_ok=$((already_ok + 1))
    else
        warn "No Nerd Font detected (icons may not render)"
    fi

    echo ""
    info "Dependencies status: ${already_ok}/${total} already installed"

    if [ ${#to_install[@]} -eq 0 ]; then
        ok "All dependencies satisfied!"
        return 0
    fi

    echo ""
    warn "Missing packages: ${to_install[*]}"
    echo ""

    case "$DISTRO" in
        arch)
            info "Installing with pacman..."
            sudo pacman -S --needed --noconfirm "${to_install[@]}"
            ;;
        debian)
            info "Installing with apt..."
            sudo apt update -qq
            sudo apt install -y -qq "${to_install[@]}"
            ;;
        fedora)
            info "Installing with dnf..."
            sudo dnf install -y "${to_install[@]}"
            ;;
        rhel)
            info "Installing with dnf/yum..."
            if has dnf; then
                sudo dnf install -y "${to_install[@]}"
            else
                sudo yum install -y "${to_install[@]}"
            fi
            ;;
        alpine)
            info "Installing with apk..."
            sudo apk add "${to_install[@]}"
            ;;
        void)
            info "Installing with xbps..."
            sudo xbps-install -Sy "${to_install[@]}"
            ;;
        gentoo)
            info "Installing with emerge..."
            sudo emerge -av --quiet "${to_install[@]}"
            ;;
        opensuse)
            info "Installing with zypper..."
            sudo zypper install -y "${to_install[@]}"
            ;;
        nixos)
            warn "NixOS detected. Add to configuration.nix:"
            warn "  environment.systemPackages = with pkgs; [ rustc cargo pkg-config git ];"
            warn "Then run: sudo nixos-rebuild switch"
            ;;
        *)
            err "Unknown distro. Please install manually:"
            err "  - rust (https://rustup.rs)"
            err "  - cargo (comes with rust)"
            err "  - pkg-config"
            err "  - git"
            return 1
            ;;
    esac

    ok "Dependencies installed!"
}

# ── Install Rust if missing ───────────────────────────────────

install_rust() {
    if ! has rustc || ! has cargo; then
        info "Installing Rust via rustup..."
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        source "$HOME/.cargo/env"
        ok "Rust installed: $(rustc --version)"
    fi
}

# ── Build and install ─────────────────────────────────────────

build_and_install() {
    info "Building musictag..."

    rm -rf "$BUILD_DIR"
    mkdir -p "$BUILD_DIR"

    cp -r ./* "$BUILD_DIR/"
    cd "$BUILD_DIR"

    cargo build --release 2>&1 | tail -3

    if [ ! -f target/release/musictag ]; then
        err "Build failed!"
        exit 1
    fi

    ok "Build successful!"

    info "Installing to $INSTALL_DIR..."
    sudo install -Dm755 target/release/musictag "$INSTALL_DIR/musictag"

    ok "musictag installed to $INSTALL_DIR/musictag"

    # Cleanup
    cd /
    rm -rf "$BUILD_DIR"
}

# ── Main ──────────────────────────────────────────────────────

main() {
    echo ""
    echo -e "${CYAN}  ╔═══════════════════════════════════╗${NC}"
    echo -e "${CYAN}  ║       musictag installer          ║${NC}"
    echo -e "${CYAN}  ╚═══════════════════════════════════╝${NC}"
    echo ""

    detect_distro
    install_deps
    install_rust
    build_and_install

    echo ""
    ok "Done! Run 'musictag' to start."
    echo ""
}

main "$@"
