// The npm licence rule. Reads the licence list (npm-licences.json) strictly, parses the registry's
// `license` field as an SPDX licence expression (stricter than SPDX, failing closed) and judges one
// package version: bundled code fails first, then an exception may admit it, then its licence
// must be readable and allowed by the list. Pure: no network and no file reads.
//
// Not checked: an exception's `textSha256` records which licence text was read; nothing compares
// it with the licence text the package ships or the live licence page.

import { GuardError, parsePlainVersion, splitPattern } from './install-check.mjs';

/** Licence ids the list may never put in `allow`, compared ignoring case. */
export const NEVER_ALLOW = Object.freeze(['MPL-2.0', 'OFL-1.1']);

const MAX_NAME_LENGTH = 214;
const RESERVED_NAMES = Object.freeze(['node_modules', 'favicon.ico']);
const MAX_LIST_BYTES = 64 * 1024;
const MAX_EXPRESSION_LENGTH = 256;
const MAX_NESTING = 8;
const OPERATORS = Object.freeze(['AND', 'OR', 'WITH']);
const ID_STRING = /^[A-Za-z0-9.-]+$/;
const LICENSE_REF = 'licenseref-';
const DOCUMENT_REF = 'documentref-';
const PATTERN_CHARACTERS = /[*]/;
const SHA256_HEX = /^[0-9a-f]{64}$/;
const CALENDAR_DATE = /^(\d{4})-(\d{2})-(\d{2})$/;
const BUNDLE_FIELDS = Object.freeze(['bundleDependencies', 'bundledDependencies']);
const TOP_KEYS = Object.freeze(['allow', 'scoped', 'exceptions']);
const SCOPED_KEYS = Object.freeze(['licence', 'packages', 'reason', 'readBy', 'readOn']);
const EXCEPTION_KEYS = Object.freeze(['package', 'registry', 'terms', 'reason', 'readBy', 'readOn']);
const HASH_KEY = 'textSha256';
const NO_LICENCE = '(no licence field)';

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

function startsWithIgnoringCase(text, prefix) {
	return text.slice(0, prefix.length).toLowerCase() === prefix;
}

function isOperatorIgnoringCase(text) {
	return OPERATORS.includes(text.toUpperCase());
}

/** Why `id` is not a plain licence id (an idstring, no operator, no `+`, no LicenseRef or DocumentRef), or undefined. */
function plainIdProblem(id) {
	if (typeof id !== 'string' || !ID_STRING.test(id)) return 'not a plain licence id';
	if (isOperatorIgnoringCase(id)) return 'an operator, not a licence id';
	if (startsWithIgnoringCase(id, LICENSE_REF) || startsWithIgnoringCase(id, DOCUMENT_REF)) return 'a LicenseRef or DocumentRef';
	return undefined;
}

/** Splits an expression into words and parentheses; null on any character outside the grammar. */
function tokenize(text) {
	const token = /\(|\)|[A-Za-z0-9.-]+\+?| /y;
	const tokens = [];
	while (token.lastIndex < text.length) {
		const match = token.exec(text);
		if (match === null) return null;
		if (match[0] !== ' ') tokens.push(match[0]);
	}
	return tokens;
}

/** The one layout the parser reads: single spaces between tokens, none inside parentheses. */
function layout(tokens) {
	return tokens.map((token, index) => (index > 0 && tokens[index - 1] !== '(' && token !== ')' ? ` ${token}` : token)).join('');
}

/** A licence position: `id`, `id+` or `LicenseRef-idstring`, as a frozen leaf; null otherwise. */
function simpleLicence(word) {
	if (typeof word !== 'string' || word === '(' || word === ')') return null;
	const plus = word.endsWith('+');
	const id = plus ? word.slice(0, -1) : word;
	if (!ID_STRING.test(id) || OPERATORS.includes(id) || startsWithIgnoringCase(id, DOCUMENT_REF)) return null;
	if (startsWithIgnoringCase(id, LICENSE_REF)) {
		if (plus || !ID_STRING.test(id.slice(LICENSE_REF.length))) return null;
		return Object.freeze({ kind: 'ref', id });
	}
	return Object.freeze({ kind: plus ? 'plus' : 'licence', id });
}

