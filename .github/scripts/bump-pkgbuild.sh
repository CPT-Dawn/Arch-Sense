#!/usr/bin/env bash
# Point the arch-sense PKGBUILD at a release.
#
#   bump-pkgbuild.sh <version|tag> <tarball-sha256> <PKGBUILD>
#
# Sets pkgver, resets pkgrel to 1 and replaces the FIRST sha256sums entry, which
# by convention is the release tarball (the packaged files that follow are local
# and checked by packaging/aur/arch-sense/check.sh). Does nothing on bad input.
set -euo pipefail

(($# == 3)) || { echo "usage: ${0##*/} <version|tag> <tarball-sha256> <PKGBUILD>" >&2; exit 2; }
version=${1#v}
sha=$2
file=$3

[[ $version =~ ^[0-9]+(\.[0-9]+)*$ ]] || { echo "not a version: $1" >&2; exit 1; }
[[ $sha =~ ^[0-9a-f]{64}$ ]] || { echo "not a sha256: $sha" >&2; exit 1; }
[[ -f $file ]] || { echo "no such file: $file" >&2; exit 1; }
grep -q "^sha256sums=('[0-9a-f]\{64\}'" "$file" || { echo "no sha256sums=('<tarball>'... line in $file" >&2; exit 1; }

tmp=$(mktemp)
trap 'rm -f "$tmp"' EXIT
sed -e "s/^pkgver=.*/pkgver=$version/" \
    -e "s/^pkgrel=.*/pkgrel=1/" \
    -e "s/^sha256sums=('[0-9a-f]\{64\}'/sha256sums=('$sha'/" "$file" >"$tmp"
cat "$tmp" >"$file"
