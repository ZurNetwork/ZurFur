// Fixture tests for the install guard: each refusal names its rule, and the clean trees pass.
//   node --test frontend/install-check.test.mjs

import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { describe, test } from 'node:test';
import { fileURLToPath } from 'node:url';
import {
	ALLOWED_INSTALL_SCRIPTS,
	INSTALL_COMMAND,
	checkBeforeInstall,
	checkInstalled,
	parseLockfile
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

function write(file, content) {
	fs.mkdirSync(path.dirname(file), { recursive: true });
	fs.writeFileSync(file, typeof content === 'string' ? content : `${JSON.stringify(content, null, '\t')}\n`);
}

/** A clean repo tree in a temp folder: the real base lockfile and web manifest, a minimal ui. */
function fixture(t) {
	const root = fs.mkdtempSync(path.join(os.tmpdir(), 'install-check-'));
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

function rulesOf(result) {
	assert.deepEqual(result.errors, [], 'unexpected unreadable-file errors');
	return new Set(result.findings.map((finding) => finding.rule));
}

function assertRefused(result, rule) {
	const rules = rulesOf(result);
	assert.ok(rules.has(rule), `expected rule ${rule}, got ${JSON.stringify(result.findings)}`);
}

function runCli(args) {
	return spawnSync(process.execPath, [SCRIPT, ...args], { encoding: 'utf8' });
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
		const run = runCli(['--root', root]);
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
		'workspace:*'
	]) {
		test(`refuses the spec ${spec}`, (t) => {
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
	test('ALLOWED_INSTALL_SCRIPTS is empty and frozen', () => {
		assert.deepEqual([...ALLOWED_INSTALL_SCRIPTS], []);
		assert.ok(Object.isFrozen(ALLOWED_INSTALL_SCRIPTS));
	});

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
		const run = runCli(['--installed', '--root', root]);
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
		const run = runCli(['--root', root]);
		assert.equal(run.status, 1, run.stdout + run.stderr);
		const lines = run.stdout.trim().split('\n');
		assert.ok(lines.includes('frontend/.yarnrc: [yarnrc] a yarn rc file is not allowed'), run.stdout);
		assert.ok(lines.some((line) => line.startsWith('frontend/package.json: [root-keys] ')), run.stdout);
	});

	test('exits 2 on an unreadable file', (t) => {
		const { root, file } = fixture(t);
		write(file('web', 'package.json'), '{ not json');
		const run = runCli(['--root', root]);
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
