#!/usr/bin/env node

/**
 * Fail-closed check for the generated Python runtime archive.
 *
 * This is the pre-build gate for a source checkout: the 38 MB archive is a
 * generated artifact and is never committed. It never downloads, extracts or
 * builds anything - a missing or stale archive is an actionable error.
 *
 * Usage: node scripts/check-python-runtime.mjs
 */

import { createHash } from "node:crypto";
import { createReadStream } from "node:fs";
import { readFile, stat } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const REPO_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const MANIFEST_NAME = "python-runtime-manifest.json";

export const BUILD_HINT = "Run ./scripts/build-python-runtime.sh before building the desktop app.";

function sha256(file) {
	return new Promise((resolve, reject) => {
		const hash = createHash("sha256");
		const stream = createReadStream(file);
		stream.on("error", reject);
		stream.on("data", chunk => hash.update(chunk));
		stream.on("end", () => resolve(hash.digest("hex")));
	});
}

/**
 * Verify that `resourcesDir` contains a manifest whose archive is present and
 * byte-identical to it. Throws with the build hint on every failure.
 */
export async function verifyPreparedRuntime(resourcesDir) {
	const dir = path.resolve(resourcesDir);
	const manifestPath = path.join(dir, MANIFEST_NAME);

	let manifest;
	try {
		manifest = JSON.parse(await readFile(manifestPath, "utf8"));
	} catch (error) {
		throw new Error(`missing or unreadable ${manifestPath}: ${error.message}\n${BUILD_HINT}`);
	}

	for (const key of ["archiveName", "archiveSha256", "archiveSize"]) {
		if (manifest[key] === undefined || manifest[key] === null || manifest[key] === "") {
			throw new Error(`${manifestPath} has no ${key}\n${BUILD_HINT}`);
		}
	}
	if (typeof manifest.archiveName !== "string" || typeof manifest.archiveSha256 !== "string") {
		throw new Error(`${manifestPath} has invalid archive fields\n${BUILD_HINT}`);
	}
	if (!Number.isSafeInteger(manifest.archiveSize) || manifest.archiveSize <= 0) {
		throw new Error(`${manifestPath} has an invalid archiveSize\n${BUILD_HINT}`);
	}

	const archivePath = path.join(dir, manifest.archiveName);
	let info;
	try {
		info = await stat(archivePath);
	} catch {
		throw new Error(`missing ${archivePath}\n${BUILD_HINT}`);
	}
	if (!info.isFile()) {
		throw new Error(`${archivePath} is not a regular file\n${BUILD_HINT}`);
	}
	if (info.size !== manifest.archiveSize) {
		throw new Error(
			`${archivePath} is ${info.size} bytes but the manifest requires ${manifest.archiveSize}\n${BUILD_HINT}`,
		);
	}

	const digest = await sha256(archivePath);
	if (digest !== manifest.archiveSha256) {
		throw new Error(
			`${archivePath} has SHA-256 ${digest} but the manifest requires ${manifest.archiveSha256}\n${BUILD_HINT}`,
		);
	}

	return {
		resourcesDir: dir,
		manifestPath,
		archivePath,
		manifest,
		archiveSize: info.size,
		archiveSha256: digest,
	};
}

async function main() {
	const resourcesDir = path.join(REPO_ROOT, "desktop", "src-tauri", "resources");
	const prepared = await verifyPreparedRuntime(resourcesDir);
	console.log(
		`[python-runtime] ${prepared.manifest.archiveName} ready (${prepared.archiveSize} bytes, sha256 ${prepared.archiveSha256})`,
	);
}

if (process.argv[1] && pathToFileURL(path.resolve(process.argv[1])).href === import.meta.url) {
	main().catch(error => {
		console.error(`[python-runtime] ${error.message}`);
		process.exit(1);
	});
}
