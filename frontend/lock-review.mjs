#!/usr/bin/env node
// Reviews a lockfile change (yarn v1 or npm lockfileVersion 3): lists the removed and added
// entries, checks each added entry's integrity against the npm registry and, with
// --min-age-days, refuses an added version published fewer than that many days ago unless a
// waiver names it. A review tool, not a CI gate: it needs the network unless a lookup is injected.
//
//   node frontend/lock-review.mjs <old> <new> [--no-removals] [--min-age-days <n> [--waivers <file>]]
//
// Exit codes: 0 clean, 1 findings (integrity, young, removal), 2 errors (unreadable or refused
// files, a refused name or version, a failed lookup, a bad waiver file, bad usage).

import fs from 'node:fs';
import process from 'node:process';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { GuardError, parseLockfile, parsePlainVersion, splitPattern } from './install-check.mjs';

const REGISTRY = 'https://registry.npmjs.org';
const NPM_RESOLVED_PREFIX = `${REGISTRY}/`;
const NODE_MODULES = 'node_modules/';
const YARN_LOCK_HEADER = '# yarn lockfile v1';
const LOOKUP_TIMEOUT_MS = 30_000;
const LOOKUP_CONCURRENCY = 8;
const DAY_MS = 86_400_000;
const MAX_NAME_LENGTH = 214;
const RESERVED_NAMES = Object.freeze(['node_modules', 'favicon.ico']);
const REGISTRY_TIME = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{1,9})?Z$/;
const WAIVER_KEYS = Object.freeze(['package', 'fixes', 'reason', 'diffReadBy', 'diffReadOn']);
const ADVISORY_ID = /^(?:GHSA-[0-9a-z]{4}-[0-9a-z]{4}-[0-9a-z]{4}|CVE-\d{4}-\d{4,})$/;
const CALENDAR_DATE = /^(\d{4})-(\d{2})-(\d{2})$/;
const POSITIVE_INTEGER = /^[1-9]\d*$/;
const NO_WAIVERS = new Map();
const USAGE =
	'usage: node frontend/lock-review.mjs <old-lockfile> <new-lockfile> [--no-removals] [--min-age-days <n> [--waivers <file>]]';

/** True for a JSON-shaped object: not null, not an array, prototype `Object.prototype` or null. */
function isPlainObject(value) {
	if (value === null || typeof value !== 'object' || Array.isArray(value)) return false;
	const prototype = Object.getPrototypeOf(value);
	return prototype === Object.prototype || prototype === null;
}

/** The object's own property `key`, never an inherited one. */
function ownValue(object, key) {
	return Object.hasOwn(object, key) ? object[key] : undefined;
}

/** The package name a lockfile key installs, following an `npm:` alias. */
export function packageName(key) {
	const { name, range } = splitPattern(key);
	if (!range.startsWith('npm:')) return name;
	return splitPattern(range.slice('npm:'.length)).name;
}

/**
 * Why `name` fails npm's naming rule (legacy capitals allowed), or undefined when it passes:
 * at most 214 characters, no leading `.` or `_`, URL-safe parts, `@scope/name` when scoped, no `.` or `..` part.
 */
export function packageNameProblem(name) {
	if (typeof name !== 'string' || name === '') return 'empty';
	if (name.length > MAX_NAME_LENGTH) return `longer than ${MAX_NAME_LENGTH} characters`;
	if (name.startsWith('.') || name.startsWith('_')) return 'starts with . or _';
	if (RESERVED_NAMES.includes(name.toLowerCase())) return 'a reserved name';
	const parts = name.startsWith('@') ? name.slice(1).split('/') : [name];
	if (name.startsWith('@') && parts.length !== 2) return 'a scoped name is not @scope/name';
	for (const part of parts) {
		if (part === '' || part === '.' || part === '..') return 'an empty, . or .. part';
		if (encodeURIComponent(part) !== part) return 'characters that are not URL-safe';
	}
	return undefined;
}

