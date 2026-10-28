#!/usr/bin/env bash
# Build the arch-sense AUR payload for a release. Must not run as root (makepkg).
#
#   prepare-aur-release.sh <tag> <out-dir> [tarball]
#
# Downloads the tag's GitHub source tarball (or uses [tarball], for tests), bumps
# the checked-in PKGBUILD to it, verifies every checksum for real, and writes the
# files the AUR repo needs plus a fresh .SRCINFO into <out-dir>. Publishes nothing.
set -euo pipefail

(($# >= 2)) || { echo "usage: ${0##*/} <tag> <out-dir> [tarball]" >&2; exit 2; }
tag=$1
out=$2
tarball=${3:-}

repo=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
src="$repo/packaging/aur/arch-sense"
version=${tag#v}
[[ $version =~ ^[0-9]+(\.[0-9]+)*$ ]] || { echo "tag does not look like vX.Y.Z: $tag" >&2; exit 1; }

cargo_version=$(sed -n 's/^version = "\(.*\)"/\1/p' "$repo/Cargo.toml" | head -n1)
[[ $cargo_version == "$version" ]] ||
  { echo "Cargo.toml says $cargo_version but the tag says $version; refusing to publish a mismatch" >&2; exit 1; }

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
mkdir -p "$out"
# Everything makepkg needs, but not the dev-only check script.
find "$src" -maxdepth 1 -type f ! -name check.sh ! -name .gitignore -exec cp {} "$out/" \;

if [[ -z $tarball ]]; then
  tarball="$work/release.tar.gz"
  curl --fail --silent --show-error --location --retry 5 --retry-delay 5 \
    -o "$tarball" "https://github.com/CPT-Dawn/Arch-Sense/archive/refs/tags/$tag.tar.gz"
fi
sha=$(sha256sum "$tarball" | cut -d' ' -f1)

"$repo/.github/scripts/bump-pkgbuild.sh" "$version" "$sha" "$out/PKGBUILD"

# Verify all checksums (tarball and packaged files) against the real sources.
mkdir -p "$work/srcdest"
cp "$tarball" "$work/srcdest/arch-sense-$version.tar.gz"
(cd "$out" && SRCDEST="$work/srcdest" BUILDDIR="$work/build" makepkg --verifysource --nodeps --force)
(cd "$out" && makepkg --printsrcinfo >.SRCINFO)
grep -qx $'\tpkgver = '"$version" "$out/.SRCINFO" || { echo ".SRCINFO has the wrong pkgver" >&2; exit 1; }
echo "AUR payload for arch-sense $version is in $out (tarball sha256 $sha)"
