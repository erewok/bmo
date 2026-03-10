# Homebrew Formula for bmo

This formula is maintained at https://github.com/erewok/homebrew-tap.

## Setup after a release

1. Create the tap repository at github.com/erewok/homebrew-tap with a Formula/ directory.
2. After cutting a release tag and the release.yaml workflow completes, get the SHA256 of each artifact:
   shasum -a 256 bmo-aarch64-apple-darwin.tar.gz
   shasum -a 256 bmo-x86_64-unknown-linux-musl.tar.gz
3. Update bmo.rb: replace REPLACE_WITH_SHA256_... placeholders with the real values and bump `version`.
4. Copy bmo.rb to the tap repo as Formula/bmo.rb.
5. Users install with: brew install erewok/tap/bmo

## Tap installation

brew tap erewok/tap https://github.com/erewok/homebrew-tap
brew install erewok/tap/bmo