/** The registry URL of a package's full document; a scoped name's slash is encoded. */
export function registryDocumentUrl(name) {
	const problem = packageNameProblem(name);
	if (problem !== undefined) throw new GuardError(`refused package name ${JSON.stringify(name)}: ${problem}`);
	return `${REGISTRY}/${name.replace('/', '%2f')}`;
}

/**
 * Reads `version` out of a registry document: its own `versions[version]` (the manifest, whole)
 * and its own `time[version]`, parsed. Throws a GuardError on any other shape.
 */
export function readRegistryDocument(document, version) {
	if (!isPlainObject(document)) throw new GuardError('the registry document is not an object');
	const versions = ownValue(document, 'versions');
	const time = ownValue(document, 'time');
	if (!isPlainObject(versions)) throw new GuardError('the registry document has no versions object');
	if (!isPlainObject(time)) throw new GuardError('the registry document has no time object');
	const manifest = ownValue(versions, version);
	if (!isPlainObject(manifest)) throw new GuardError(`the registry document has no versions[${JSON.stringify(version)}]`);
	const stamp = ownValue(time, version);
	if (typeof stamp !== 'string') throw new GuardError(`the registry document has no time[${JSON.stringify(version)}]`);
	const publishedMs = REGISTRY_TIME.test(stamp) ? Date.parse(stamp) : Number.NaN;
	if (!Number.isFinite(publishedMs)) throw new GuardError(`time[${JSON.stringify(version)}] is not a date: ${JSON.stringify(stamp)}`);
	return Object.freeze({ manifest, published: new Date(publishedMs) });
}

/** Fetches `name`'s registry document, refusing redirects; resolves to `{ manifest, published }` for `version`. */
export async function registryLookup(name, version) {
	const url = registryDocumentUrl(name);
	const response = await fetch(url, { redirect: 'error', signal: AbortSignal.timeout(LOOKUP_TIMEOUT_MS) });
	if (!response.ok) throw new GuardError(`${url}: HTTP ${response.status}`);
	const document = await response.json();
	try {
		return readRegistryDocument(document, version);
	} catch (cause) {
		throw new GuardError(`${url}: ${cause.message}`);
	}
}

