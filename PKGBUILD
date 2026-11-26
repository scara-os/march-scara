# Maintainer: Scara <scara@archlinux>
pkgname=march-scara-git
_pkgname=march-scara
pkgver=0.1.0.r12.g653e742
pkgrel=1
pkgdesc="macOS Tahoe-styled Desktop Environment with Hyprland freedom and GNOME comfort"
arch=('x86_64')
url="https://github.com/scara-os/march-scara"
license=('GPL-3.0-or-later')

depends=(
  'gtk4'
  'gtk4-layer-shell'
  'wayland'
  'libxkbcommon'
  'libinput'
  'seatd'
  'mesa'
  'pipewire'
  'pipewire-audio'
  'pipewire-pulse'
  'wireplumber'
  'upower'
  'power-profiles-daemon'
  'lua54'
  'kitty'
  'thunar'
  'gvfs'
  'firefox'
  'mpv'
  'loupe'
  'gnome-calendar'
  'gnome-calculator'
  'gnome-control-center'
  'gnome-text-editor'
  'evince'
  'file-roller'
  'mako'
  'swappy'
  'grim'
  'slurp'
  'wl-clipboard'
  'pamixer'
  'pavucontrol'
  'networkmanager'
  'nm-connection-editor'
  'bluez'
  'bluez-utils'
  'blueman'
  'polkit'
  'polkit-gnome'
  'xdg-desktop-portal'
  'xdg-desktop-portal-gtk'
  'xdg-desktop-portal-wlr'
)

makedepends=(
  'git'
  'cargo'
  'rust'
  'clang'
  'pkgconf'
)

optdepends=(
  'mactahoe-icon-theme-git: native macOS Tahoe icon theme'
  'mactahoe-gtk-theme: macOS Tahoe dark theme'
  'thunar-archive-plugin: archive integration in Thunar'
  'gvfs-mtp: MTP device support in Thunar'
  'seahorse: GNOME credentials and keyring manager'
  'gnome-keyring: Secret Service credential storage'
  'cups: CUPS print service and lpstat client'
  'system-config-printer: printer and queue settings utility'
  'gnome-software: graphical software update center'
)

provides=('march-scara')
conflicts=('march-scara')
source=("git+https://github.com/scara-os/march-scara.git")
sha256sums=('SKIP')

pkgver() {
  cd "$srcdir/$_pkgname"
  if git describe --tags >/dev/null 2>&1; then
    git describe --long --tags | sed 's/^v//;s/\([^-]*-g\)/r\1/;s/-/./g'
  else
    printf "0.1.0.r%s.g%s" "$(git rev-list --count HEAD)" "$(git rev-parse --short=7 HEAD)"
  fi
}

build() {
  cd "$srcdir/$_pkgname"
  export RUSTUP_TOOLCHAIN=stable
  export CARGO_TARGET_DIR=target
  cargo build --release --workspace
}

package() {
  cd "$srcdir/$_pkgname"

  # 1. Pasang Binary Inti ke /usr/bin/
  install -Dm755 "target/release/scara-wm" "$pkgdir/usr/bin/scara-wm"
  install -Dm755 "target/release/scara-shell" "$pkgdir/usr/bin/scara-shell"
  install -Dm755 "target/release/scara-daemon" "$pkgdir/usr/bin/scara-daemon"
  install -Dm755 "target/release/scara-ctl" "$pkgdir/usr/bin/scara-ctl"
  install -Dm755 "scripts/scara-session" "$pkgdir/usr/bin/scara-session"

  # 2. Pasang Desktop Entry untuk Login Manager (GDM/SDDM)
  install -Dm644 "data/march-scara.desktop" "$pkgdir/usr/share/wayland-sessions/march-scara.desktop"
  install -Dm644 "data/march-scara-settings-handler.desktop" "$pkgdir/usr/share/applications/march-scara-settings-handler.desktop"

  # 3. Pasang Konfigurasi XDG Desktop Portal
  install -Dm644 "data/march-scara-portals.conf" "$pkgdir/usr/share/xdg-desktop-portal/march-scara-portals.conf"

  # 4. Pasang Default Template Config (untuk sistem dan user baru)
  install -Dm644 "data/init.lua" "$pkgdir/usr/share/march-scara/init.lua"
  install -Dm644 "data/init.lua" "$pkgdir/etc/skel/.config/march-scara/init.lua"

  # 5. Dokumentasi aktivasi layanan sistem secara opsional
  install -Dm644 "POST-INSTALL.md" "$pkgdir/usr/share/doc/march-scara/POST-INSTALL.md"
}
