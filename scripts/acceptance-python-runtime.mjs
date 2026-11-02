#!/usr/bin/env node

/**
 * Offline / concurrency / recovery acceptance for the packaged runtime host.
 *
 * Usage:
 *   node scripts/acceptance-python-runtime.mjs <app-exe> <Resources/resources>
 *
 * Every host process runs under `sandbox-exec` with network denied and an
 * empty PATH, so the deployment cannot reach a package index or a system
 * Python. The caller's NPI_ENV_DIR and NPI_PYTHON_RESOURCE_DIR are untouched.
 */

import { spawn } from "node:child_process";
import { cp, mkdtemp, mkdir, open, readdir, readFile, rm, stat } from "node:fs/promises";
import os from "node:os";
import path from "node:path";

const [appExeArg, resourcesArg] = process.argv.slice(2);
if (!appExeArg || !resourcesArg) {
	console.error("usage: node scripts/acceptance-python-runtime.mjs <app-exe> <Resources/resources>");
	process.exit(2);
}
const appExe = path.resolve(appExeArg);
const resources = path.resolve(resourcesArg);
const manifest = JSON.parse(await readFile(path.join(resources, "python-runtime-manifest.json"), "utf8"));
const profile = "(version 1) (allow default) (deny network*)";

function host(request, env, onStderr) {
	const child = spawn("/usr/bin/sandbox-exec", ["-p", profile, appExe, "--npi-python-host"], {
		env,
		stdio: ["pipe", "pipe", "pipe"],
		detached: true,
	});
	let stdout = "";
	let stderr = "";
	child.stdout.setEncoding("utf8").on("data", chunk => (stdout += chunk));
	child.stderr.setEncoding("utf8").on("data", chunk => {
		stderr += chunk;
		onStderr?.(chunk, child);
	});
	child.stdin.end(`${JSON.stringify(request)}\n`);
	const timer = setTimeout(() => process.kill(-child.pid, "SIGKILL"), 300_000);
	const done = new Promise((resolve, reject) => {
		child.on("error", reject);
		child.on("close", code => {
			clearTimeout(timer);
			resolve({ code, stdout, stderr });
		});
	});
	return { child, done };
}

async function ensure(env, onStderr) {
	const result = await host({ action: "ensure" }, env, onStderr).done;
	if (result.code !== 0) throw new Error(result.stderr.trim() || `ensure exited ${result.code}`);
	const health = JSON.parse(result.stdout);
	if (!health.ok) throw new Error(health.message ?? "unhealthy");
	return health;
}

const scratch = await mkdtemp(path.join(os.tmpdir(), "npi-python-acceptance-"));
const emptyPath = path.join(scratch, "empty-path");
const home = path.join(scratch, "home");
const tmp = path.join(scratch, "tmp");
await Promise.all([mkdir(emptyPath), mkdir(home), mkdir(tmp)]);
const baseEnv = { HOME: home, TMPDIR: tmp, PATH: emptyPath, NPI_PYTHON_RESOURCE_DIR: resources };
const checks = [];
const pass = name => {
	checks.push(name);
	console.log(`PASS ${name}`);
};

try {
	// 1. Fresh offline deployment with no system programs on PATH, then reuse.
	const envA = path.join(scratch, "env a 空格");
	const first = await ensure({ ...baseEnv, NPI_ENV_DIR: envA });
	const before = await stat(first.pythonPath);
	const second = await ensure({ ...baseEnv, NPI_ENV_DIR: envA });
	const after = await stat(second.pythonPath);
	if (first.pythonPath !== second.pythonPath || before.ino !== after.ino) {
		throw new Error("a restarted host did not reuse the installed interpreter");
	}
	pass("offline fresh deployment with empty PATH; restart reuses the interpreter");

	// 2. Two independent host processes racing on one fresh environment.
	const envB = path.join(scratch, "env b");
	const [raceA, raceB] = await Promise.all([
		ensure({ ...baseEnv, NPI_ENV_DIR: envB }),
		ensure({ ...baseEnv, NPI_ENV_DIR: envB }),
	]);
	if (raceA.pythonPath !== raceB.pythonPath) throw new Error("concurrent ensures published different interpreters");
	pass("two concurrent processes share one install");

	// 3. A corrupt archive is rejected and the authentic bundle retries cleanly.
	const corrupt = path.join(scratch, "corrupt resources");
	await mkdir(corrupt);
	await cp(path.join(resources, "python-runtime-manifest.json"), path.join(corrupt, "python-runtime-manifest.json"));
	const corruptArchive = path.join(corrupt, manifest.archiveName);
	await cp(path.join(resources, manifest.archiveName), corruptArchive);
	const handle = await open(corruptArchive, "r+");
	await handle.write(Buffer.from([0]), 0, 1, 0);
	await handle.close();
	const envC = path.join(scratch, "env c");
	const bad = await host({ action: "ensure" }, { ...baseEnv, NPI_ENV_DIR: envC, NPI_PYTHON_RESOURCE_DIR: corrupt }).done;
	if (bad.code === 0 || !/sha|checksum|digest/i.test(bad.stderr)) {
		throw new Error(`a corrupt archive was accepted: ${bad.stdout} ${bad.stderr}`);
	}
	await ensure({ ...baseEnv, NPI_ENV_DIR: envC });
	pass("corrupt archive is rejected; retry with the authentic bundle succeeds");

	// 4. Kill the host during extraction; the next run clears staging and succeeds.
	const envD = path.join(scratch, "env d");
	let killed = false;
	const interrupted = host({ action: "ensure" }, { ...baseEnv, NPI_ENV_DIR: envD }, (chunk, child) => {
		if (!killed && /extracting/.test(chunk)) {
			killed = true;
			process.kill(-child.pid, "SIGKILL");
		}
	});
	const interruptedResult = await interrupted.done;
	if (!killed || interruptedResult.code === 0) throw new Error("the extraction kill did not land");
	const recovered = await ensure({ ...baseEnv, NPI_ENV_DIR: envD });
	const runtimes = await readdir(path.join(envD, "runtimes", "python"));
	if (runtimes.some(name => name.startsWith(".staging-"))) throw new Error("stale staging survived recovery");
	if (!runtimes.some(name => name.includes(manifest.archiveSha256.slice(0, 12)))) {
		throw new Error("recovery did not publish the digest-addressed runtime");
	}
	pass("SIGKILL during extraction recovers on the next run");

	// 5. Real computation and the standard library inside the managed venv.
	const program = [
		"import json, statistics, sqlite3, ssl, zlib",
		"print(json.dumps({'sum': sum(i * i for i in range(10)), 'median': statistics.median([1, 9, 4])}))",
	].join("\n");
	const child = spawn("/usr/bin/sandbox-exec", ["-p", profile, recovered.pythonPath, "-I", "-B", "-c", program], {
		env: { ...baseEnv, VIRTUAL_ENV: recovered.envPath },
		stdio: ["ignore", "pipe", "pipe"],
	});
	let stdout = "";
	let stderr = "";
	child.stdout.on("data", chunk => (stdout += chunk));
	child.stderr.on("data", chunk => (stderr += chunk));
	const code = await new Promise(resolve => child.on("close", resolve));
	if (code !== 0) throw new Error(stderr.trim());
	const value = JSON.parse(stdout);
	if (value.sum !== 285 || value.median !== 4) throw new Error(`unexpected computation ${stdout}`);
	pass("managed interpreter runs standard-library computations offline");

	console.log(JSON.stringify({ appExe, runtimeVersion: manifest.runtimeVersion, checks }, null, 2));
} finally {
	await rm(scratch, { recursive: true, force: true });
}
