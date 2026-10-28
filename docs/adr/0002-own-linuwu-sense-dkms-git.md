# Arch-Sense hard-depends on a maintained `linuwu-sense-dkms-git`

`paru -S arch-sense` must yield a working setup without a manual `git clone && make install`. The existing `linuwu-sense-dkms` AUR package is pinned to a stale release that lacks newer model support, so we maintain `linuwu-sense-dkms-git` tracking upstream main and make Arch-Sense depend on it. Cost: we own that package's upkeep. Runtime failures (module not loaded, acer_wmi conflict, clang-built kernels) are explained by the Doctor rather than by README prose.
