#!/usr/bin/env bash
# Scripted checks for the linuwu-sense-dkms-git package (ticket #17, ADR-0002).
# Seams: `makepkg --printsrcinfo`, the built package archive, and `dkms` driven
# against private trees (no root, nothing installed, no module loaded).
# Usage: tests/check.sh [srcinfo|package|dkms|clang]...   (default: all)
set -uo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
fails=0

pass() { printf 'ok   %s\n' "$1"; }
fail() { printf 'FAIL %s\n' "$1"; fails=$((fails + 1)); }
expect() { # expect <description> <command...>
  local d=$1
  shift
  if "$@" >/dev/null 2>&1; then pass "$d"; else fail "$d"; fi
}

root=
stage() { # copy the packaging dir to scratch so makepkg litter stays out of the repo
  [[ -d $work/pkg ]] && return 0
  mkdir -p "$work/pkg"
  cp -r "$here"/. "$work/pkg/"
  rm -rf "$work/pkg/tests"
}

check_srcinfo() {
  stage
  local info
  info=$(cd "$work/pkg" && makepkg --printsrcinfo 2>&1) || { fail "makepkg --printsrcinfo runs"; return; }
  pass "makepkg --printsrcinfo runs"
  expect "pkgname is linuwu-sense-dkms-git" grep -q $'^pkgname = linuwu-sense-dkms-git$' <<<"$info"
  expect "provides linuwu-sense-dkms" grep -q $'^\tprovides = linuwu-sense-dkms$' <<<"$info"
  expect "conflicts with linuwu-sense-dkms" grep -q $'^\tconflicts = linuwu-sense-dkms$' <<<"$info"
  expect "depends on dkms" grep -q $'^\tdepends = dkms' <<<"$info"
  expect "arch is any" grep -q $'^\tarch = any$' <<<"$info"
  expect "sources upstream Linuwu-Sense via git" grep -q 'source = .*git+https://github.com/0x7375646F/Linuwu-Sense' <<<"$info"
  expect "maintainer is CPT_Dawn" grep -q '^# Maintainer: CPT_Dawn <dev@cptdawn.dev>' "$here/PKGBUILD"
}

build_package() { # build once, extract to $work/root; sets $pkgfile, $ver, $root
  [[ -n ${root:-} ]] && return 0
  stage
  (cd "$work/pkg" && SRCDEST="$work/srcdest" PKGDEST="$work/out" BUILDDIR="$work/build" \
    makepkg -f --nodeps --noconfirm --skippgpcheck >"$work/makepkg.log" 2>&1) || { cat "$work/makepkg.log"; fail "makepkg builds the package"; return 1; }
  pass "makepkg builds the package"
  pkgfile=$(ls "$work"/out/linuwu-sense-dkms-git-*.pkg.tar* 2>/dev/null | head -1)
  [[ -n $pkgfile ]] || { fail "makepkg produced a package file"; return 1; }
  root=$work/root
  mkdir -p "$root"
  bsdtar -xf "$pkgfile" -C "$root"
  ver=$(sed -n 's/^pkgver = //p' "$root/.PKGINFO")
  ver=${ver%-*} # drop -pkgrel
}

check_package() {
  build_package || return
  expect "pkgver looks like r<count>.<sha>" grep -Eq '^r[0-9]+\.[0-9a-f]+$' <<<"$ver"
  local src=$root/usr/src/linuwu-sense-$ver
  expect "DKMS source tree has dkms.conf" test -f "$src/dkms.conf"
  expect "DKMS source tree has Kbuild" test -f "$src/Kbuild"
  expect "DKMS source tree has the module source" test -f "$src/src/linuwu_sense.c"
  expect "no signing script / sudo Makefile shipped" test ! -e "$src/dkms-sign.sh" -a ! -e "$src/Makefile"
  expect "dkms.conf PACKAGE_NAME is linuwu-sense" grep -qx 'PACKAGE_NAME="linuwu-sense"' "$src/dkms.conf"
  expect "dkms.conf PACKAGE_VERSION matches pkgver" grep -qx "PACKAGE_VERSION=\"$ver\"" "$src/dkms.conf"
  expect "dkms.conf has no unexpanded placeholder" bash -c '[[ -s $1 ]] && ! grep -q @ "$1"' _ "$src/dkms.conf"
  expect "dkms.conf AUTOINSTALL=yes" grep -qx 'AUTOINSTALL="yes"' "$src/dkms.conf"
  expect "dkms.conf has no POST_BUILD signing" bash -c '[[ -s $1 ]] && ! grep -q POST_BUILD "$1"' _ "$src/dkms.conf"
  expect "acer_wmi is blacklisted" grep -qx 'blacklist acer_wmi' "$root/usr/lib/modprobe.d/linuwu-sense.conf"
  expect "linuwu_sense autoloads at boot" grep -qx 'linuwu_sense' "$root/usr/lib/modules-load.d/linuwu-sense.conf"
  expect "licence installed" test -s "$root/usr/share/licenses/linuwu-sense-dkms-git/LICENSE"
  if command -v namcap >/dev/null; then
    expect "namcap reports no errors" test -z "$(namcap "$pkgfile" 2>&1 | grep ' E: ')"
  else
    echo "skip namcap (not installed)"
  fi
}

