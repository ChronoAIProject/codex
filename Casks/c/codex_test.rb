require "minitest/autorun"

class CodexCaskTest < Minitest::Test
  def setup
    @source = File.read(File.expand_path("codex.rb", __dir__))
  end

  def test_downloads_package_archive_with_code_mode_host
    assert_includes @source, "codex-package-\#{arch}-\#{os}.tar.gz"
    refute_includes @source, "/codex-\#{arch}-\#{os}.tar.gz"
    assert_includes @source, 'binary "bin/codex"'
    assert_includes @source, 'binary "bin/codex-code-mode-host"'
  end
end
