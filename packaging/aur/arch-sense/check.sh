#!/usr/bin/env bash
# Checks for the arch-sense package. Needs makepkg and systemd's tools; never
# needs root and never touches the live system.
#
#   ./check.sh           static checks (seconds)
#   ./check.sh --build   also run makepkg (build + cargo test) and inspect the package
set -uo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$here/../../.." && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

failures=0
ok() { printf '  ok    %s\n' "$1"; }
fail() { printf '  FAIL  %s\n' "$1"; failures=$((failures + 1)); }
skip() { printf '  skip  %s\n' "$1"; }
check() { # check <description> <command...>
	local what="$1"; shift
	local out
	if out="$("$@" 2>&1)"; then ok "$what"; else fail "$what"; printf '%s\n' "$out" | sed 's/^/        /'; fi
}

rules="$here/71-arch-sense.rules"
sysusers="$here/arch-sense.sysusers"
tmpfiles="$here/arch-sense.tmpfiles"
service="$here/arch-sense.service"
install_script="$here/arch-sense.install"

# The only sysfs attributes the arch-sense group may write. Anything else in the
# rules file is a review failure, not a style nit. Paths are relative to the
# acer-wmi platform device, except the legacy ACPI Thermal Profile.
allowed_sysfs=(
	predator_sense/backlight_timeout
	predator_sense/battery_calibration
	predator_sense/battery_limiter
	predator_sense/boot_animation_sound
	predator_sense/fan_speed
	predator_sense/lcd_override
	predator_sense/usb_charging
	nitro_sense/backlight_timeout
	nitro_sense/battery_calibration
	nitro_sense/battery_limiter
	nitro_sense/boot_animation_sound
	nitro_sense/fan_speed
	nitro_sense/lcd_override
	nitro_sense/usb_charging
	four_zoned_kb/four_zone_mode
	four_zoned_kb/per_zone_mode
	profile
	/sys/firmware/acpi/platform_profile
)

echo "files"
for f in PKGBUILD "$rules" "$sysusers" "$tmpfiles" "$service" "$install_script"; do
	check "${f##*/} exists" test -f "$here/${f##*/}"
done
((failures)) && { echo "missing files; stopping"; exit 1; }

