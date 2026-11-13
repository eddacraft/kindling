# Post-merge: feat/conversion-surface

Reconciled 2026-11-13. v0.4.0 and v0.4.1 are published; the
[active conversion module](../../modules/08-conversion-surface.aps.md) owns
C0/C2/C12 acceptance. This checklist no longer authorises a new release.

## Release ops

- [x] Historical 2026-10-22 receipt: add `HOMEBREW_TAP_TOKEN` to kindling repo secrets (`contents:write` on `eddacraft/homebrew-tap`)
- [ ] Confirm unused `NPM_TOKEN` removal after D-010; release permissions remain operational maintenance
- [x] Published tags v0.4.0 and v0.4.1 match their release prep
- [x] Confirm published v0.4.0/v0.4.1 assets include 7 target archives + `.sha256` sidecars (including Linux gnu)
- [ ] Merge automated `homebrew-tap` PR; verify `brew install eddacraft/tap/kindling` on macOS and Linux
- [x] D-010 supersedes npm publishing; VSIX is attached and Rust crates are published
- [ ] Record asciinema: `asciinema rec -c "./scripts/record-demo.sh" docs/assets/kindling-demo.cast`; embed in `README.md`
- [ ] Mirror `docs/quickstart/`, `docs/integrations.md`, `docs/adapters/cookbook.md` on docs.eddacraft.ai

homebrew-tap#1 is merged. The combined tap/install checkbox remains open because
the native fresh-install matrix is still owed. Both release workflow runs have
an overall failure conclusion; attached assets do not prove all release ops passed.

## Install smoke tests

```bash
curl -fsSL https://raw.githubusercontent.com/eddacraft/kindling/main/install.sh | sh
kindling demo && kindling search JWT && kindling browse --no-open
brew install eddacraft/tap/kindling   # macOS or Linux Homebrew
cargo install eddacraft-kindling
```
