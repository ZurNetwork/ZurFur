#!/usr/bin/env node
// Reviews npm lockfiles (yarn v1 or npm lockfileVersion 3) against the npm registry, in two modes.
//
// The review of a change lists the entries removed and added between two lockfiles and checks each
// added entry against its full registry document, fetched once: its integrity must match; with
// --licences, its licence must pass the list (licences.mjs) and a package the list scopes must not
// be installed under an alias; with --min-age-days, a version published fewer than that many days
// ago is refused unless a waiver names it (a waiver lifts only the age finding). CI's npm-lock job
// runs the base commit's copy, with the base's licence list, as its 7-day floor.
//
//   node frontend/lock-review.mjs <old> <new> [--no-removals] [--licences <file> [--min-age-days <n> [--waivers <file>]]]
//
// The licence sweep checks every entry of each lockfile, once per (name, version, integrity),
// against its per-version registry document: the integrity must match, the licence must pass the
// list, and a package the list scopes must not be installed under an alias. It prints a count per
// licence and a note for each exception no entry used.
//
//   node frontend/lock-review.mjs --licence-sweep --licences <file> <lockfile>...
//
// Both modes need the network unless the lookups are injected through main().
// Exit codes: 0 clean, 1 findings (integrity, young, licence, removal), 2 errors (unreadable or
// refused files, a refused name or version, a failed lookup, a refused registry document, a bad
// waiver or licence list file, bad usage).

import fs from 'node:fs';
import process from 'node:process';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { GuardError, parseLockfile, parsePlainVersion, splitPattern } from './install-check.mjs';
import {
	isPlainObject,
	isScopedPackage,
	judgeLicence,
	loadLicenceList,
	nonEmptyString,
	ownValue,
	packageNameProblem,
	readOnProblem
} from './licences.mjs';

export { packageNameProblem };

const REGISTRY = 'https://registry.npmjs.org';
const NPM_RESOLVED_PREFIX = `${REGISTRY}/`;
const NODE_MODULES = 'node_modules/';
const YARN_LOCK_HEADER = '# yarn lockfile v1';
const LOOKUP_TIMEOUT_MS = 30_000;
const LOOKUP_CONCURRENCY = 8;
const DAY_MS = 86_400_000;
const REGISTRY_TIME = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{1,9})?Z$/;
const WAIVER_KEYS = Object.freeze(['package', 'fixes', 'reason', 'diffReadBy', 'diffReadOn']);
const ADVISORY_ID = /^(?:GHSA-[0-9a-z]{4}-[0-9a-z]{4}-[0-9a-z]{4}|CVE-\d{4}-\d{4,})$/;
const POSITIVE_INTEGER = /^[1-9]\d*$/;
const NO_WAIVERS = new Map();
const NPM_ALIAS = 'npm:';
const USAGE = [
	'usage: node frontend/lock-review.mjs <old-lockfile> <new-lockfile> [--no-removals] [--licences <file> [--min-age-days <n> [--waivers <file>]]]',
	'       node frontend/lock-review.mjs --licence-sweep --licences <file> <lockfile>...'
].join('\n');

