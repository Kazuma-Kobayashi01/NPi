#!/usr/bin/env node
// Vendor the patched v10 UMD build without adding a runtime CDN dependency.
import { createHash } from "node:crypto";
import { copyFileSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { execFileSync } from "node:child_process";

const version = "10.9.8";
const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const temporary = mkdtempSync(join(tmpdir(), "npi-mermaid-vendor-"));
async function get(url) {
  const response = await fetch(url);
  if (!response.ok) throw new Error(`${url}: HTTP ${response.status}`);
  return response;
}
try {
  const metadata = await (await get(`https://registry.npmjs.org/mermaid/${version}`)).json();
  const archive = Buffer.from(await (await get(metadata.dist.tarball)).arrayBuffer());
  const integrity = `sha512-${createHash("sha512").update(archive).digest("base64")}`;
  if (integrity !== metadata.dist.integrity) throw new Error("Mermaid package integrity mismatch");
  const tarball = join(temporary, "mermaid.tgz");
  writeFileSync(tarball, archive);
  execFileSync("tar", ["-xzf", tarball, "-C", temporary]);
  copyFileSync(join(temporary, "package/dist/mermaid.min.js"), join(root, "src/mermaid.min.js"));
  const license = await (await get(`https://raw.githubusercontent.com/mermaid-js/mermaid/v${version}/LICENSE`)).text();
  writeFileSync(join(root, "src/mermaid.LICENSE"), license);
  console.log(`Vendored Mermaid ${version}; verified ${integrity}`);
} finally {
  rmSync(temporary, { recursive: true, force: true });
}
