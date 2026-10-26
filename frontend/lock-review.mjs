#!/usr/bin/env node
// Reviews a yarn v1 lockfile change: lists removed and added blocks and checks each
// added block's integrity against the npm registry. A review tool, not a CI gate.
//
//   node frontend/lock-review.mjs <old> <new> [--no-removals]
//
// Exit codes: 0 clean, 1 findings, 2 unreadable files, bad usage or a failed lookup.
// LOCK_REVIEW_REGISTRY_STUB names a JSON file {"name@version": "sha512-…"} used in place
// of the registry (for tests); the tool says so when it is set.

import fs from 'node:fs';
import process from 'node:process';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { GuardError, parseLockfile, splitPattern } from './install-check.mjs';

const REGISTRY = 'https://registry.npmjs.org';
const LOOKUP_TIMEOUT_MS = 30_000;
const LOOKUP_CONCURRENCY = 8;
const USAGE = 'usage: node frontend/lock-review.mjs <old-lockfile> <new-lockfile> [--no-removals]';

/** The package name a lockfile key installs, following an `npm:` alias. */
export function packageName(key) {
	const { name, range } = splitPattern(key);
	if (!range.startsWith('npm:')) return name;
	return splitPattern(range.slice('npm:'.length)).name;
}

/** Lists the blocks of `oldText` missing from `newText` and the reverse; a changed block is both. */
export function diffLockfiles(oldText, newText) {
	const oldBlocks = parseLockfile(oldText).blocks;
	const newBlocks = parseLockfile(newText).blocks;
	return { removed: subtract(oldBlocks, newBlocks), added: subtract(newBlocks, oldBlocks) };
}

function subtract(from, others) {
	const counts = new Map();
	for (const block of others) counts.set(block.text, (counts.get(block.text) ?? 0) + 1);
	return from.filter((block) => {
		const count = counts.get(block.text) ?? 0;
		if (count === 0) return true;
		counts.set(block.text, count - 1);
		return false;
	});
}

/** The registry URL of one version's manifest; a scoped name's slash is encoded. */
export function registryUrl(name, version) {
	return `${REGISTRY}/${name.replace('/', '%2f')}/${encodeURIComponent(version)}`;
}

/** Fetches `dist.integrity` for `name@version` from the npm registry; rejects on any failure. */
export async function registryLookup(name, version) {
	const url = registryUrl(name, version);
	const response = await fetch(url, { signal: AbortSignal.timeout(LOOKUP_TIMEOUT_MS) });
	if (!response.ok) throw new GuardError(`${url}: HTTP ${response.status}`);
	const manifest = await response.json();
	const integrity = manifest?.dist?.integrity;
	if (typeof integrity !== 'string' || integrity === '') throw new GuardError(`${url}: no dist.integrity`);
	return integrity;
}

/** A lookup reading `{"name@version": integrity}` from a JSON file, for offline runs. */
export function stubLookup(file) {
	const table = JSON.parse(fs.readFileSync(file, 'utf8'));
	return async (name, version) => {
		const integrity = table[`${name}@${version}`];
		if (typeof integrity !== 'string') throw new GuardError(`stub ${file}: no entry for ${name}@${version}`);
		return integrity;
	};
}

function describe(block) {
	const version = block.fields.get('version');
	return `${block.keys.map((key) => JSON.stringify(key)).join(', ')} (version ${typeof version === 'string' ? version : '?'})`;
}

async function mapLimited(items, limit, task) {
	const results = new Array(items.length);
	let next = 0;
	const workers = Array.from({ length: Math.min(limit, items.length) }, async () => {
		while (next < items.length) {
			const index = next;
			next += 1;
			results[index] = await task(items[index]);
		}
	});
	await Promise.all(workers);
	return results;
}

/**
 * Reviews `oldText` → `newText` with `lookup(name, version)` resolving the registry's integrity.
 * Returns the removed and added blocks, findings (exit 1) and errors (exit 2).
 */
