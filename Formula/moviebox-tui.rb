class MovieboxTui < Formula
  VERSION = "0.1.31"
  MACOS_SHA256 = "2805865ad52d2c291d53f344a90a97d6f3bf6edb5e5138835551cfefdd7a888f"
  LINUX_X64_SHA256 = "9de15c7a282fd418227a400cbfb075e21d7d837beb64ae6dea022044f0902ccc"
  LINUX_ARM64_SHA256 = "6cd601920b987ef8daeaea740c0940314b9bd449621f88823ccf768b47d1338b"

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
