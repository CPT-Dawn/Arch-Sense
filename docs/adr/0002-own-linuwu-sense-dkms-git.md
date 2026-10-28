# Arch-Sense hard-depends on a maintained `linuwu-sense-dkms-git`

`paru -S arch-sense` must yield a working setup without a manual `git clone && make install`. The existing `linuwu-sense-dkms` AUR package is pinned to a stale release that lacks newer model support, so we maintain `linuwu-sense-dkms-git` tracking upstream main and make Arch-Sense depend on it. Cost: we own that package's upkeep. Runtime failures (module not loaded, acer_wmi conflict, clang-built kernels) are explained by the Doctor rather than by README prose.

## Addendum: carried PH16-71 patch (temporary)

Tracking main bit us once: upstream ee01b84 (Linuwu-Sense#103) dropped the Predator PH16-71 DMI entry, so the module loads with `quirk_unknown` and no `predator_sense/` group. The package carries the maintainer's fix from [Linuwu-Sense#152](https://github.com/0x7375646F/Linuwu-Sense/pull/152) as `0001-restore-predator-ph16-71-dmi-entry.patch`. `prepare()` applies it only when the source lacks the entry and fails the build if it no longer applies. Remove the patch and its `prepare()` step once #152 is merged; `tests/check.sh` notices that moment (it skips the "applies to upstream" check). The Doctor names this case and the `options linuwu_sense predator_v4=1` workaround for modules built some other way.