export async function reviewLockfiles(oldText, newText, { noRemovals = false, lookup = registryLookup } = {}) {
	const { removed, added } = diffLockfiles(oldText, newText);
	const findings = [];
	const errors = [];
	if (noRemovals) {
		for (const block of removed) findings.push({ rule: 'removal', message: `removed block ${describe(block)}` });
	}
	await mapLimited(added, LOOKUP_CONCURRENCY, async (block) => {
		const names = new Set(block.keys.map(packageName));
		const version = block.fields.get('version');
		if (names.size !== 1 || typeof version !== 'string') {
			errors.push(`cannot look up ${describe(block)}: ${names.size !== 1 ? 'keys name different packages' : 'no version'}`);
			return;
		}
		const [name] = names;
		let expected;
		try {
			expected = await lookup(name, version);
		} catch (cause) {
			errors.push(`lookup failed for ${name}@${version}: ${cause?.message ?? cause}`);
			return;
		}
		const actual = block.fields.get('integrity');
		if (actual !== expected) {
			findings.push({
				rule: 'integrity',
				message: `${name}@${version}: lockfile ${JSON.stringify(actual ?? null)}, registry ${JSON.stringify(expected)}`
			});
		}
	});
	return { removed, added, findings, errors };
}

function parseArgs(argv) {
	const files = [];
	let noRemovals = false;
	for (const arg of argv) {
		if (arg === '--no-removals' && !noRemovals) noRemovals = true;
		else if (arg.startsWith('-')) throw new GuardError(`bad usage: ${JSON.stringify(arg)}\n${USAGE}`);
		else files.push(arg);
	}
	if (files.length !== 2) throw new GuardError(`bad usage: expected two lockfiles\n${USAGE}`);
	return { oldFile: files[0], newFile: files[1], noRemovals };
}

function readFile(file) {
	try {
		return fs.readFileSync(file, 'utf8');
	} catch (cause) {
		throw new GuardError(`${file}: cannot read (${cause.code ?? cause.message})`);
	}
}

/** Runs the review as the CLI does and resolves to its exit code. */
export async function main(argv, out = process.stdout, err = process.stderr, env = process.env) {
	try {
		const { oldFile, newFile, noRemovals } = parseArgs(argv);
		const oldText = readFile(oldFile);
		const newText = readFile(newFile);
		let lookup = registryLookup;
		const stub = env.LOCK_REVIEW_REGISTRY_STUB;
		if (stub) {
			err.write(`lock-review: using the registry stub ${stub}, not the npm registry\n`);
			lookup = stubLookup(stub);
		}
		const { removed, added, findings, errors } = await reviewLockfiles(oldText, newText, { noRemovals, lookup });
		for (const block of removed) out.write(`removed: ${describe(block)}\n`);
		for (const block of added) out.write(`added: ${describe(block)}\n`);
		out.write(`${removed.length} removed, ${added.length} added\n`);
		for (const finding of findings) out.write(`FAIL [${finding.rule}] ${finding.message}\n`);
		for (const error of errors) err.write(`lock-review: ${error}\n`);
		if (errors.length > 0) return 2;
		if (findings.length > 0) return 1;
		out.write(`ok: every added block matches the registry${noRemovals ? ', and nothing was removed' : ''}\n`);
		return 0;
	} catch (cause) {
		const message = cause instanceof GuardError ? cause.message : `error: ${cause?.message ?? cause}`;
		err.write(`lock-review: ${message}\n`);
		return 2;
	}
}

function isEntryPoint() {
	if (!process.argv[1]) return false;
	try {
		return pathToFileURL(fs.realpathSync(process.argv[1])).href === pathToFileURL(fs.realpathSync(fileURLToPath(import.meta.url))).href;
	} catch {
		return false;
	}
}

if (isEntryPoint()) process.exitCode = await main(process.argv.slice(2));
