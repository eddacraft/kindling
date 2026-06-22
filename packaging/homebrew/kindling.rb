# Homebrew formula for kindling.
#
# Publishes alongside anvil in github.com/eddacraft/homebrew-tap (Formula/kindling.rb).
#
#   brew install eddacraft/tap/kindling
#
# Generate version + checksums: ./scripts/generate-homebrew-formula.sh vX.Y.Z --sync-tap
class Kindling < Formula
  desc "Local memory and continuity engine for AI-assisted development"
  homepage "https://github.com/eddacraft/kindling"
  license "Apache-2.0"

  # Bump per release (bare version, no leading "v").
  version "0.1.2"

  if OS.mac?
    if Hardware::CPU.arm?
      url "https://github.com/eddacraft/kindling/releases/download/v#{version}/kindling-#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "REPLACE_WITH_SHA256_AARCH64_APPLE_DARWIN"
    end
    if Hardware::CPU.intel?
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