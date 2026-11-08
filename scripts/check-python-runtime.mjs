#!/usr/bin/env node

/**
 * Fail-closed check for the generated Python runtime archives.
 *
 * This is the pre-build gate for a source checkout: the 38 MB runtime archive
 * and the 76 MB scientific wheelhouse are generated artifacts and are never
 * committed. It never downloads, extracts or builds anything - a missing or
 * stale archive is an actionable error.
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
const WHEELS_MANIFEST_NAME = "python-runtime-wheels-manifest.json";

export const BUILD_HINT = "Run ./scripts/build-python-runtime.sh before building the desktop app.";
export const WHEELS_BUILD_HINT = "Run ./scripts/build-python-wheelhouse.sh before building the desktop app.";

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
	return await verifyArchive(resourcesDir, MANIFEST_NAME, BUILD_HINT);
}

/**
 * Verify the scientific wheelhouse the same way. Returns `null` when the
 * checkout ships no wheelhouse manifest (a build without the suite).
 */
export async function verifyPreparedWheelhouse(resourcesDir) {
	try {
		await stat(path.join(path.resolve(resourcesDir), WHEELS_MANIFEST_NAME));
	} catch {
		return null;
	}
	return await verifyArchive(resourcesDir, WHEELS_MANIFEST_NAME, WHEELS_BUILD_HINT);
}

async function verifyArchive(resourcesDir, manifestName, hint) {
	const dir = path.resolve(resourcesDir);
	const manifestPath = path.join(dir, manifestName);

	let manifest;
	try {
		manifest = JSON.parse(await readFile(manifestPath, "utf8"));
	} catch (error) {
		throw new Error(`missing or unreadable ${manifestPath}: ${error.message}\n${hint}`);
	}

	for (const key of ["archiveName", "archiveSha256", "archiveSize"]) {
		if (manifest[key] === undefined || manifest[key] === null || manifest[key] === "") {
			throw new Error(`${manifestPath} has no ${key}\n${hint}`);
		}
	}
	if (typeof manifest.archiveName !== "string" || typeof manifest.archiveSha256 !== "string") {
		throw new Error(`${manifestPath} has invalid archive fields\n${hint}`);
	}
	if (!Number.isSafeInteger(manifest.archiveSize) || manifest.archiveSize <= 0) {
		throw new Error(`${manifestPath} has an invalid archiveSize\n${hint}`);
	}

	const archivePath = path.join(dir, manifest.archiveName);
	let info;
	try {
		info = await stat(archivePath);
	} catch {
		throw new Error(`missing ${archivePath}\n${hint}`);
	}
	if (!info.isFile()) {
		throw new Error(`${archivePath} is not a regular file\n${hint}`);
	}
	if (info.size !== manifest.archiveSize) {
		throw new Error(
			`${archivePath} is ${info.size} bytes but the manifest requires ${manifest.archiveSize}\n${hint}`,
		);
	}

	const digest = await sha256(archivePath);
	if (digest !== manifest.archiveSha256) {
		throw new Error(
			`${archivePath} has SHA-256 ${digest} but the manifest requires ${manifest.archiveSha256}\n${hint}`,
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
	const prepared = [await verifyPreparedRuntime(resourcesDir), await verifyPreparedWheelhouse(resourcesDir)];
	for (const archive of prepared.filter(Boolean)) {
		console.log(
			`[python-runtime] ${archive.manifest.archiveName} ready (${archive.archiveSize} bytes, sha256 ${archive.archiveSha256})`,
		);
	}
}

if (process.argv[1] && pathToFileURL(path.resolve(process.argv[1])).href === import.meta.url) {
	main().catch(error => {
		console.error(`[python-runtime] ${error.message}`);
		process.exit(1);
	});
}
