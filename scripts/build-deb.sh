#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_root"

version=$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -n 1)
if [[ -z "$version" ]]; then
  echo "Could not read the package version from Cargo.toml" >&2
  exit 1
fi

cargo build --release --locked

package_root=$(mktemp -d)
trap 'rm -rf "$package_root"' EXIT
chmod 755 "$package_root"

install -Dm755 target/release/bingwall "$package_root/usr/bin/bingwall"
strip --strip-unneeded "$package_root/usr/bin/bingwall"
install -Dm644 packaging/bingwall.desktop \
  "$package_root/usr/share/applications/bingwall.desktop"
install -Dm644 packaging/systemd/bingwall.service \
  "$package_root/usr/lib/systemd/user/bingwall.service"
install -Dm644 packaging/systemd/bingwall.timer \
  "$package_root/usr/lib/systemd/user/bingwall.timer"
install -Dm644 README.md "$package_root/usr/share/doc/bingwall/README.md"

installed_size=$(du -sk "$package_root/usr" | cut -f1)
install -Dm644 packaging/debian/control "$package_root/DEBIAN/control"
sed -i \
  -e "s/@VERSION@/$version/g" \
  -e "s/@INSTALLED_SIZE@/$installed_size/g" \
  "$package_root/DEBIAN/control"

mkdir -p dist
output="dist/bingwall_${version}_amd64.deb"
dpkg-deb --root-owner-group --build "$package_root" "$output"
echo "Built $output"
