#!/usr/bin/env node

/**
 * Smoke-test a built desktop app's embedded Python runtime through the real
 * `--npi-python-host` entry point, in a throwaway NPI_ENV_DIR.
 *
 * Usage:
 *   node scripts/smoke-python-runtime.mjs <app-exe> <app-bundle> <Resources/resources>
 *   node scripts/smoke-python-runtime.mjs <app-exe> <Resources/resources>
 *
 * The app bundle is optional but is the recommended form because it makes the
 * resource path explicit. Nothing touches the user's real NPI_ENV_DIR, and no
 * packages are installed (no network). Exits non-zero on any mismatch.
 */

import { execFile, spawn } from "node:child_process";
import { access, mkdtemp, realpath, rm, stat } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { promisify } from "node:util";
import { verifyPreparedRuntime } from "./check-python-runtime.mjs";

const execFileP = promisify(execFile);
const HOST_TIMEOUT_MS = 300_000;
const PYTHON_TIMEOUT_MS = 60_000;

function usage() {
	console.error(
		"usage: node scripts/smoke-python-runtime.mjs <app-exe> [app-bundle] <Resources/resources>",
	);
	process.exit(2);
}

function runHost(appExe, request, env) {
	return new Promise((resolve, reject) => {
		const child = spawn(appExe, ["--npi-python-host"], {
			env,
			stdio: ["pipe", "pipe", "pipe"],
		});
		let stdout = "";
		let stderr = "";
		child.stdout.setEncoding("utf8").on("data", chunk => {
			stdout += chunk;
		});
		child.stderr.setEncoding("utf8").on("data", chunk => {
			stderr += chunk;
		});
		const timer = setTimeout(() => {
			child.kill("SIGKILL");
			reject(new Error(`host ${request.action} did not answer within ${HOST_TIMEOUT_MS}ms`));
		}, HOST_TIMEOUT_MS);
		child.on("error", error => {
			clearTimeout(timer);
			reject(error);
		});
		child.on("close", code => {
			clearTimeout(timer);
			if (code !== 0) {
				reject(
					new Error(
						`host ${request.action} failed (exit ${code}): ${stderr.trim() || "(no stderr)"}`,
					),
				);
				return;
			}
			try {
				resolve(JSON.parse(stdout));
			} catch {
				reject(
					new Error(
						`host ${request.action} returned invalid JSON: ${stdout.slice(0, 200) || "(empty)"}`,
					),
				);
			}
		});
		child.stdin.end(`${JSON.stringify(request)}\n`);
	});
}

async function resolvePrepared(resourcesDir) {
	let lastError;
	for (const candidate of [resourcesDir, path.join(resourcesDir, "resources")]) {
		try {
			return await verifyPreparedRuntime(candidate);
		} catch (error) {
			lastError = error;
		}
	}
	throw lastError;
}

async function main() {
	const args = process.argv.slice(2);
	if (args.length < 1 || args.length > 3) usage();
	const appExe = path.resolve(args[0]);
	let appBundle;
	let resourcesArg;
	if (args.length === 3) {
		appBundle = path.resolve(args[1]);
		resourcesArg = args[2];
	} else if (args.length === 2) {
		if (args[1].endsWith(".app")) appBundle = path.resolve(args[1]);
		else resourcesArg = args[1];
	}
	const resourcesDir = path.resolve(
		resourcesArg ?? path.join(appBundle ?? path.dirname(appExe), "Contents", "Resources"),
	);

	const exeInfo = await stat(appExe);
	if (!exeInfo.isFile()) throw new Error(`${appExe} is not a file`);
	if (appBundle) {
		const bundleInfo = await stat(appBundle);
		if (!bundleInfo.isDirectory()) throw new Error(`${appBundle} is not an app bundle directory`);
	}

	const prepared = await resolvePrepared(resourcesDir);
	const tempEnv = await mkdtemp(path.join(os.tmpdir(), "npi-python-smoke-"));
	const env = {
		...process.env,
		NPI_ENV_DIR: tempEnv,
		NPI_PYTHON_RESOURCE_DIR: prepared.resourcesDir,
	};

	try {
		const before = await runHost(appExe, { action: "environment" }, env);
		if (before.ok) throw new Error("a fresh NPI_ENV_DIR unexpectedly reported a healthy environment");
		if (before.pythonPath !== "") {
			throw new Error(`environment reported an interpreter before ensure: ${before.pythonPath}`);
		}

		const health = await runHost(appExe, { action: "ensure" }, env);
		if (!health.ok) throw new Error(`ensure reported unhealthy: ${health.message ?? "no message"}`);
		for (const key of ["pythonPath", "envPath", "runtimePath", "pythonVersion", "uvVersion"]) {
			if (typeof health[key] !== "string" || health[key] === "") {
				throw new Error(`ensure health has no ${key}`);
			}
		}
		if (!path.isAbsolute(health.pythonPath)) {
			throw new Error(`ensure returned a relative interpreter: ${health.pythonPath}`);
		}
		await access(health.pythonPath);
		const realTemp = await realpath(tempEnv);
		const realPython = await realpath(health.pythonPath);
		if (!realPython.startsWith(`${realTemp}${path.sep}`)) {
			throw new Error(`ensure used ${realPython}, outside the smoke NPI_ENV_DIR ${realTemp}`);
		}
		if (!health.runtimePath.includes(prepared.manifest.archiveSha256.slice(0, 12))) {
			throw new Error(`runtime path ${health.runtimePath} is not addressed by the archive digest`);
		}

		const { stdout: version } = await execFileP(
			health.pythonPath,
			["-I", "-c", "import sys, ssl, sqlite3, zlib, bz2; print(sys.version.split()[0])"],
			{ timeout: PYTHON_TIMEOUT_MS, maxBuffer: 1024 * 1024 },
		);
		if (version.trim() !== prepared.manifest.runtimeVersion) {
			throw new Error(
				`interpreter reports ${version.trim()}, manifest expects ${prepared.manifest.runtimeVersion}`,
			);
		}

		const environment = await runHost(appExe, { action: "environment" }, env);
		if (!environment.ok) throw new Error(`environment reported unhealthy: ${environment.message}`);
		if (environment.pythonPath !== health.pythonPath) {
			throw new Error("environment and ensure disagree about the interpreter");
		}

		const packages = await runHost(appExe, { action: "packages", operation: "list" }, env);
		if (!Array.isArray(packages)) throw new Error("packages list did not return an array");

		if (process.platform === "darwin") {
			for (const binary of [
				path.join(health.runtimePath, "bin", "python3.12"),
				path.join(health.runtimePath, "bin", "uv"),
			]) {
				await execFileP("codesign", ["--verify", "--strict", binary], {
					timeout: PYTHON_TIMEOUT_MS,
				});
			}
		}

		console.log("[smoke] python runtime OK");
		console.log(`  host:      ${appExe}`);
		console.log(`  resources: ${prepared.resourcesDir}`);
		console.log(`  env:       ${tempEnv}`);
		console.log(`  runtime:   ${health.runtimePath}`);
		console.log(`  python:    ${health.pythonPath} (${health.pythonVersion})`);
		console.log(`  uv:        ${health.uvVersion}`);
		console.log(`  packages:  ${packages.length}`);
	} finally {
		await rm(tempEnv, { recursive: true, force: true });
	}
}

main().catch(error => {
	console.error(`[smoke] ${error.message}`);
	process.exit(1);
});
