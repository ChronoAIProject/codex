import fs from "node:fs";
import path from "node:path";
import { createRequire } from "node:module";

import { describe, expect, it } from "@jest/globals";

const importMetaRequire = createRequire(import.meta.url);

function jsFilesUnder(dir: string): string[] {
  const entries = fs.readdirSync(dir, { withFileTypes: true });
  return entries.flatMap((entry) => {
    const entryPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      return jsFilesUnder(entryPath);
    }
    return /\.(?:cjs|mjs|js)$/.test(entry.name) ? [entryPath] : [];
  });
}

describe("zod dependency", () => {
  it("does not touch navigator during module initialization", () => {
    const packageRoot = path.dirname(importMetaRequire.resolve("zod/package.json"));
    const unsafeNavigatorProbe =
      /typeof\s+navigator\s*!==\s*["']undefined["'][\s\S]{0,160}navigator\?\.[\s\S]{0,80}userAgent/s;

    const offenders = jsFilesUnder(packageRoot).filter((file) =>
      unsafeNavigatorProbe.test(fs.readFileSync(file, "utf8")),
    );

    expect(offenders.map((file) => path.relative(packageRoot, file))).toEqual([]);
  });
});