/** An exception id after WITH: a plain idstring, no `+`, no operator, no LicenseRef or DocumentRef. */
function exceptionId(word) {
	if (typeof word !== 'string' || !ID_STRING.test(word) || OPERATORS.includes(word)) return null;
	if (startsWithIgnoringCase(word, LICENSE_REF) || startsWithIgnoringCase(word, DOCUMENT_REF)) return null;
	return word;
}

/** Recursive descent over the tokens: `or := and (OR and)*`, `and := with (AND with)*`. */
function parseTokens(tokens) {
	let position = 0;
	let depth = 0;
	const peek = () => tokens[position];

	function parseOr() {
		let left = parseAnd();
		while (left !== null && peek() === 'OR') {
			position += 1;
			const right = parseAnd();
			left = right === null ? null : Object.freeze({ kind: 'or', left, right });
		}
		return left;
	}

	function parseAnd() {
		let left = parseWith();
		while (left !== null && peek() === 'AND') {
			position += 1;
			const right = parseWith();
			left = right === null ? null : Object.freeze({ kind: 'and', left, right });
		}
		return left;
	}

	function parseWith() {
		if (peek() === '(') return parseGroup();
		const licence = simpleLicence(peek());
		if (licence === null) return null;
		position += 1;
		if (peek() !== 'WITH') return licence;
		if (licence.kind === 'ref') return null;
		const exception = exceptionId(tokens[position + 1]);
		if (exception === null) return null;
		position += 2;
		return Object.freeze({ kind: 'with', licence, exception });
	}

	function parseGroup() {
		depth += 1;
		if (depth > MAX_NESTING) return null;
		position += 1;
		const inner = parseOr();
		if (inner === null || peek() !== ')') return null;
		position += 1;
		depth -= 1;
		return inner;
	}

	const tree = parseOr();
	return tree !== null && position === tokens.length ? tree : null;
}

/**
 * Parses an SPDX licence expression into a frozen tree of `licence`, `plus`, `ref` (LicenseRef),
 * `with`, `and` and `or` nodes; null for anything it can't read. Operators are upper case only,
 * single spaces only, at most 256 characters and 8 nested parentheses. Never throws.
 */
export function parseLicenceExpression(text) {
	if (typeof text !== 'string' || text === '' || text.length > MAX_EXPRESSION_LENGTH) return null;
	const tokens = tokenize(text);
	if (tokens === null || tokens.length === 0 || layout(tokens) !== text) return null;
	return parseTokens(tokens);
}

/** True when the tree holds a LicenseRef anywhere. */
function hasLicenseRef(tree) {
	switch (tree.kind) {
		case 'ref':
			return true;
		case 'licence':
		case 'plus':
		case 'with':
			return false;
		case 'and':
		case 'or':
			return hasLicenseRef(tree.left) || hasLicenseRef(tree.right);
	}
}

/** True when a plain id is allowed for `name`: in `allow`, or in a scoped entry naming `name`. */
function admitsId(list, id, name) {
	if (list.allow.has(id)) return true;
	const packages = list.scoped.get(id);
	return packages !== undefined && packages.has(name);
}

/** True when the tree passes the list for `name`; LicenseRefs pass only when `admitRefs` is set. */
function admits(list, tree, name, admitRefs) {
	switch (tree.kind) {
		case 'or':
			return admits(list, tree.left, name, admitRefs) || admits(list, tree.right, name, admitRefs);
		case 'and':
			return admits(list, tree.left, name, admitRefs) && admits(list, tree.right, name, admitRefs);
		case 'licence':
			return admitsId(list, tree.id, name);
		case 'ref':
			return admitRefs;
		case 'plus':
		case 'with':
			return false;
	}
}

/** Why each failing leaf of the tree fails for `name`, in order, without repeats. */
function leafProblems(list, tree, name) {
	switch (tree.kind) {
		case 'or':
		case 'and':
			return [...new Set([...leafProblems(list, tree.left, name), ...leafProblems(list, tree.right, name)])];
		case 'licence':
			if (admitsId(list, tree.id, name)) return [];
			if (list.scoped.has(tree.id)) return [`${JSON.stringify(tree.id)} is allowed only for the packages its scoped entry names`];
			return [`${JSON.stringify(tree.id)} is not allowed`];
		case 'plus':
			return [`${JSON.stringify(`${tree.id}+`)} is never allowed: an "or later" licence`];
		case 'with':
			return [`${JSON.stringify(`${tree.licence.id} WITH ${tree.exception}`)} is never allowed: a licence with an exception`];
		case 'ref':
			return [`${JSON.stringify(tree.id)} is never allowed: a LicenseRef`];
	}
}

