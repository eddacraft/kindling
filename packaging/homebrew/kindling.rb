# Homebrew formula for kindling.
#
# Publish to the live tap: github.com/eddacraft/homebrew-tap → Formula/kindling.rb
#
#   brew install eddacraft/tap/kindling
#
# Generate version + checksums: ./scripts/generate-homebrew-formula.sh vX.Y.Z
#
# Per-release maintenance: bump `version` and replace the four
# REPLACE_WITH_SHA256_* placeholders with the SHA256 of each macOS release
# tarball. The values come straight from the `.sha256` sidecars attached to the
# GitHub Release by .github/workflows/release.yml — they are the first field of
# `kindling-<version>-<target>.tar.gz.sha256`. A release workflow could also
# template this file automatically (see packaging/README.md).
class Kindling < Formula
  desc "Local memory and continuity engine for AI-assisted development"
  homepage "https://github.com/eddacraft/kindling"
  license "Apache-2.0"

  # Bump per release (the bare version, no leading "v").
  version "0.1.2"

  # macOS-only tap. Linux users should use the install.sh installer or
  # download the GitHub Release tarball directly.
  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/eddacraft/kindling/releases/download/v#{version}/kindling-#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "REPLACE_WITH_SHA256_AARCH64_APPLE_DARWIN"
    else
      url "https://github.com/eddacraft/kindling/releases/download/v#{version}/kindling-#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "REPLACE_WITH_SHA256_X86_64_APPLE_DARWIN"
    end
  end

  def install
    bin.install "kindling"
  end

  test do
    assert_match "kindling #{version}", shell_output("#{bin}/kindling --version")
  end
end