dkms_setup() { # private dkms trees seeded from the built package; sets $dk (command prefix)
  build_package || return 1
  [[ -d $work/dkms ]] && return 0
  mkdir -p "$work/dkms/tree" "$work/dkms/src" "$work/dkms/inst"
  cp -r "$root/usr/src/linuwu-sense-$ver" "$work/dkms/src/"
  dk=(dkms --dkmstree "$work/dkms/tree" --sourcetree "$work/dkms/src" --installtree "$work/dkms/inst")
  "${dk[@]}" add -m linuwu-sense -v "$ver" >"$work/dkms-add.log" 2>&1 || { cat "$work/dkms-add.log"; fail "dkms add"; return 1; }
}

check_dkms() {
  dkms_setup || return
  local kver
  kver=$(uname -r)
  if [[ ! -d /lib/modules/$kver/build ]]; then
    echo "skip dkms build (no kernel headers for $kver)"
    return
  fi
  if "${dk[@]}" build -m linuwu-sense -v "$ver" -k "$kver" --kernelsourcedir "/lib/modules/$kver/build" >"$work/dkms-build.log" 2>&1; then
    pass "dkms builds the module for the running kernel ($kver)"
  else
    cat "$work/dkms-build.log"
    fail "dkms builds the module for the running kernel ($kver)"
    return
  fi
  expect "dkms produced linuwu_sense.ko" bash -c 'ls "$1"/linuwu_sense.ko*' _ "$work/dkms/tree/linuwu-sense/$ver/$kver/$(uname -m)/module"
}

# No clang kernel is available here, so use a stub kernel tree whose Makefile
# reports the LLVM variable it was invoked with. Proves the dkms.conf wiring
# only; a real clang-built kernel build is a manual check (see README).
fake_kernel_llvm() { # fake_kernel_llvm <name> <config line> -> prints the LLVM value seen by make
  local k=$work/fake-$1
  mkdir -p "$k/include"
  echo "$2" >"$k/.config"
  printf 'modules:\n\t@echo "LLVM-SEEN=[$(LLVM)]"\nclean:\n\t@true\n' >"$k/Makefile"
  "${dk[@]}" build -m linuwu-sense -v "$ver" -k "0.0.0-$1" --kernelsourcedir "$k" >"$work/dkms-$1.log" 2>&1
  grep -o 'LLVM-SEEN=\[[^]]*\]' "$work/dkms/tree/linuwu-sense/$ver/build/make.log" 2>/dev/null | head -1
}

check_clang() {
  dkms_setup || return
  local seen
  seen=$(fake_kernel_llvm clang 'CONFIG_CC_IS_CLANG=y')
  [[ $seen == 'LLVM-SEEN=[1]' ]] && pass "clang-built kernel builds with LLVM=1" || { cat "$work/dkms-clang.log"; fail "clang-built kernel builds with LLVM=1 (saw '$seen')"; }
  # dkms >= 3.x also adds LLVM=1 itself, so check the shipped dkms.conf in isolation too.
  local conf=$work/dkms/src/linuwu-sense-$ver/dkms.conf
  expect "dkms.conf alone adds LLVM=1 for a clang kernel" \
    bash -c 'kernel_source_dir=$1; dkms_tree=/x; . "$2"; [[ ${MAKE[0]} == *" LLVM=1" ]]' _ "$work/fake-clang" "$conf"
  seen=$(fake_kernel_llvm gcc 'CONFIG_CC_IS_GCC=y')
  expect "dkms.conf alone leaves a gcc kernel alone" \
    bash -c 'kernel_source_dir=$1; dkms_tree=/x; . "$2"; [[ ${MAKE[0]} != *LLVM* ]]' _ "$work/fake-gcc" "$conf"
  [[ $seen == 'LLVM-SEEN=[]' ]] && pass "gcc-built kernel does not get LLVM=1" || { cat "$work/dkms-gcc.log"; fail "gcc-built kernel does not get LLVM=1 (saw '$seen')"; }
}

(($# > 0)) || set -- srcinfo package dkms clang
for t in "$@"; do
  case $t in
    srcinfo) check_srcinfo ;;
    package) check_package ;;
    dkms) check_dkms ;;
    clang) check_clang ;;
    *) echo "unknown check: $t" >&2; fails=$((fails + 1)) ;;
  esac
done
((fails == 0)) && echo "all checks passed" || { echo "$fails check(s) failed"; exit 1; }
