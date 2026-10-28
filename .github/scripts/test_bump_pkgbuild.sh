#!/usr/bin/env bash
# Tests for bump-pkgbuild.sh. Seam: the script's command line and the PKGBUILD it edits.
set -uo pipefail
here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
bump="$here/bump-pkgbuild.sh"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
fails=0
ok() { printf 'ok   %s\n' "$1"; }
bad() { printf 'FAIL %s\n' "$1"; fails=$((fails + 1)); }
expect() { local d=$1; shift; if "$@" >/dev/null 2>&1; then ok "$d"; else bad "$d"; fi; }

new_pkgbuild() { # the shape of the real PKGBUILD: tarball first, then local files
  cat >"$work/PKGBUILD" <<'PKG'
pkgname=arch-sense
pkgver=1.3.2
pkgrel=3
source=(
  "$pkgname-$pkgver.tar.gz::https://example.invalid/v$pkgver.tar.gz"
  'arch-sense.service'
  '71-arch-sense.rules'
)
sha256sums=('1111111111111111111111111111111111111111111111111111111111111111'
            '2222222222222222222222222222222222222222222222222222222222222222'
            '3333333333333333333333333333333333333333333333333333333333333333')
PKG
}
sha=$(printf 'a%.0s' {1..64})

new_pkgbuild
expect "bumps to a new version" "$bump" 2.0.0 "$sha" "$work/PKGBUILD"
expect "sets pkgver" grep -qx 'pkgver=2.0.0' "$work/PKGBUILD"
expect "resets pkgrel to 1" grep -qx 'pkgrel=1' "$work/PKGBUILD"
expect "replaces the tarball checksum" grep -qF "sha256sums=('$sha'" "$work/PKGBUILD"
expect "leaves the local file checksums alone" grep -qF "'2222222222222222222222222222222222222222222222222222222222222222'" "$work/PKGBUILD"
expect "keeps the last checksum and closing paren" grep -qF "'3333333333333333333333333333333333333333333333333333333333333333')" "$work/PKGBUILD"
expect "result still parses as bash" bash -n "$work/PKGBUILD"

before=$(cat "$work/PKGBUILD")
expect "same version twice is idempotent" "$bump" 2.0.0 "$sha" "$work/PKGBUILD"
expect "idempotent run changes nothing" test "$before" = "$(cat "$work/PKGBUILD")"

new_pkgbuild
expect "accepts a leading v from the tag" "$bump" v2.1.0 "$sha" "$work/PKGBUILD"
expect "strips the v" grep -qx 'pkgver=2.1.0' "$work/PKGBUILD"

new_pkgbuild
before=$(cat "$work/PKGBUILD")
expect "rejects a malformed checksum" test "$("$bump" 2.0.0 nothex "$work/PKGBUILD" >/dev/null 2>&1; echo $?)" != 0
expect "rejects a non-version" test "$("$bump" banana "$sha" "$work/PKGBUILD" >/dev/null 2>&1; echo $?)" != 0
expect "rejected runs leave the file untouched" test "$before" = "$(cat "$work/PKGBUILD")"

# prepare-aur-release.sh end to end, offline, with a tarball cut from this checkout.
repo=$(cd "$here/../.." && pwd)
ver=$(sed -n 's/^version = "\(.*\)"/\1/p' "$repo/Cargo.toml" | head -n1)
git -C "$repo" archive --prefix="Arch-Sense-$ver/" HEAD | gzip -n >"$work/release.tar.gz"
expect "prepare-aur-release builds a payload" "$here/prepare-aur-release.sh" "v$ver" "$work/aur" "$work/release.tar.gz"
want=$(sha256sum "$work/release.tar.gz" | cut -d' ' -f1)
expect "payload PKGBUILD carries the tarball checksum" grep -qF "sha256sums=('$want'" "$work/aur/PKGBUILD"
expect "payload has .SRCINFO for the version" grep -qx $'\tpkgver = '"$ver" "$work/aur/.SRCINFO"
expect "payload has the install script and packaged files" test -f "$work/aur/arch-sense.install" -a -f "$work/aur/71-arch-sense.rules"
expect "payload leaves out dev-only files" test ! -e "$work/aur/check.sh"
expect "refuses a tag that disagrees with Cargo.toml" test "$("$here/prepare-aur-release.sh" v99.0.0 "$work/aur2" "$work/release.tar.gz" >/dev/null 2>&1; echo $?)" != 0

((fails == 0)) && echo "all tests passed" || { echo "$fails test(s) failed"; exit 1; }
