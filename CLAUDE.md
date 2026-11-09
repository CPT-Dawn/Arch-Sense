## Agent skills

### Issue tracker

Issues are tracked in GitHub Issues on CPT-Dawn/Arch-Sense (via `gh`). See `docs/agents/issue-tracker.md`.

### Triage labels

Default vocabulary: needs-triage, needs-info, ready-for-agent, ready-for-human, wontfix. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: one `GLOSSARY.md` and `docs/adr/` at the repo root. See `docs/agents/domain.md`.

## Commands

- Build: `cargo build --release`
- Test: `cargo test --all-targets` (CI runs `cargo test --locked`)
- Lint: `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check`
- Packaging checks (both AUR packages): `packaging/check.sh` (add `--build` for makepkg)
- Run safely: `cargo run --release -- --demo` (simulated laptop) and `target/release/arch-sense doctor` (read-only). Never run the plain TUI or `--apply` in automation: they write to real hardware.
- Release: bump `version` in Cargo.toml, `cargo update -p arch-sense`, and `pkgver` in `packaging/aur/arch-sense/PKGBUILD`; commit `chore(release): …`; `git tag -s vX.Y.Z -m "Arch-Sense vX.Y.Z"`; push main and the tag; publish with `gh release create vX.Y.Z --notes-file …`. Publishing triggers `.github/workflows/release.yml`, which pushes the AUR package and commits the real tarball checksum back to main (so `git pull` afterwards).
