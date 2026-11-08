class MovieboxTui < Formula
  VERSION = "0.1.32"
  MACOS_SHA256 = "0ce82f9c0c9a0efae09e11c4af1fd4f403f7899798afa93dc139e40c10a02a13"
  LINUX_X64_SHA256 = "8bf034b09a7ff49996a7e00d4212eedc8a7016ad16eb6bd05adbcfa91e3d7429"
  LINUX_ARM64_SHA256 = "8cb04f105c3dae8bc1201ea72bb3c23871ef3631b169556dd33536a975d94bc7"

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
