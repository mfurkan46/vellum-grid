#!/bin/sh
# Pardus / Debian paketi oluşturur: sh packaging/linux/build-deb.sh
set -eu
cd "$(dirname "$0")/../.."

cargo build --release -p vellum-grid

version=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
arch=$(dpkg --print-architecture)
root="target/deb/vellum-grid_${version}_${arch}"
rm -rf "$root"

install -Dm755 target/release/vellum-grid "$root/usr/bin/vellum-grid"
install -Dm644 packaging/linux/vellum-grid.desktop "$root/usr/share/applications/vellum-grid.desktop"
for size in 32 64 256 512; do
    install -Dm644 "assets/vellum-grid-$size.png" "$root/usr/share/icons/hicolor/${size}x${size}/apps/vellum-grid.png"
done

mkdir -p "$root/DEBIAN"
cat > "$root/DEBIAN/control" <<EOF
Package: vellum-grid
Version: $version
Section: office
Priority: optional
Architecture: $arch
Depends: libc6, libgtk-3-0, libxkbcommon0, libgl1, xdg-user-dirs, fonts-crosextra-carlito, fonts-liberation2 | fonts-liberation
Maintainer: Vellum Grid geliştiricileri
Description: Hızlı, açık kaynak hesap tablosu
 Vellum Grid, Rust ile yazılmış, çevrimdışı çalışan bir hesap tablosudur.
 xlsx, ods ve csv dosyalarını açar ve kaydeder, PDF olarak dışa aktarır.
EOF
cat > "$root/DEBIAN/postinst" <<'EOF'
#!/bin/sh
set -e
command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database -q /usr/share/applications || true
command -v gtk-update-icon-cache >/dev/null 2>&1 && gtk-update-icon-cache -q -t /usr/share/icons/hicolor || true
EOF
chmod 755 "$root/DEBIAN/postinst"

dpkg-deb --build --root-owner-group "$root"
echo "Paket hazır: $root.deb"
