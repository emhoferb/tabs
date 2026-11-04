#!/bin/sh
# SPDX-License-Identifier: GPL-2.0-or-later
#
# Installs Tabs from the latest GitHub release, or updates it when it is
# already installed:
#
#   curl -fsSL https://raw.githubusercontent.com/emhoferb/tabs/main/install.sh | sh
#
# and removes it again (your settings in ~/.var/app stay):
#
#   curl -fsSL https://raw.githubusercontent.com/emhoferb/tabs/main/install.sh | sh -s -- --uninstall
#
# Tabs runs as a Flatpak. If Flatpak itself is missing, it is installed with
# your package manager, which asks for your password.

set -eu

APP=io.github.emhoferb.Tabs
BUNDLE=tabs-x86_64.flatpak
DOWNLOAD=https://github.com/emhoferb/tabs/releases/latest/download
FLATHUB=https://dl.flathub.org/repo/flathub.flatpakrepo

data_home=${XDG_DATA_HOME:-$HOME/.local/share}
exports=${FLATPAK_USER_DIR:-$data_home/flatpak}/exports/share
bin_dir=$HOME/.local/bin
# The checksum of the installed release, to tell whether there is a newer one.
stamp=${XDG_STATE_HOME:-$HOME/.local/state}/tabs/installed.sha256

say() { printf '\033[1m==> %s\033[0m\n' "$*"; }
die() { printf 'error: %s\n' "$*" >&2; exit 1; }

# Everything below reads from /dev/null, not from stdin: under `curl | sh`,
# stdin is the rest of this script.

install_flatpak() {
    command -v flatpak >/dev/null && return
    say "Installing Flatpak"
    if command -v apt-get >/dev/null; then
        sudo apt-get update </dev/null
        sudo apt-get install -y flatpak </dev/null
    elif command -v dnf >/dev/null; then
        sudo dnf install -y flatpak </dev/null
    elif command -v pacman >/dev/null; then
        sudo pacman -S --needed --noconfirm flatpak </dev/null
    elif command -v zypper >/dev/null; then
        sudo zypper --non-interactive install flatpak </dev/null
    else
        die "install Flatpak with your package manager, then run this again"
    fi
}

download() {
    if command -v curl >/dev/null; then
        curl -fL --progress-bar -o "$2" "$1"
    elif command -v wget >/dev/null; then
        wget -q --show-progress -O "$2" "$1"
    else
        die "neither curl nor wget is installed"
    fi
}

# A desktop session only looks into Flatpak's folders once Flatpak was
# installed before you logged in. Until then, links in your own folders let
# the app drawer find Tabs and its icon right away.
link_into_session() {
    case ":${XDG_DATA_DIRS:-}:" in
        *":$exports:"* | *":$exports/:"*) return ;;
    esac
    for src in "$exports/applications/$APP.desktop" \
               "$exports"/icons/hicolor/*/apps/"$APP".png \
               "$exports"/icons/hicolor/*/apps/"$APP"*.svg; do
        [ -e "$src" ] || continue
        dest=$data_home/${src#"$exports"/}
        mkdir -p "${dest%/*}"
        ln -sf "$src" "$dest"
    done
    if command -v update-desktop-database >/dev/null; then
        update-desktop-database -q "$data_home/applications" || true
    fi
}

unlink_from_session() {
    for link in "$data_home/applications/$APP.desktop" \
                "$data_home"/icons/hicolor/*/apps/"$APP"*; do
        [ -L "$link" ] || continue
        case $(readlink "$link") in
            "$exports"/*) rm -f "$link" ;;
        esac
    done
}

install_command() {
    mkdir -p "$bin_dir"
    cat >"$bin_dir/tabspdf" <<EOF
#!/bin/sh
exec flatpak run $APP "\$@"
EOF
    chmod +x "$bin_dir/tabspdf"
}

do_install() {
    [ "$(uname -m)" = x86_64 ] || die "the release is built for x86_64 only; see the README to build it yourself"
    install_flatpak

    say "Adding Flathub, for the GNOME runtime Tabs runs on"
    flatpak remote-add --user --if-not-exists flathub "$FLATHUB" </dev/null

    tmp=$(mktemp -d)
    trap 'rm -rf "$tmp"' EXIT
    download "$DOWNLOAD/$BUNDLE.sha256" "$tmp/$BUNDLE.sha256" 2>/dev/null
    expected=$(cut -d' ' -f1 "$tmp/$BUNDLE.sha256")
    installed=no
    flatpak info --user "$APP" >/dev/null 2>&1 && installed=yes

    if [ $installed = yes ] && [ "$(cat "$stamp" 2>/dev/null)" = "$expected" ]; then
        say "Tabs is already up to date"
    else
        say "Downloading Tabs"
        download "$DOWNLOAD/$BUNDLE" "$tmp/$BUNDLE"
        actual=$(sha256sum "$tmp/$BUNDLE" | cut -d' ' -f1)
        [ "$expected" = "$actual" ] || die "the download is damaged (checksum mismatch); try again"

        # Flatpak refuses to install a bundle over an installed copy, even
        # with --reinstall, so the old one goes first. Settings stay.
        if [ $installed = yes ]; then
            say "Removing the installed version"
            flatpak uninstall --user --noninteractive "$APP" </dev/null
        fi
        say "Installing Tabs (a first install also fetches the GNOME runtime, about 1 GB)"
        flatpak install --user --noninteractive "$tmp/$BUNDLE" </dev/null
        mkdir -p "${stamp%/*}"
        echo "$expected" >"$stamp"
    fi

    link_into_session
    install_command

    say "Done. Tabs is in your app drawer."
    case ":$PATH:" in
        *":$bin_dir:"*) echo "From a terminal: tabspdf file.pdf" ;;
        *) echo "From a terminal: tabspdf file.pdf (in terminals opened after your next login)" ;;
    esac
}

do_uninstall() {
    command -v flatpak >/dev/null && flatpak uninstall --user --noninteractive "$APP" </dev/null || true
    unlink_from_session
    if command -v update-desktop-database >/dev/null && [ -d "$data_home/applications" ]; then
        update-desktop-database -q "$data_home/applications" || true
    fi
    rm -f "$bin_dir/tabspdf" "$stamp"
    rmdir "${stamp%/*}" 2>/dev/null || true
    say "Tabs is removed. Your settings stay in ~/.var/app/$APP; delete that folder to remove them too."
}

case ${1:-} in
    "") do_install ;;
    --uninstall) do_uninstall ;;
    *) die "unknown option '$1' (the only option is --uninstall)" ;;
esac