echo "PKGBUILD"
cd "$here" || exit 1
check "makepkg --printsrcinfo parses the PKGBUILD" makepkg --printsrcinfo
cargo_version="$(sed -n 's/^version = "\(.*\)"/\1/p' "$repo/Cargo.toml" | head -n1)"
pkgver="$(makepkg --printsrcinfo 2>/dev/null | sed -n 's/^\tpkgver = //p')"
check "pkgver ($pkgver) matches Cargo.toml ($cargo_version)" test "$pkgver" = "$cargo_version"
# The release tarball has no checksum we can know before tagging, so only the
# local files (every source after the tarball) are verified here.
local_sources_match() {
	local i=0 src sum
	mapfile -t sources < <(makepkg --printsrcinfo | sed -n 's/^\tsource = //p')
	mapfile -t sums < <(makepkg --printsrcinfo | sed -n 's/^\tsha256sums = //p')
	((${#sources[@]} == ${#sums[@]})) || { echo "source/checksum count mismatch"; return 1; }
	[[ "${sources[0]}" == "$pkgname-"*".tar.gz::"* ]] || { echo "first source must be the release tarball"; return 1; }
	for ((i = 1; i < ${#sources[@]}; i++)); do
		src="${sources[i]}"
		[[ "$(sha256sum "$here/$src" | cut -d' ' -f1)" == "${sums[i]}" ]] || { echo "checksum drift: $src (run makepkg -g)"; return 1; }
	done
}
pkgname=arch-sense
check "local sources match their checksums" local_sources_match
check "release tarball checksum is a sha256" grep -qE "^sha256sums=\('[0-9a-f]{64}'" "$here/PKGBUILD"
srcinfo="$(makepkg --printsrcinfo 2>/dev/null)"
check "depends on linuwu-sense-dkms-git" grep -qx $'\tdepends = linuwu-sense-dkms-git' <<<"$srcinfo"

echo "udev rules"
check "udevadm verify" udevadm verify --resolve-names=never "$rules"

# Every RUN must be exactly `chgrp arch-sense <allowed>` or `chmod g+w <allowed>`.
allowed_target() {
	local target="$1" a
	for a in "${allowed_sysfs[@]}"; do
		[[ "$target" == "%S%p/$a" || "$target" == "$a" ]] && return 0
	done
	return 1
}
run_problems="$(
	grep -o 'RUN+="[^"]*"' "$rules" | sed 's/^RUN+="//; s/"$//' | while read -r prog arg1 target extra; do
		case "$prog $arg1" in
		"/usr/bin/chgrp arch-sense" | "/usr/bin/chmod g+w") ;;
		*) echo "unexpected command: $prog $arg1 $target $extra" ;;
		esac
		[[ -z "$extra" ]] || echo "extra arguments: $extra"
		allowed_target "$target" || echo "not on the allowlist: $target"
	done
)"
check "every RUN is a chgrp/chmod g+w of an allowlisted attribute" test -z "$run_problems"
[[ -z "$run_problems" ]] || printf '        %s\n' "$run_problems"

granted() { grep -o "RUN+=\"/usr/bin/$1 [^\"]*\"" "$rules" | awk '{print $3}' | tr -d '"' | sort -u; }
check "every chgrp'd attribute is also made group-writable (and vice versa)" \
	diff <(granted chgrp) <(granted chmod)
check "every allowlisted attribute is granted" \
	diff <(printf '%s\n' "${allowed_sysfs[@]}" | sort) \
	<(granted chgrp | sed 's|^%S%p/||' | sort)
check "device nodes only ever get GROUP=arch-sense" \
	bash -c '! grep -o "GROUP=\"[^\"]*\"" "$1" | grep -vx "GROUP=\"arch-sense\""' _ "$rules"
check "device nodes only ever get MODE=0664" \
	bash -c '! grep -o "MODE=\"[^\"]*\"" "$1" | grep -vx "MODE=\"0664\""' _ "$rules"
check "the USB HID keyboard (04f2:0117) device node is group-writable" \
	grep -qE 'ATTR\{idVendor\}=="04f2", ATTR\{idProduct\}=="0117",.*GROUP="arch-sense", MODE="0664"' "$rules"
check "no uaccess, OWNER or wildcard grants" \
	bash -c '! grep -nE "uaccess|OWNER|RUN\+=\"[^\"]*[*?]" "$1"' _ "$rules"

# On a laptop with Linuwu-Sense loaded, ask udev what it would actually run.
acer=/sys/devices/platform/acer-wmi
cp "$rules" "$work/"
if [[ -d "$acer/predator_sense" || -d "$acer/nitro_sense" ]]; then
	udev_out="$(udevadm test --action=add -N never -D "$work" "$acer" 2>&1)"
	for d in predator_sense nitro_sense four_zoned_kb; do
		[[ -d "$acer/$d" ]] || continue
		for f in "$acer/$d"/*; do
			allowed_target "$d/${f##*/}" || continue
			check "udev would chgrp $d/${f##*/} on this machine" \
				grep -qF "/usr/bin/chgrp arch-sense $f" <<<"$udev_out"
		done
	done
	for pp in /sys/class/platform-profile/*; do
		[[ "$(cat "$pp/name" 2>/dev/null)" == acer-wmi ]] || continue
		pp_out="$(udevadm test --action=add -N never -D "$work" "$(readlink -f "$pp")" 2>&1)"
		check "udev would chgrp the acer-wmi Thermal Profile on this machine" \
			grep -qF "/usr/bin/chgrp arch-sense $(readlink -f "$pp")/profile" <<<"$pp_out"
	done
else
	skip "udevadm test (no Linuwu-Sense device on this machine)"
fi
for usb in /sys/bus/usb/devices/*; do
	[[ "$(cat "$usb/idVendor" 2>/dev/null):$(cat "$usb/idProduct" 2>/dev/null)" == 04f2:0117 ]] || continue
	kb_out="$(udevadm test --action=add -D "$work" "$(readlink -f "$usb")" 2>&1)"
	check "udev would make the USB HID keyboard node mode 0664 on this machine" \
		grep -qE '71-arch-sense.rules:[0-9]+ MODE="0664"' <<<"$kb_out"
done

echo "systemd"
check "sysusers.d creates the arch-sense group" \
	bash -c 'systemd-sysusers --dry-run --root="$1" "$2" 2>&1 | grep -q "Creating group .arch-sense."' _ "$work/sysroot" "$sysusers"
mkdir -p "$work/tmproot/etc" && echo 'arch-sense:x:999:' >"$work/tmproot/etc/group"
check "tmpfiles.d entry is valid" systemd-tmpfiles --dry-run --create --root="$work/tmproot" "$tmpfiles"
mkdir -p "$work/unitroot/usr/bin" "$work/unitroot/usr/lib/systemd/system"
install -m755 /dev/null "$work/unitroot/usr/bin/arch-sense"
cp "$service" "$work/unitroot/usr/lib/systemd/system/"
check "systemd-analyze verify arch-sense.service" \
	systemd-analyze verify --root="$work/unitroot" "$work/unitroot/usr/lib/systemd/system/arch-sense.service"

echo "install script"
check "bash -n arch-sense.install" bash -n "$install_script"
# Run the scriptlets against stub tools that only log what they were asked to do.
stubs="$work/stubs"
mkdir -p "$stubs"
for tool in systemctl udevadm systemd-sysusers systemd-notify systemd-detect-virt; do
	printf '#!/bin/sh\necho "%s $*" >>"%s/calls"\n' "$tool" "$work" >"$stubs/$tool"
	chmod +x "$stubs/$tool"
done
# A booted, non-chroot system: systemd-notify --booted succeeds, detect-virt --chroot fails.
printf '#!/bin/sh\necho "systemd-detect-virt $*" >>"%s/calls"\nexit 1\n' "$work" >"$stubs/systemd-detect-virt"
post_install_out="$(PATH="$stubs:$PATH" bash -c '. "$1"; post_install 2.0.0-1' _ "$install_script" 2>&1)"
calls="$(cat "$work/calls" 2>/dev/null)"
check "post_install tells the user to join the group" grep -qF 'usermod -aG arch-sense' <<<"$post_install_out"
check "post_install says to log out and back in" grep -qi 'log out' <<<"$post_install_out"
check "post_install mentions opt-in Boot Restore" grep -qF 'systemctl enable arch-sense.service' <<<"$post_install_out"
check "post_install creates the group before udev uses it" grep -qF 'systemd-sysusers arch-sense.conf' <<<"$calls"
check "post_install reloads udev rules" grep -qF 'udevadm control --reload' <<<"$calls"
check "post_install re-triggers the hardware" grep -qF 'udevadm trigger' <<<"$calls"
check "post_install never enables or starts a unit" bash -c '! grep -E "systemctl .*(enable|start)" <<<"$1"' _ "$calls"
: >"$work/calls"
# v1's post_upgrade enabled Boot Restore for everyone; v2 makes it opt-in, so
# upgrading from v1 switches it off (the stub says it's enabled) and says how to keep it.
v1_upgrade_out="$(PATH="$stubs:$PATH" bash -c '. "$1"; post_upgrade 2.0.0-1 1.3.2-1' _ "$install_script" 2>&1)"
check "post_upgrade never enables or starts a unit" bash -c '! grep -E "systemctl .*(enable|start)( |$)" "$1"' _ "$work/calls"
check "post_upgrade from v1 disables the Boot Restore v1 enabled" \
	grep -qE 'systemctl .*disable arch-sense.service' "$work/calls"
check "post_upgrade from v1 says Boot Restore is opt-in, in one line" \
	bash -c '[[ "$(grep -c "Boot Restore" <<<"$1")" == 1 ]] && grep "Boot Restore" <<<"$1" | grep -qF "systemctl enable arch-sense.service"' _ "$v1_upgrade_out"
: >"$work/calls"
v2_upgrade_out="$(PATH="$stubs:$PATH" bash -c '. "$1"; post_upgrade 2.0.1-1 2.0.0-1' _ "$install_script" 2>&1)"
check "post_upgrade within v2 leaves Boot Restore alone" \
	bash -c '! grep -qE "systemctl .*(disable|enable)" "$1"' _ "$work/calls"
check "post_upgrade within v2 says nothing about Boot Restore" \
	bash -c '! grep -q "Boot Restore" <<<"$1"' _ "$v2_upgrade_out"
# A v1 user who never had Boot Restore on: nothing to switch off, nothing to say.
v1_off="$work/stubs-v1-off"
mkdir -p "$v1_off"
printf '#!/bin/sh\necho "systemctl $*" >>"%s/calls"\ncase "$*" in *is-enabled*) exit 1 ;; esac\n' "$work" >"$v1_off/systemctl"
chmod +x "$v1_off/systemctl"
: >"$work/calls"
v1_off_out="$(PATH="$v1_off:$stubs:$PATH" bash -c '. "$1"; post_upgrade 2.0.0-1 1.3.2-1' _ "$install_script" 2>&1)"
check "post_upgrade from v1 without Boot Restore disables nothing" \
	bash -c '! grep -qE "systemctl .*disable" "$1"' _ "$work/calls"
check "post_upgrade from v1 without Boot Restore stays quiet about it" \
	bash -c '! grep -q "Boot Restore" <<<"$1"' _ "$v1_off_out"
: >"$work/calls"
PATH="$stubs:$PATH" bash -c '. "$1"; pre_remove 2.0.0-1' _ "$install_script" >/dev/null 2>&1
check "pre_remove disables Boot Restore so no enable symlink outlives the unit" \
	grep -qE 'systemctl .*disable arch-sense.service' "$work/calls"

if [[ "${1:-}" == "--build" ]]; then
	echo "build"
	# Unreleased versions have no GitHub tarball, so build from one cut off the
	# working tree (committed + tracked changes) and pre-seed it. --skipchecksum is
	# fine: the local files were verified above and the tarball is ours by design.
	# --nodeps: linuwu-sense-dkms-git needn't be installed to build or test the app.
	mkdir -p "$work/srcdest"
	tree="$(git -C "$repo" stash create 2>/dev/null || true)" # empty when the tree is clean
	git -C "$repo" archive --prefix="Arch-Sense-$pkgver/" "${tree:-HEAD}" |
		gzip -n >"$work/srcdest/arch-sense-$pkgver.tar.gz"
	check "makepkg builds and tests the package" \
		env SRCDEST="$work/srcdest" BUILDDIR="$work/build" PKGDEST="$work/pkg" \
		makepkg --nodeps --force --cleanbuild --noconfirm --skipchecksum
	pkg="$(ls "$work"/pkg/arch-sense-[0-9]*.pkg.tar* 2>/dev/null | grep -v '\.sig$' | head -n1)"
	check "makepkg produced a package" test -n "$pkg"
	if [[ -n "$pkg" ]]; then
		expected="$(printf '%s\n' \
			usr/bin/arch-sense \
			usr/lib/systemd/system/arch-sense.service \
			usr/lib/sysusers.d/arch-sense.conf \
			usr/lib/tmpfiles.d/arch-sense.conf \
			usr/lib/udev/rules.d/71-arch-sense.rules \
			usr/share/licenses/arch-sense/LICENSE | sort)"
		actual="$(bsdtar -tf "$pkg" | grep -v '^\.' | grep -v '/$' | sort)"
		check "package ships exactly the expected files" diff <(echo "$expected") <(echo "$actual")
		if command -v namcap >/dev/null; then
			check "namcap reports no errors for the PKGBUILD" test -z "$(namcap "$here/PKGBUILD" 2>&1 | grep ' E: ')"
			check "namcap reports no errors for the package" test -z "$(namcap "$pkg" 2>&1 | grep ' E: ')"
			echo "  namcap warnings (review):"
			{ namcap "$here/PKGBUILD"; namcap "$pkg"; } 2>&1 | grep ' W: ' | sed 's/^/        /'
		else
			skip "namcap (not installed)"
		fi
	fi
fi

echo
if ((failures)); then
	echo "$failures check(s) failed"
	exit 1
fi
echo "all checks passed"