/** Why `text` is not a real `YYYY-MM-DD` date, or undefined when it is. */
function calendarDateProblem(text) {
	const match = typeof text === 'string' ? CALENDAR_DATE.exec(text) : null;
	if (match === null) return 'not a YYYY-MM-DD date';
	const [year, month, day] = match.slice(1).map(Number);
	const date = new Date(Date.UTC(year, month - 1, day));
	if (date.getUTCFullYear() !== year || date.getUTCMonth() !== month - 1 || date.getUTCDate() !== day) {
		return 'not a real date';
	}
	return undefined;
}

/** Why `text` is not a real `YYYY-MM-DD` date on or before `today`, or undefined when it is. */
function readOnProblem(text, today) {
	const problem = calendarDateProblem(text);
	if (problem !== undefined) return problem;
	if (text > today) return `after today (${today})`;
	return undefined;
}

function nonEmptyString(value) {
	return typeof value === 'string' && value.trim() !== '';
}

/** Refuses an object whose own keys are not exactly `required` plus any of `optional`. */
function requireKeys(where, object, required, optional = []) {
	if (!isPlainObject(object)) throw new GuardError(`${where}: not an object`);
	const keys = Object.keys(object);
	const unexpected = keys.filter((key) => !required.includes(key) && !optional.includes(key));
	const missing = required.filter((key) => !keys.includes(key));
	if (unexpected.length > 0) throw new GuardError(`${where}: unexpected ${unexpected.map((key) => JSON.stringify(key)).join(', ')}`);
	if (missing.length > 0) throw new GuardError(`${where}: missing ${missing.join(', ')}`);
}

/** Refuses an empty `reason` or `readBy`, or a `readOn` that is not a real date on or before `today`. */
function requireProvenance(where, { reason, readBy, readOn }, today) {
	if (!nonEmptyString(reason)) throw new GuardError(`${where}: reason is empty`);
	if (!nonEmptyString(readBy)) throw new GuardError(`${where}: readBy is empty`);
	const problem = readOnProblem(readOn, today);
	if (problem !== undefined) throw new GuardError(`${where}: readOn ${JSON.stringify(readOn)} is ${problem}`);
}

/** Refuses a package name that fails npm's naming rule or holds a pattern character. */
function requirePackageName(where, name) {
	const problem = packageNameProblem(name);
	if (problem !== undefined) throw new GuardError(`${where}: package name ${JSON.stringify(name)}: ${problem}`);
	if (PATTERN_CHARACTERS.test(name)) throw new GuardError(`${where}: package name ${JSON.stringify(name)} is a pattern`);
}

/** Refuses an array that is not strictly sorted (so also any repeat). */
function requireStrictlySorted(where, values) {
	values.forEach((value, index) => {
		if (index > 0 && !(values[index - 1] < value)) {
			throw new GuardError(`${where}: ${JSON.stringify(value)} is out of order or repeated (the list is strictly sorted)`);
		}
	});
}

function loadAllow(allow) {
	if (!Array.isArray(allow)) throw new GuardError('allow is not an array');
	allow.forEach((id, index) => {
		const problem = plainIdProblem(id);
		if (problem !== undefined) throw new GuardError(`allow[${index}]: ${JSON.stringify(id)} is ${problem}`);
		const never = NEVER_ALLOW.find((refused) => refused.toLowerCase() === id.toLowerCase());
		if (never !== undefined) throw new GuardError(`allow[${index}]: ${JSON.stringify(id)} is never allowed (${never}); give it a scoped entry`);
	});
	requireStrictlySorted('allow', allow);
	return new Set(allow);
}