/** The package name a lockfile key installs, following an `npm:` alias. */
export function packageName(key) {
	const { name, range } = splitPattern(key);
	if (!range.startsWith(NPM_ALIAS)) return name;
	return splitPattern(range.slice(NPM_ALIAS.length)).name;
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

/** The registry URL of one version's document; a scoped name's slash is encoded. */
export function registryVersionUrl(name, version) {
	if (parsePlainVersion(version) === null) throw new GuardError(`refused version ${JSON.stringify(version)} of ${name}: not strict semver`);
	return `${registryDocumentUrl(name)}/${encodeURIComponent(version)}`;
}

/** Fetches one version's registry document, refusing redirects; resolves to the parsed JSON, unread. */
export async function registryVersionLookup(name, version) {
	const url = registryVersionUrl(name, version);
	const response = await fetch(url, { redirect: 'error', signal: AbortSignal.timeout(LOOKUP_TIMEOUT_MS) });
	if (!response.ok) throw new GuardError(`${url}: HTTP ${response.status}`);
	return response.json();
}

/**
 * Reads a per-version registry document for `name`@`version`: its own `name` and `version` must be
 * exactly those, and its own `dist.integrity` a non-empty string. Returns `{ manifest, integrity }`,
 * the manifest being the document whole. Throws a GuardError on any other shape.
 */
export function readVersionDocument(document, name, version) {
	if (!isPlainObject(document)) throw new GuardError('the version document is not an object');
	const ownName = ownValue(document, 'name');
	if (ownName !== name) throw new GuardError(`the version document names package ${JSON.stringify(ownName ?? null)}, not ${JSON.stringify(name)}`);
	const ownVersion = ownValue(document, 'version');
	if (ownVersion !== version) {
		throw new GuardError(`the version document is version ${JSON.stringify(ownVersion ?? null)}, not ${JSON.stringify(version)}`);
	}
	const dist = ownValue(document, 'dist');
	const integrity = isPlainObject(dist) ? ownValue(dist, 'integrity') : undefined;
	if (typeof integrity !== 'string' || integrity === '') throw new GuardError('the version document has no dist.integrity');
	return Object.freeze({ manifest: document, integrity });
}

/** Which lockfile format `text` is in: `npm` for a JSON document, `yarn` for a yarn v1 file. */
function lockfileFormat(text) {
	if (/^\s*\{/.test(text)) return 'npm';
	if (text.split('\n').includes(YARN_LOCK_HEADER)) return 'yarn';
	throw new GuardError(`neither an npm lockfile nor a yarn v1 lockfile (no "${YARN_LOCK_HEADER}" line)`);
}

/**
 * One npm v3 `packages` entry as `{ key, name, version, integrity, alias }`, `alias` being its folder
 * name when its own `name` differs, else undefined; refuses anything the review can't vouch for.
 */
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
	const folder = key.slice(key.lastIndexOf(NODE_MODULES) + NODE_MODULES.length);
	const name = declared ?? folder;
	const alias = name === folder ? undefined : folder;
	return Object.freeze({ key, name, version, integrity, alias });
}

/**
 * Reads an npm lockfileVersion 3 file: every `packages` entry but the root `""`, as
 * `{ key, name, version, integrity, alias }`. Link and bundled entries, entries without integrity and
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

/** The names a yarn block's `npm:` alias keys install under, in first-seen order. */
function yarnAliases(keys) {
	const aliases = keys.map(splitPattern).filter(({ range }) => range.startsWith(NPM_ALIAS));
	return Object.freeze([...new Set(aliases.map(({ name }) => name))]);
}

/**
 * A yarn block as a review entry, carrying the names its `npm:` alias keys install under;
 * `unreadable` when its keys name different packages or it has no version.
 */
function yarnEntry(block) {
	const version = block.fields.get('version');
	const label = `${block.keys.map((key) => JSON.stringify(key)).join(', ')} (version ${typeof version === 'string' ? version : '?'})`;
	const names = new Set(block.keys.map(packageName));
	if (names.size !== 1) return Object.freeze({ kind: 'unreadable', label, reason: 'keys name different packages' });
	if (typeof version !== 'string') return Object.freeze({ kind: 'unreadable', label, reason: 'no version' });
	const [name] = names;
	const integrity = block.fields.get('integrity');
	const aliases = yarnAliases(block.keys);
	return Object.freeze({ kind: 'entry', label, name, version, integrity: typeof integrity === 'string' ? integrity : undefined, aliases });
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

/** The (name, version, integrity) triple an entry is grouped and compared by. */
function tripleOf(entry) {
	return JSON.stringify([entry.name, entry.version, entry.integrity]);
}

/** npm entries grouped by their triple, in first-seen order, each with its keys and alias names. */
function npmTriples(entries) {
	const groups = new Map();
	for (const entry of entries) {
		const triple = tripleOf(entry);
		const group = groups.get(triple) ?? { entry, keys: [], aliases: new Set() };
		group.keys.push(entry.key);
		if (entry.alias !== undefined) group.aliases.add(entry.alias);
		groups.set(triple, group);
	}
	return groups;
}

/** One triple's group as a review entry. */
function tripleEntry({ entry, keys, aliases }) {
	const label = `${entry.name}@${entry.version} at ${keys.map((key) => JSON.stringify(key)).join(', ')}`;
	const { name, version, integrity } = entry;
	return Object.freeze({ kind: 'entry', label, name, version, integrity, aliases: Object.freeze([...aliases]) });
}

/** The triples of `from` that `others` lacks. */
function subtractTriples(from, others) {
	return [...from].filter(([triple]) => !others.has(triple)).map(([, group]) => tripleEntry(group));
}

/** The triples of `newer` that `older` lacks, and those installed under an alias `older` lacked. */
function addedTriples(newer, older) {
	return [...newer]
		.filter(([triple, group]) => {
			const old = older.get(triple);
			return old === undefined || [...group.aliases].some((alias) => !old.aliases.has(alias));
		})
		.map(([, group]) => tripleEntry(group));
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
 * (a changed block is both removed and added); npm compares (name, version, integrity) triples,
 * and a triple newly installed under an alias counts as added. Both files must be in the same format.
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
	return { format: 'npm', removed: subtractTriples(oldTriples, newTriples), added: addedTriples(newTriples, oldTriples) };
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
	const todayUtc = utcDate(nowMs);
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

/** The `YYYY-MM-DD` UTC date of `nowMs`. */
function utcDate(nowMs) {
	return new Date(nowMs).toISOString().slice(0, 10);
}

/**
 * The licence findings for one entry: the verdict's failure, and each alias a package the list
 * scopes is installed under. `lockfile` names the lockfile in each message.
 */
function licenceFindings(list, entry, verdict, lockfile) {
	const { name, version } = entry;
	const where = `${name}@${version} (${lockfile})`;
	const findings = [];
	if (!verdict.pass) findings.push({ rule: 'licence', message: `${where}: ${verdict.problem}` });
	if (isScopedPackage(list, name)) {
		for (const alias of entry.aliases ?? []) {
			const message = `${where}: installed under the alias ${JSON.stringify(alias)}, and the list scopes ${name} to named packages`;
			findings.push({ rule: 'licence', message });
		}
	}
	return findings;
}

/** Why an entry can't be looked up (unreadable, a refused name or version), or undefined. */
function lookupProblem(entry) {
	if (entry.kind === 'unreadable') return `cannot look up ${entry.label}: ${entry.reason}`;
	const { name, version } = entry;
	const nameProblem = packageNameProblem(name);
	if (nameProblem !== undefined) return `refused package name ${JSON.stringify(name)}: ${nameProblem}`;
	if (parsePlainVersion(version) === null) return `refused version ${JSON.stringify(version)} of ${name}: not strict semver`;
	return undefined;
}

/** Reviews one added entry: `{ kind: 'error', message }` or `{ kind: 'checked', findings, age }`. */
async function reviewEntry(entry, { lookup, nowMs, minAgeDays, waivers, licences, lockfile }) {
	const problem = lookupProblem(entry);
	if (problem !== undefined) return { kind: 'error', message: problem };
	const { name, version } = entry;
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
	let age;
	if (minAgeDays !== undefined) {
		const published = ownValue(found, 'published');
		const ageMs = published instanceof Date ? nowMs - published.getTime() : Number.NaN;
		const young = !(ageMs >= minAgeDays * DAY_MS);
		const waiver = young ? waivers.get(`${name}@${version}`) : undefined;
		if (young && waiver === undefined) {
			const message = `${name}@${version}: published ${formatDate(published)}, ${formatAge(ageMs)}, under the ${minAgeDays}-day floor`;
			findings.push({ rule: 'young', message });
		}
		age = { published, ageMs, waiver };
	}
	if (licences !== undefined) {
		const verdict = judgeLicence(licences, { name, version, manifest });
		findings.push(...licenceFindings(licences, entry, verdict, lockfile));
	}
	return { kind: 'checked', findings, age };
}

/**
 * Reviews `oldText` → `newText`, calling `lookup(name, version)` → `{ manifest, published }` once per
 * added entry. With `minAgeDays`, an added version younger than the floor is a finding unless
 * `waivers` names it. With `licences` (a loaded list), an added entry whose licence fails the list,
 * or a package the list scopes installed under an alias, is a finding naming `lockfile`. Returns the
 * removed and added entries, each added entry's age line (or undefined), findings (exit 1) and
 * errors (exit 2).
 */
export async function reviewLockfiles(
	oldText,
	newText,
	{
		noRemovals = false,
		lookup = registryLookup,
		now = Date.now,
		minAgeDays = undefined,
		waivers = NO_WAIVERS,
		licences = undefined,
		lockfile = 'the new lockfile'
	} = {}
) {
	const { removed, added } = diffLockfiles(oldText, newText);
	const nowMs = now();
	const findings = [];
	const errors = [];
	if (noRemovals) {
		for (const entry of removed) findings.push({ rule: 'removal', message: `removed ${entry.label}` });
	}
	const results = await mapLimited(added, LOOKUP_CONCURRENCY, (entry) =>
		reviewEntry(entry, { lookup, nowMs, minAgeDays, waivers, licences, lockfile })
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

/** Every entry of one lockfile's text, once per (name, version, integrity), alias names merged; unreadable yarn blocks kept. */
function sweepEntries(text) {
	const format = lockfileFormat(text);
	switch (format) {
		case 'npm':
			return [...npmTriples(parseNpmLockfile(text)).values()].map(tripleEntry);
		case 'yarn': {
			const unreadable = [];
			const groups = new Map();
			for (const entry of parseLockfile(text).blocks.map(yarnEntry)) {
				if (entry.kind === 'unreadable') {
					unreadable.push(entry);
					continue;
				}
				const triple = tripleOf(entry);
				const seen = groups.get(triple);
				const aliases = seen === undefined ? entry.aliases : [...new Set([...seen.aliases, ...entry.aliases])];
				groups.set(triple, Object.freeze({ ...(seen ?? entry), aliases: Object.freeze(aliases) }));
			}
			return [...unreadable, ...groups.values()];
		}
	}
}

/** Sweeps one entry: `{ kind: 'error', message }` or `{ kind: 'judged', key, verdict, findings }`. */
async function sweepEntry(entry, { list, versionLookup, lockfile }) {
	const problem = lookupProblem(entry);
	if (problem !== undefined) return { kind: 'error', message: `${lockfile}: ${problem}` };
	const { name, version } = entry;
	const where = `${name}@${version} (${lockfile})`;
	let document;
	try {
		document = await versionLookup(name, version);
	} catch (cause) {
		return { kind: 'error', message: `lookup failed for ${where}: ${cause?.message ?? cause}` };
	}
	let read;
	try {
		read = readVersionDocument(document, name, version);
	} catch (cause) {
		if (!(cause instanceof GuardError)) throw cause;
		return { kind: 'error', message: `${where}: ${cause.message}` };
	}
	const findings = [];
	if (entry.integrity !== read.integrity) {
		const message = `${where}: lockfile ${JSON.stringify(entry.integrity ?? null)}, registry ${JSON.stringify(read.integrity)}`;
		findings.push({ rule: 'integrity', message });
	}
	const verdict = judgeLicence(list, { name, version, manifest: read.manifest });
	findings.push(...licenceFindings(list, entry, verdict, lockfile));
	return { kind: 'judged', key: `${name}@${version}`, verdict, findings };
}

/**
 * Sweeps every entry of each `{ label, text }` lockfile against `list` (a loaded licence list),
 * calling `versionLookup(name, version)` → the per-version registry document once per entry per
 * lockfile. Returns the count of entries per licence, the list's exceptions no entry used,
 * findings (exit 1) and errors (exit 2).
 */
export async function sweepLockfiles(lockfiles, { list, versionLookup = registryVersionLookup }) {
	const counts = new Map();
	const used = new Set();
	const findings = [];
	const errors = [];
	for (const { label, text } of lockfiles) {
		let entries;
		try {
			entries = sweepEntries(text);
		} catch (cause) {
			if (!(cause instanceof GuardError)) throw cause;
			errors.push(`${label}: ${cause.message}`);
			continue;
		}
		const results = await mapLimited(entries, LOOKUP_CONCURRENCY, (entry) => sweepEntry(entry, { list, versionLookup, lockfile: label }));
		for (const result of results) {
			switch (result.kind) {
				case 'error':
					errors.push(result.message);
					break;
				case 'judged':
					counts.set(result.verdict.licence, (counts.get(result.verdict.licence) ?? 0) + 1);
					if (result.verdict.pass && result.verdict.via === 'exception') used.add(result.key);
					findings.push(...result.findings);
					break;
			}
		}
	}
	const sortedCounts = [...counts].toSorted(([left], [right]) => (left < right ? -1 : left > right ? 1 : 0));
	const unusedExceptions = [...list.exceptions.keys()].filter((key) => !used.has(key));
	return { counts: sortedCounts, unusedExceptions, findings, errors };
}

function ageNote(age) {
	if (age === undefined) return '';
	const waived = age.waiver === undefined ? '' : `, waived (fixes ${age.waiver.fixes})`;
	return `, published ${formatDate(age.published)}, ${formatAge(age.ageMs)}${waived}`;
}

function badUsage(message) {
	return new GuardError(`bad usage: ${message}\n${USAGE}`);
}

/** The command line as `{ mode: 'review', … }` or `{ mode: 'sweep', licencesFile, lockfiles }`; bad usage is a GuardError. */
function parseArgs(argv) {
	const files = [];
	let noRemovals = false;
	let sweep = false;
	let minAgeDays;
	let waiversFile;
	let licencesFile;
	for (let index = 0; index < argv.length; index += 1) {
		const arg = argv[index];
		if (arg === '--no-removals' && !noRemovals) {
			noRemovals = true;
		} else if (arg === '--licence-sweep' && !sweep) {
			sweep = true;
		} else if (arg === '--min-age-days' && minAgeDays === undefined) {
			const value = argv[(index += 1)];
			const days = typeof value === 'string' && POSITIVE_INTEGER.test(value) ? Number(value) : Number.NaN;
			if (!Number.isSafeInteger(days)) throw badUsage(`--min-age-days takes a positive integer, not ${JSON.stringify(value ?? null)}`);
			minAgeDays = days;
		} else if (arg === '--waivers' && waiversFile === undefined) {
			waiversFile = argv[(index += 1)];
			if (waiversFile === undefined) throw badUsage('--waivers takes a file');
		} else if (arg === '--licences' && licencesFile === undefined) {
			licencesFile = argv[(index += 1)];
			if (licencesFile === undefined) throw badUsage('--licences takes a file');
		} else if (arg.startsWith('-')) {
			throw badUsage(JSON.stringify(arg));
		} else {
			files.push(arg);
		}
	}
	if (sweep) {
		if (licencesFile === undefined) throw badUsage('--licence-sweep needs --licences');
		if (files.length === 0) throw badUsage('--licence-sweep needs at least one lockfile');
		if (minAgeDays !== undefined || waiversFile !== undefined || noRemovals) {
			throw badUsage('--licence-sweep takes no --min-age-days, --waivers or --no-removals');
		}
		return { mode: 'sweep', licencesFile, lockfiles: files };
	}
	if (files.length !== 2) throw badUsage('expected two lockfiles');
	if (waiversFile !== undefined && minAgeDays === undefined) throw badUsage('--waivers needs --min-age-days');
	if (minAgeDays !== undefined && licencesFile === undefined) throw badUsage('--min-age-days needs --licences');
	return { mode: 'review', oldFile: files[0], newFile: files[1], noRemovals, minAgeDays, waiversFile, licencesFile };
}

function readFile(file) {
	try {
		return fs.readFileSync(file, 'utf8');
	} catch (cause) {
		throw new GuardError(`${file}: cannot read (${cause.code ?? cause.message})`);
	}
}

/** Reads and loads the licence list at `file`, judged on the UTC date of `nowMs`; a refusal names the file. */
function readLicenceList(file, nowMs) {
	const text = readFile(file);
	try {
		return loadLicenceList(text, { today: utcDate(nowMs) });
	} catch (cause) {
		if (cause instanceof GuardError) throw new GuardError(`${file}: ${cause.message}`);
		throw cause;
	}
}

/** Runs the review of a change and resolves to its exit code. */
async function runReview({ oldFile, newFile, noRemovals, minAgeDays, waiversFile, licencesFile }, out, err, { lookup, nowMs }) {
	const oldText = readFile(oldFile);
	const newText = readFile(newFile);
	const licences = licencesFile === undefined ? undefined : readLicenceList(licencesFile, nowMs);
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
	const options = { noRemovals, lookup, now: () => nowMs, minAgeDays, waivers, licences, lockfile: newFile };
	const { removed, added, ages, findings, errors } = await reviewLockfiles(oldText, newText, options);
	for (const entry of removed) out.write(`removed: ${entry.label}\n`);
	added.forEach((entry, index) => out.write(`added: ${entry.label}${ageNote(ages[index])}\n`));
	out.write(`${removed.length} removed, ${added.length} added\n`);
	for (const finding of findings) out.write(`FAIL [${finding.rule}] ${finding.message}\n`);
	for (const error of errors) err.write(`lock-review: ${error}\n`);
	if (errors.length > 0) return 2;
	if (findings.length > 0) return 1;
	const licensed = licences === undefined ? '' : ', every one passes the licence list';
	const floor = minAgeDays === undefined ? '' : `, every one is at least ${minAgeDays} days old or waived`;
	out.write(`ok: every added entry matches the registry${licensed}${floor}${noRemovals ? ', and nothing was removed' : ''}\n`);
	return 0;
}

/** Runs the licence sweep and resolves to its exit code. */
async function runSweep({ licencesFile, lockfiles }, out, err, { versionLookup, nowMs }) {
	const list = readLicenceList(licencesFile, nowMs);
	const texts = [];
	const unreadable = [];
	for (const label of lockfiles) {
		try {
			texts.push({ label, text: readFile(label) });
		} catch (cause) {
			if (!(cause instanceof GuardError)) throw cause;
			unreadable.push(cause.message);
		}
	}
	const sweep = await sweepLockfiles(texts, { list, versionLookup });
	const errors = [...unreadable, ...sweep.errors];
	for (const [licence, count] of sweep.counts) out.write(`licence ${licence}: ${count}\n`);
	for (const key of sweep.unusedExceptions) out.write(`note: the exception for ${key} matches no entry\n`);
	for (const finding of sweep.findings) out.write(`FAIL [${finding.rule}] ${finding.message}\n`);
	for (const error of errors) err.write(`lock-review: ${error}\n`);
	if (errors.length > 0) return 2;
	if (sweep.findings.length > 0) return 1;
	out.write(`ok: every entry of ${lockfiles.join(', ')} matches the registry and passes the licence list\n`);
	return 0;
}

/**
 * Runs the review or the sweep as the CLI does and resolves to its exit code. `lookup` (the full
 * document), `versionLookup` (one version's document) and `now` default to the npm registry and
 * `Date.now`; the CLI passes none.
 */
export async function main(
	argv,
	out = process.stdout,
	err = process.stderr,
	{ lookup = registryLookup, versionLookup = registryVersionLookup, now = Date.now } = {}
) {
	try {
		const args = parseArgs(argv);
		const nowMs = now();
		switch (args.mode) {
			case 'review':
				return await runReview(args, out, err, { lookup, nowMs });
			case 'sweep':
				return await runSweep(args, out, err, { versionLookup, nowMs });
		}
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
