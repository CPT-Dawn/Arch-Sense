#!/usr/bin/env bash
set -euo pipefail

if [[ -z "${GITHUB_REF_NAME:-}" ]]; then
  echo "GITHUB_REF_NAME is required (example: v1.2.0)." >&2
  exit 1
fi

release_tag="${GITHUB_REF_NAME}"
if [[ "$release_tag" != v* ]]; then
  echo "Release tag must start with 'v' (received: $release_tag)." >&2
  exit 1
fi

pkgname="arch-sense"
pkgver="${release_tag#v}"
pkgrel="${PKGREL:-1}"
maintainer="${AUR_MAINTAINER:-CPT_Dawn <dawnsp0456@gmail.com>}"
repo="${GITHUB_REPOSITORY:-CPT-Dawn/Arch-Sense}"
server_url="${GITHUB_SERVER_URL:-https://github.com}"

tarball_url="${server_url}/${repo}/archive/refs/tags/${release_tag}.tar.gz"
source_name="${pkgname}-${pkgver}.tar.gz"

tmp_tarball="$(mktemp)"
trap 'rm -f "$tmp_tarball"' EXIT

curl -fsSL "$tarball_url" -o "$tmp_tarball"
sha256="$(sha256sum "$tmp_tarball" | awk '{print $1}')"

cat > PKGBUILD <<EOF
# Maintainer: ${maintainer}
pkgname=${pkgname}
pkgver=${pkgver}
pkgrel=${pkgrel}
pkgdesc="Acer Predator PH16-71 control center for Arch Linux — thermal profiles, fan control, battery management, and keyboard RGB via a Rust TUI"
arch=('x86_64')
url="${server_url}/${repo}"
license=('MIT')
depends=('libusb' 'gcc-libs' 'glibc')
makedepends=('cargo' 'git')
optdepends=(
  'nvidia-utils: GPU temperature monitoring via nvidia-smi'
  'linuwu-sense-dkms: kernel module for sysfs hardware controls'
)

install=arch-sense.install

source=("${source_name}::${tarball_url}")
sha256sums=('${sha256}')

prepare() {
  cd "Arch-Sense-\${pkgver}"
  cargo fetch --locked --target "\$(rustc -vV | sed -n 's/host: //p')"
}

build() {
  cd "Arch-Sense-\${pkgver}"
  export CARGO_TARGET_DIR=target
  cargo build --frozen --release --all-features
}

check() {
  cd "Arch-Sense-\${pkgver}"
  cargo test --frozen --all-features
}

package() {
  cd "Arch-Sense-\${pkgver}"

  install -Dm755 "target/release/arch-sense" "\${pkgdir}/usr/bin/arch-sense"
  install -Dm644 "arch-sense.service" "\${pkgdir}/usr/lib/systemd/system/arch-sense.service"
  install -Dm644 "LICENSE" "\${pkgdir}/usr/share/licenses/\${pkgname}/LICENSE"
  install -dm755 "\${pkgdir}/var/lib/arch-sense"

  echo "d /var/lib/arch-sense 0755 root root -" | \\
    install -Dm644 /dev/stdin "\${pkgdir}/usr/lib/tmpfiles.d/arch-sense.conf"
}
EOF

cat > .SRCINFO <<EOF
pkgbase = ${pkgname}
	pkgdesc = Acer Predator PH16-71 control center for Arch Linux — thermal profiles, fan control, battery management, and keyboard RGB via a Rust TUI
	pkgver = ${pkgver}
	pkgrel = ${pkgrel}
	url = ${server_url}/${repo}
	install = arch-sense.install
	arch = x86_64
	license = MIT
	makedepends = cargo
	makedepends = git
	depends = libusb
	depends = gcc-libs
	depends = glibc
	optdepends = nvidia-utils: GPU temperature monitoring via nvidia-smi
	optdepends = linuwu-sense-dkms: kernel module for sysfs hardware controls
	source = ${source_name}::${tarball_url}
	sha256sums = ${sha256}

pkgname = ${pkgname}
EOF