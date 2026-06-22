# Post-merge: feat/conversion-surface

Run after PR merges to `main` and before tagging release.

## Release ops

- [ ] Add `HOMEBREW_TAP_TOKEN` to kindling repo secrets (`contents:write` on `eddacraft/homebrew-tap`)
- [ ] Confirm `NPM_TOKEN` and release workflow permissions are current
- [ ] Tag `vX.Y.Z` matching workspace version in `Cargo.toml` / `package.json`
- [ ] Confirm `release.yml` attaches 7 target archives + `.sha256` sidecars (including Linux gnu)
- [ ] Merge automated `homebrew-tap` PR; verify `brew install eddacraft/tap/kindling` on macOS and Linux
- [ ] Confirm `publish.yml` publishes `@eddacraft/kindling` and `@eddacraft/kindling-adapter-vscode`
- [ ] Record asciinema: `asciinema rec -c "./scripts/record-demo.sh" docs/assets/kindling-demo.cast`; embed in `README.md`
- [ ] Mirror `docs/quickstart/`, `docs/integrations.md`, `docs/adapters/cookbook.md` on docs.eddacraft.ai

## Install smoke tests

```bash
curl -fsSL https://raw.githubusercontent.com/eddacraft/kindling/main/install.sh | sh
kindling demo && kindling search JWT && kindling browse --no-open
brew install eddacraft/tap/kindling   # macOS or Linux Homebrew
npm install @eddacraft/kindling
```
