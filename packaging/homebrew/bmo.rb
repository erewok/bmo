class Bmo < Formula
  desc "Local-first SQLite-backed CLI issue tracker for AI agents and developers"
  homepage "https://github.com/erewok/bmo"
  version "0.1.0"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/erewok/bmo/releases/download/v#{version}/bmo-aarch64-apple-darwin.tar.gz"
      sha256 "REPLACE_WITH_SHA256_OF_aarch64-apple-darwin_TARBALL"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/erewok/bmo/releases/download/v#{version}/bmo-x86_64-unknown-linux-musl.tar.gz"
      sha256 "REPLACE_WITH_SHA256_OF_x86_64-unknown-linux-musl_TARBALL"
    end
  end

  def install
    bin.install "bmo"
  end

  test do
    system bin/"bmo", "version"
  end
end