function loadScoped(scoped, allow, today) {
	if (!Array.isArray(scoped)) throw new GuardError('scoped is not an array');
	const table = new Map();
	scoped.forEach((entry, index) => {
		const where = `scoped[${index}]`;
		requireKeys(where, entry, SCOPED_KEYS);
		const { licence, packages } = entry;
		const idProblem = plainIdProblem(licence);
		if (idProblem !== undefined) throw new GuardError(`${where}: licence ${JSON.stringify(licence)} is ${idProblem}`);
		if (allow.has(licence)) throw new GuardError(`${where}: licence ${JSON.stringify(licence)} is already in allow`);
		if (!Array.isArray(packages) || packages.length === 0) throw new GuardError(`${where}: packages is not a non-empty array`);
		packages.forEach((name, position) => requirePackageName(`${where}.packages[${position}]`, name));
		requireStrictlySorted(`${where}.packages`, packages);
		const names = table.get(licence) ?? new Set();
		for (const name of packages) {
			if (names.has(name)) throw new GuardError(`${where}: a second scoped entry for ${licence} names ${JSON.stringify(name)}`);
			names.add(name);
		}
		table.set(licence, names);
		requireProvenance(where, entry, today);
	});
	return table;
}

function loadExceptions(exceptions, partial, today) {
	if (!Array.isArray(exceptions)) throw new GuardError('exceptions is not an array');
	const table = new Map();
	exceptions.forEach((exception, index) => {
		const where = `exceptions[${index}]`;
		requireKeys(where, exception, EXCEPTION_KEYS, [HASH_KEY]);
		const { package: spec, registry, terms } = exception;
		if (typeof spec !== 'string') throw new GuardError(`${where}: package is not a string`);
		const { name, range } = splitPattern(spec);
		requirePackageName(where, name);
		if (parsePlainVersion(range) === null) throw new GuardError(`${where}: package ${JSON.stringify(spec)} is not an exact name@x.y.z`);
		if (registry !== null && typeof registry !== 'string') throw new GuardError(`${where}: registry is neither a string nor null`);
		if (parseLicenceExpression(registry) !== null) {
			throw new GuardError(
				`${where}: registry ${JSON.stringify(registry)} is an expression the rule can read; an exception is only for a missing or unreadable licence`
			);
		}
		const tree = parseLicenceExpression(terms);
		if (tree === null) throw new GuardError(`${where}: terms ${JSON.stringify(terms)} is not an SPDX expression the rule can read`);
		if (!admits(partial, tree, name, true)) throw new GuardError(`${where}: terms ${JSON.stringify(terms)} does not pass the list for ${name}`);
		const hash = ownValue(exception, HASH_KEY);
		if (hasLicenseRef(tree)) {
			if (typeof hash !== 'string' || !SHA256_HEX.test(hash)) throw new GuardError(`${where}: a LicenseRef in terms needs textSha256 as 64 lower-case hex`);
		} else if (Object.hasOwn(exception, HASH_KEY)) {
			throw new GuardError(`${where}: textSha256 is only for terms holding a LicenseRef`);
		}
		if (table.has(spec)) throw new GuardError(`${where}: a second exception for ${spec}`);
		requireProvenance(where, exception, today);
		table.set(spec, Object.freeze({ ...exception }));
	});
	return table;
}

/**
 * Loads the licence list's text, judged at `today` (`YYYY-MM-DD`, UTC), into a frozen
 * `{ allow: Set, scoped: Map<licence, Set<name>>, exceptions: Map<'name@version', exception> }`.
 * Anything but the exact canonical shape is a GuardError.
 */
export function loadLicenceList(text, { today } = {}) {
	const todayProblem = calendarDateProblem(today);
	if (todayProblem !== undefined) throw new GuardError(`today ${JSON.stringify(today)} is ${todayProblem}`);
	if (typeof text !== 'string') throw new GuardError('the licence list is not text');
	const bytes = new TextEncoder().encode(text).length;
	if (bytes > MAX_LIST_BYTES) throw new GuardError(`the licence list is ${bytes} bytes, over 64 KiB`);
	let document;
	try {
		document = JSON.parse(text);
	} catch (cause) {
		throw new GuardError(`not valid JSON (${cause.message})`);
	}
	if (`${JSON.stringify(document, null, 2)}\n` !== text) {
		throw new GuardError('not in canonical form (two-space JSON, one trailing newline, no repeated keys)');
	}
	if (!isPlainObject(document)) throw new GuardError('the top level is not an object');
	const topKeys = Object.keys(document);
	if (topKeys.length !== TOP_KEYS.length || !TOP_KEYS.every((key) => topKeys.includes(key))) {
		throw new GuardError('the top level must be exactly {"allow", "scoped", "exceptions"}');
	}
	const allow = loadAllow(document.allow);
	const scoped = loadScoped(document.scoped, allow, today);
	const exceptions = loadExceptions(document.exceptions, { allow, scoped }, today);
	return Object.freeze({ allow, scoped, exceptions });
}

