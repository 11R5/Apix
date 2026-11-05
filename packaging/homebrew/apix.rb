# The Homebrew cask for Apix. scripts/channel-manifests.sh fills in
# @VERSION@ and @SHA256@ for each release; the filled-in file goes into a tap
# repository (homebrew-apix) as Casks/apix.rb:
#   brew install --cask 11r5/apix/apix
cask "apix" do
  version "@VERSION@"
  sha256 "@SHA256@"

  url "https://github.com/11R5/Apix/releases/download/v#{version}/Apix-#{version}-macos-universal.dmg"
  name "Apix"
  desc "Fast, native API client with a command line"
  homepage "https://11r5.github.io/Apix/"

  livecheck do
    url :url
    strategy :github_latest
  end

  depends_on macos: ">= :big_sur"

  app "Apix.app"
  binary "#{appdir}/Apix.app/Contents/MacOS/apix"

  zap trash: [
    "~/Library/Application Support/Apix",
    "~/Library/Caches/Apix",
  ]

  caveats <<~EOS
    Apix is not notarized by Apple. If macOS refuses to open it, run:
      xattr -dr com.apple.quarantine /Applications/Apix.app
  EOS
end
