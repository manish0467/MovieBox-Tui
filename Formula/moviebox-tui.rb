class MovieboxTui < Formula
  VERSION = "0.1.28"
  MACOS_SHA256 = "d222cb45258d4cec1375bfa26fce5f3236b7a1ce0abda132be773fb6df2cae9c"
  LINUX_X64_SHA256 = "0c2a34a9cb8f0f903c886a60cf772152fc7807a658f1c787d22949f331f833fc"
  LINUX_ARM64_SHA256 = "06f65bd1428314cdf4746baa00a1bc03b5a2ee41ada1da8f376ca78142261d4e"

  desc "Stream movies, shows, anime, and live TV from your terminal"
  homepage "https://github.com/mesamirh/MovieBox-Tui"
  version VERSION
  license any_of: ["MIT", "Apache-2.0"]

  on_macos do
    url "https://github.com/manish0467/MovieBox-Tui/releases/download/v#{VERSION}/MovieBox_macOS_Universal.tar.gz"
    sha256 MACOS_SHA256
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/manish0467/MovieBox-Tui/releases/download/v#{VERSION}/MovieBox_Linux_arm64.tar.gz"
      sha256 LINUX_ARM64_SHA256
    else
      url "https://github.com/manish0467/MovieBox-Tui/releases/download/v#{VERSION}/MovieBox_Linux_x64.tar.gz"
      sha256 LINUX_X64_SHA256
    end
  end

  def install
    bin.install "moviebox-tui"
  end

  test do
    system "#{bin}/moviebox-tui", "--version"
  end
end
