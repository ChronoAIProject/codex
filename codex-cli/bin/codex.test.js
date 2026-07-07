import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  copyFileSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  realpathSync,
  symlinkSync,
  writeFileSync,
} from "fs";
import os from "os";
import path from "path";
import test from "node:test";

const PLATFORM_TARGETS = {
  "linux:x64": "x86_64-unknown-linux-musl",
  "linux:arm64": "aarch64-unknown-linux-musl",
  "darwin:x64": "x86_64-apple-darwin",
  "darwin:arm64": "aarch64-apple-darwin",
  "win32:x64": "x86_64-pc-windows-msvc",
  "win32:arm64": "aarch64-pc-windows-msvc",
};

const targetTriple = PLATFORM_TARGETS[`${process.platform}:${process.arch}`];

test(
  "marks Vite+ package roots as managed by Vite+",
  { skip: !targetTriple },
  () => {
    const tempDir = mkdtempSync(path.join(os.tmpdir(), "codex-vp-"));
    const vpHome = path.join(tempDir, "vp-home");
    const packageRoot = path.join(vpHome, "packages", "@openai", "codex");
    const binDir = path.join(packageRoot, "bin");
    const nativeBinDir = path.join(packageRoot, "vendor", targetTriple, "bin");
    const nativeBin = path.join(
      nativeBinDir,
      process.platform === "win32" ? "codex.exe" : "codex",
    );
    mkdirSync(binDir, { recursive: true });
    mkdirSync(nativeBinDir, { recursive: true });
    writeFileSync(path.join(packageRoot, "package.json"), "{}");
    copyFileSync(
      new URL("codex.js", import.meta.url),
      path.join(binDir, "codex.js"),
    );
    linkNodeExecutable(nativeBin);

    const scriptPath = path.join(tempDir, "capture-env.mjs");
    const outputPath = path.join(tempDir, "env.json");
    writeFileSync(
      scriptPath,
      `import { writeFileSync } from "fs";
writeFileSync(process.argv[2], JSON.stringify({
  npm: process.env.CODEX_MANAGED_BY_NPM ?? null,
  bun: process.env.CODEX_MANAGED_BY_BUN ?? null,
  vp: process.env.CODEX_MANAGED_BY_VP,
  root: process.env.CODEX_MANAGED_PACKAGE_ROOT,
}));
`,
    );

    const result = spawnSync(
      process.execPath,
      [path.join(binDir, "codex.js"), scriptPath, outputPath],
      {
        env: {
          ...process.env,
          CODEX_MANAGED_BY_NPM: "1",
          VP_HOME: vpHome,
          npm_config_user_agent: "npm/10 node/v26",
        },
        encoding: "utf8",
      },
    );

    assert.equal(result.status, 0, result.stderr);
    assert.deepEqual(JSON.parse(readFileSync(outputPath, "utf8")), {
      npm: null,
      bun: null,
      vp: "1",
      root: realpathSync(packageRoot),
    });
  },
);

function linkNodeExecutable(targetPath) {
  try {
    symlinkSync(process.execPath, targetPath);
  } catch {
    copyFileSync(process.execPath, targetPath);
  }
}