/** Which lockfile format `text` is in: `npm` for a JSON document, `yarn` for a yarn v1 file. */
function lockfileFormat(text) {
	if (/^\s*\{/.test(text)) return 'npm';
	if (text.split('\n').includes(YARN_LOCK_HEADER)) return 'yarn';
	throw new GuardError(`neither an npm lockfile nor a yarn v1 lockfile (no "${YARN_LOCK_HEADER}" line)`);
}

/** One npm v3 `packages` entry as `{ key, name, version, integrity }`; refuses anything the review can't vouch for. */
function npmEntry(key, entry) {
	const where = `packages[${JSON.stringify(key)}]`;
	if (!key.startsWith(NODE_MODULES)) throw new GuardError(`${where}: not under node_modules/`);
	if (!isPlainObject(entry)) throw new GuardError(`${where}: not an object`);
	if (Object.hasOwn(entry, 'link')) throw new GuardError(`${where}: a link entry`);
	if (Object.hasOwn(entry, 'inBundle')) throw new GuardError(`${where}: a bundled entry`);
	const integrity = ownValue(entry, 'integrity');
	if (typeof integrity !== 'string' || integrity === '') throw new GuardError(`${where}: no integrity`);
	const resolved = ownValue(entry, 'resolved');
	if (typeof resolved !== 'string' || !resolved.startsWith(NPM_RESOLVED_PREFIX)) {
		throw new GuardError(`${where}: resolved is not under ${NPM_RESOLVED_PREFIX}`);
	}
	const version = ownValue(entry, 'version');
	if (typeof version !== 'string') throw new GuardError(`${where}: no version`);
	const declared = ownValue(entry, 'name');
	if (declared !== undefined && typeof declared !== 'string') throw new GuardError(`${where}: name is not a string`);
	const name = declared ?? key.slice(key.lastIndexOf(NODE_MODULES) + NODE_MODULES.length);
	return Object.freeze({ key, name, version, integrity });
}

/**
 * Reads an npm lockfileVersion 3 file: every `packages` entry but the root `""`, as
 * `{ key, name, version, integrity }`. Link and bundled entries, entries without integrity and
 * entries resolved outside the npm registry are refused with a GuardError.
 */
export function parseNpmLockfile(text) {
	let document;
	try {
		document = JSON.parse(text);
	} catch (cause) {
		throw new GuardError(`not valid JSON (${cause.message})`);
	}
	if (!isPlainObject(document)) throw new GuardError('not a JSON object');
	const lockfileVersion = ownValue(document, 'lockfileVersion');
	if (lockfileVersion !== 3) throw new GuardError(`lockfileVersion ${JSON.stringify(lockfileVersion ?? null)}, not 3`);
	const packages = ownValue(document, 'packages');
	if (!isPlainObject(packages)) throw new GuardError('no packages object');
	const entries = Object.entries(packages)
		.filter(([key]) => key !== '')
		.map(([key, entry]) => npmEntry(key, entry));
	return Object.freeze(entries);
}

/** A yarn block as a review entry; `unreadable` when its keys name different packages or it has no version. */
function yarnEntry(block) {
	const version = block.fields.get('version');
	const label = `${block.keys.map((key) => JSON.stringify(key)).join(', ')} (version ${typeof version === 'string' ? version : '?'})`;
	const names = new Set(block.keys.map(packageName));
	if (names.size !== 1) return Object.freeze({ kind: 'unreadable', label, reason: 'keys name different packages' });
	if (typeof version !== 'string') return Object.freeze({ kind: 'unreadable', label, reason: 'no version' });
	const [name] = names;
	const integrity = block.fields.get('integrity');
	return Object.freeze({ kind: 'entry', label, name, version, integrity: typeof integrity === 'string' ? integrity : undefined });
}

function subtractBlocks(from, others) {
	const counts = new Map();
	for (const block of others) counts.set(block.text, (counts.get(block.text) ?? 0) + 1);
	return from.filter((block) => {
		const count = counts.get(block.text) ?? 0;
		if (count === 0) return true;
		counts.set(block.text, count - 1);
		return false;
	});
}

/** npm entries grouped by their (name, version, integrity) triple, in first-seen order. */
function npmTriples(entries) {
	const groups = new Map();
	for (const entry of entries) {
		const triple = JSON.stringify([entry.name, entry.version, entry.integrity]);
		const group = groups.get(triple) ?? { entry, keys: [] };
		group.keys.push(entry.key);
		groups.set(triple, group);
	}
	return groups;
}

function subtractTriples(from, others) {
	return [...from]
		.filter(([triple]) => !others.has(triple))
		.map(([, { entry, keys }]) => {
			const label = `${entry.name}@${entry.version} at ${keys.map((key) => JSON.stringify(key)).join(', ')}`;
			const { name, version, integrity } = entry;
			return Object.freeze({ kind: 'entry', label, name, version, integrity });
		});
}

function readLockfile(text, which) {
	try {
		const format = lockfileFormat(text);
		switch (format) {
			case 'yarn':
				return { format, blocks: parseLockfile(text).blocks };
			case 'npm':
				return { format, entries: parseNpmLockfile(text) };
		}
	} catch (cause) {
		if (cause instanceof GuardError) throw new GuardError(`${which} lockfile: ${cause.message}`);
		throw cause;
	}
}

/**
 * Lists the entries of `oldText` missing from `newText` and the reverse. Yarn compares block text
 * (a changed block is both removed and added); npm compares (name, version, integrity) triples.
 * Both files must be in the same format.
 */
export function diffLockfiles(oldText, newText) {
	const oldLock = readLockfile(oldText, 'old');
	const newLock = readLockfile(newText, 'new');
	if (oldLock.format !== newLock.format) {
		throw new GuardError(`the old lockfile is ${oldLock.format} and the new one ${newLock.format}`);
	}
	if (oldLock.format === 'yarn') {
		const removed = subtractBlocks(oldLock.blocks, newLock.blocks).map(yarnEntry);
		const added = subtractBlocks(newLock.blocks, oldLock.blocks).map(yarnEntry);
		return { format: 'yarn', removed, added };
	}
	const oldTriples = npmTriples(oldLock.entries);
	const newTriples = npmTriples(newLock.entries);
	return { format: 'npm', removed: subtractTriples(oldTriples, newTriples), added: subtractTriples(newTriples, oldTriples) };
}

/** Why `text` is not a real `YYYY-MM-DD` date on or before `todayUtc`, or undefined when it is. */
function readOnProblem(text, todayUtc) {
	const match = typeof text === 'string' ? CALENDAR_DATE.exec(text) : null;
	if (match === null) return 'not a YYYY-MM-DD date';
	const [year, month, day] = match.slice(1).map(Number);
	const date = new Date(Date.UTC(year, month - 1, day));
	if (date.getUTCFullYear() !== year || date.getUTCMonth() !== month - 1 || date.getUTCDate() !== day) {
		return 'not a real date';
	}
	if (text > todayUtc) return `after today (${todayUtc})`;
	return undefined;
}

function nonEmptyString(value) {
	return typeof value === 'string' && value.trim() !== '';
}

/** Checks one waiver entry and returns its exact `name@version` key. */
function waiverKey(waiver, index, todayUtc) {
	const where = `waivers[${index}]`;
	if (!isPlainObject(waiver)) throw new GuardError(`${where}: not an object`);
	const keys = Object.keys(waiver);
	const unexpected = keys.filter((key) => !WAIVER_KEYS.includes(key));
	const missing = WAIVER_KEYS.filter((key) => !keys.includes(key));
	if (unexpected.length > 0) throw new GuardError(`${where}: unexpected ${unexpected.map((key) => JSON.stringify(key)).join(', ')}`);
	if (missing.length > 0) throw new GuardError(`${where}: missing ${missing.join(', ')}`);
	const { package: spec, fixes, reason, diffReadBy, diffReadOn } = waiver;
	if (typeof spec !== 'string') throw new GuardError(`${where}: package is not a string`);
	const { name, range } = splitPattern(spec);
	const nameProblem = packageNameProblem(name);
	if (nameProblem !== undefined) throw new GuardError(`${where}: package name ${JSON.stringify(name)}: ${nameProblem}`);
	if (parsePlainVersion(range) === null) throw new GuardError(`${where}: package ${JSON.stringify(spec)} is not an exact name@x.y.z`);
	if (typeof fixes !== 'string' || !ADVISORY_ID.test(fixes)) throw new GuardError(`${where}: fixes is not a GHSA or CVE id`);
	if (!nonEmptyString(reason)) throw new GuardError(`${where}: reason is empty`);
	if (!nonEmptyString(diffReadBy)) throw new GuardError(`${where}: diffReadBy is empty`);
	const readOn = readOnProblem(diffReadOn, todayUtc);
	if (readOn !== undefined) throw new GuardError(`${where}: diffReadOn ${JSON.stringify(diffReadOn)} is ${readOn}`);
	return `${name}@${range}`;
}

/**
 * Parses a waiver file `{"waivers": [...]}` read at `nowMs` into a Map from the exact
 * `name@version` to its waiver. Anything but the exact shape is a GuardError.
 */
export function parseWaivers(text, nowMs) {
	let document;
	try {
		document = JSON.parse(text);
	} catch (cause) {
		throw new GuardError(`not valid JSON (${cause.message})`);
	}
	if (!isPlainObject(document)) throw new GuardError('not a JSON object');
	const topKeys = Object.keys(document);
	if (topKeys.length !== 1 || topKeys[0] !== 'waivers') throw new GuardError('the top level must be exactly {"waivers": [...]}');
	const { waivers } = document;
	if (!Array.isArray(waivers)) throw new GuardError('waivers is not an array');
	const todayUtc = new Date(nowMs).toISOString().slice(0, 10);
	const table = new Map();
	waivers.forEach((waiver, index) => {
		const key = waiverKey(waiver, index, todayUtc);
		if (table.has(key)) throw new GuardError(`waivers[${index}]: a second waiver for ${key}`);
		table.set(key, Object.freeze({ ...waiver }));
	});
	return table;
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

/** The age in days, rounded down to a tenth so a young entry never shows the floor itself. */
function formatAge(ageMs) {
	return Number.isFinite(ageMs) ? `${(Math.floor((ageMs / DAY_MS) * 10) / 10).toFixed(1)} days old` : 'age unknown';
}

function formatDate(published) {
	return published instanceof Date && Number.isFinite(published.getTime()) ? published.toISOString() : 'an unknown date';
}

/** Reviews one added entry: `{ kind: 'error', message }` or `{ kind: 'checked', findings, age }`. */
async function reviewEntry(entry, { lookup, nowMs, minAgeDays, waivers }) {
	if (entry.kind === 'unreadable') return { kind: 'error', message: `cannot look up ${entry.label}: ${entry.reason}` };
	const { name, version } = entry;
	const nameProblem = packageNameProblem(name);
	if (nameProblem !== undefined) return { kind: 'error', message: `refused package name ${JSON.stringify(name)}: ${nameProblem}` };
	if (parsePlainVersion(version) === null) {
		return { kind: 'error', message: `refused version ${JSON.stringify(version)} of ${name}: not strict semver` };
	}
	let found;
	try {
		found = await lookup(name, version);
	} catch (cause) {
		return { kind: 'error', message: `lookup failed for ${name}@${version}: ${cause?.message ?? cause}` };
	}
	const manifest = isPlainObject(found) ? ownValue(found, 'manifest') : undefined;
	const dist = isPlainObject(manifest) ? ownValue(manifest, 'dist') : undefined;
	const expected = isPlainObject(dist) ? ownValue(dist, 'integrity') : undefined;
	if (typeof expected !== 'string' || expected === '') {
		return { kind: 'error', message: `lookup failed for ${name}@${version}: no dist.integrity` };
	}
	const findings = [];
	if (entry.integrity !== expected) {
		const message = `${name}@${version}: lockfile ${JSON.stringify(entry.integrity ?? null)}, registry ${JSON.stringify(expected)}`;
		findings.push({ rule: 'integrity', message });
	}
	if (minAgeDays === undefined) return { kind: 'checked', findings, age: undefined };
	const published = ownValue(found, 'published');
	const ageMs = published instanceof Date ? nowMs - published.getTime() : Number.NaN;
	const young = !(ageMs >= minAgeDays * DAY_MS);
	const waiver = young ? waivers.get(`${name}@${version}`) : undefined;
	if (young && waiver === undefined) {
		const message = `${name}@${version}: published ${formatDate(published)}, ${formatAge(ageMs)}, under the ${minAgeDays}-day floor`;
		findings.push({ rule: 'young', message });
	}
	return { kind: 'checked', findings, age: { published, ageMs, waiver } };
}

/**
 * Reviews `oldText` → `newText`, calling `lookup(name, version)` → `{ manifest, published }` once per
 * added entry. With `minAgeDays`, an added version younger than the floor is a finding unless
 * `waivers` names it. Returns the removed and added entries, each added entry's age line (or
 * undefined), findings (exit 1) and errors (exit 2).
 */
export async function reviewLockfiles(
	oldText,
	newText,
	{ noRemovals = false, lookup = registryLookup, now = Date.now, minAgeDays = undefined, waivers = NO_WAIVERS } = {}
) {
	const { removed, added } = diffLockfiles(oldText, newText);
	const nowMs = now();
	const findings = [];
	const errors = [];
	if (noRemovals) {
		for (const entry of removed) findings.push({ rule: 'removal', message: `removed ${entry.label}` });
	}
	const results = await mapLimited(added, LOOKUP_CONCURRENCY, (entry) =>
		reviewEntry(entry, { lookup, nowMs, minAgeDays, waivers })
	);
	const ages = results.map((result) => {
		switch (result.kind) {
			case 'error':
				errors.push(result.message);
				return undefined;
			case 'checked':
				findings.push(...result.findings);
				return result.age;
		}
	});
	return { removed, added, ages, findings, errors };
}

function ageNote(age) {
	if (age === undefined) return '';
	const waived = age.waiver === undefined ? '' : `, waived (fixes ${age.waiver.fixes})`;
	return `, published ${formatDate(age.published)}, ${formatAge(age.ageMs)}${waived}`;
}

function badUsage(message) {
	return new GuardError(`bad usage: ${message}\n${USAGE}`);
}

function parseArgs(argv) {
	const files = [];
	let noRemovals = false;
	let minAgeDays;
	let waiversFile;
	for (let index = 0; index < argv.length; index += 1) {
		const arg = argv[index];
		if (arg === '--no-removals' && !noRemovals) {
			noRemovals = true;
		} else if (arg === '--min-age-days' && minAgeDays === undefined) {
			const value = argv[(index += 1)];
			const days = typeof value === 'string' && POSITIVE_INTEGER.test(value) ? Number(value) : Number.NaN;
			if (!Number.isSafeInteger(days)) throw badUsage(`--min-age-days takes a positive integer, not ${JSON.stringify(value ?? null)}`);
			minAgeDays = days;
		} else if (arg === '--waivers' && waiversFile === undefined) {
			waiversFile = argv[(index += 1)];
			if (waiversFile === undefined) throw badUsage('--waivers takes a file');
		} else if (arg.startsWith('-')) {
			throw badUsage(JSON.stringify(arg));
		} else {
			files.push(arg);
		}
	}
	if (files.length !== 2) throw badUsage('expected two lockfiles');
	if (waiversFile !== undefined && minAgeDays === undefined) throw badUsage('--waivers needs --min-age-days');
	return { oldFile: files[0], newFile: files[1], noRemovals, minAgeDays, waiversFile };
}

function readFile(file) {
	try {
		return fs.readFileSync(file, 'utf8');
	} catch (cause) {
		throw new GuardError(`${file}: cannot read (${cause.code ?? cause.message})`);
	}
}

/**
 * Runs the review as the CLI does and resolves to its exit code. `lookup` and `now` default to the
 * npm registry and `Date.now`; the CLI passes neither.
 */
export async function main(argv, out = process.stdout, err = process.stderr, { lookup = registryLookup, now = Date.now } = {}) {
	try {
		const { oldFile, newFile, noRemovals, minAgeDays, waiversFile } = parseArgs(argv);
		const nowMs = now();
		const oldText = readFile(oldFile);
		const newText = readFile(newFile);
		let waivers = NO_WAIVERS;
		if (waiversFile !== undefined) {
			const waiverText = readFile(waiversFile);
			try {
				waivers = parseWaivers(waiverText, nowMs);
			} catch (cause) {
				if (cause instanceof GuardError) throw new GuardError(`${waiversFile}: ${cause.message}`);
				throw cause;
			}
		}
		const review = await reviewLockfiles(oldText, newText, { noRemovals, lookup, now: () => nowMs, minAgeDays, waivers });
		const { removed, added, ages, findings, errors } = review;
		for (const entry of removed) out.write(`removed: ${entry.label}\n`);
		added.forEach((entry, index) => out.write(`added: ${entry.label}${ageNote(ages[index])}\n`));
		out.write(`${removed.length} removed, ${added.length} added\n`);
		for (const finding of findings) out.write(`FAIL [${finding.rule}] ${finding.message}\n`);
		for (const error of errors) err.write(`lock-review: ${error}\n`);
		if (errors.length > 0) return 2;
		if (findings.length > 0) return 1;
		const floor = minAgeDays === undefined ? '' : `, every one is at least ${minAgeDays} days old or waived`;
		out.write(`ok: every added entry matches the registry${floor}${noRemovals ? ', and nothing was removed' : ''}\n`);
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
