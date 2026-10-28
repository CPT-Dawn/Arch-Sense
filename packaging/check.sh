#!/usr/bin/env bash
# Runs the packaging checks for both AUR packages (what CI runs).
#
#   packaging/check.sh           static checks for arch-sense, all checks for linuwu-sense-dkms-git
#   packaging/check.sh --build   also build arch-sense with makepkg
#
# Needs makepkg, systemd's tools and (for the DKMS checks) dkms; namcap runs when
# installed. Never needs root.
set -uo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
status=0
echo "=== arch-sense ==="
"$here/aur/arch-sense/check.sh" "$@" || status=1
echo
echo "=== linuwu-sense-dkms-git ==="
"$here/aur/linuwu-sense-dkms-git/tests/check.sh" || status=1
exit "$status"