/** True when any scoped entry of the list names `name`. */
export function isScopedPackage(list, name) {
	return [...list.scoped.values()].some((packages) => packages.has(name));
}

/** A display string for a non-string licence value. */
function displayValue(value) {
	try {
		const text = JSON.stringify(value);
		if (typeof text === 'string') return text;
	} catch {
		// Falls through to the type name.
	}
	return `(${typeof value})`;
}

/** Why the manifest bundles packages no lockfile lists, or undefined when it bundles none. */
function bundleProblem(manifest) {
	for (const field of BUNDLE_FIELDS) {
		if (!Object.hasOwn(manifest, field)) continue;
		const value = manifest[field];
		const none = value === false || (Array.isArray(value) && value.length === 0);
		if (!none) return `bundles packages no lockfile lists (${JSON.stringify(field)} is ${displayValue(value)})`;
	}
	return undefined;
}

/** True when the exception's `registry` is exactly the manifest's own licence field; null matches only a missing one. */
function registryMatches(exception, manifest) {
	if (exception.registry === null) return !Object.hasOwn(manifest, 'license');
	return Object.hasOwn(manifest, 'license') && manifest.license === exception.registry;
}

function fail(licence, problem) {
	return Object.freeze({ pass: false, licence, problem });
}

/**
 * Judges one package version's registry manifest against the list. Returns a frozen
 * `{ pass: true, via: 'allow' | 'scoped' | 'exception', licence }` or `{ pass: false, licence, problem }`.
 */
export function judgeLicence(list, { name, version, manifest }) {
	if (!isPlainObject(manifest)) return fail(NO_LICENCE, 'the registry manifest is not an object');
	const hasLicence = Object.hasOwn(manifest, 'license');
	const licence = hasLicence ? manifest.license : undefined;
	const shown = !hasLicence ? NO_LICENCE : typeof licence === 'string' ? licence : displayValue(licence);
	if (typeof name !== 'string' || typeof version !== 'string') return fail(shown, 'no package name and version to judge');
	const bundled = bundleProblem(manifest);
	if (bundled !== undefined) return fail(shown, bundled);
	const key = `${name}@${version}`;
	const exception = list.exceptions.get(key);
	if (exception !== undefined && registryMatches(exception, manifest)) {
		return Object.freeze({ pass: true, via: 'exception', licence: shown });
	}
	const unmatched = exception === undefined ? '' : ` (the exception for ${key} names registry ${displayValue(exception.registry)})`;
	if (!hasLicence) {
		const legacy = Object.hasOwn(manifest, 'licenses') ? ', only a legacy "licenses" list' : '';
		return fail(shown, `no licence field${legacy}${unmatched}`);
	}
	if (typeof licence !== 'string') return fail(shown, `the licence field is not a string${unmatched}`);
	const tree = parseLicenceExpression(licence);
	if (tree === null) return fail(shown, `the licence field is not an SPDX expression this rule can read${unmatched}`);
	if (admits(list, tree, name, false)) {
		const via = admits({ allow: list.allow, scoped: new Map() }, tree, name, false) ? 'allow' : 'scoped';
		return Object.freeze({ pass: true, via, licence: shown });
	}
	const problems = leafProblems(list, tree, name);
	const leaf = tree.kind === 'licence' || tree.kind === 'plus' || tree.kind === 'with' || tree.kind === 'ref';
	const problem = leaf ? problems[0] : `${JSON.stringify(licence)} is not met: ${problems.join('; ')}`;
	return fail(shown, `${problem}${unmatched}`);
}
