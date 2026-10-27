import { describe, expect, it } from 'vitest';
import { existsSync, readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const frontend = fileURLToPath(new URL('../../../', import.meta.url));

type Fields = Readonly<Record<string, unknown>>;

function isFields(value: unknown): value is Fields {
	return Object(value) === value;
}

function readJson(relative: string): Fields {
	const parsed: unknown = JSON.parse(readFileSync(`${frontend}${relative}`, 'utf8'));
	if (!isFields(parsed)) throw new Error(`${relative} is not a JSON object`);
	return parsed;
}

function stringMap(manifest: Fields, key: string): Readonly<Record<string, string>> {
	const value = manifest[key];
	if (value === undefined) return {};
	if (!isFields(value)) throw new Error(`${key} is not an object`);
	const entries = Object.entries(value).map(([name, range]) => {
		if (typeof range !== 'string') throw new Error(`${key}.${name} is not a string`);
		return [name, range] as const;
	});
	return Object.fromEntries(entries);
}

const dependencyKeys = ['dependencies', 'devDependencies', 'peerDependencies'] as const;

function listedRanges(manifest: Fields): ReadonlyMap<string, ReadonlySet<string>> {
	const ranges = new Map<string, Set<string>>();
	for (const key of dependencyKeys) {
		for (const [name, range] of Object.entries(stringMap(manifest, key))) {
			ranges.set(name, (ranges.get(name) ?? new Set()).add(range));
		}
	}
	return ranges;
}

/** Package names mapped to the versions `yarn.lock` resolves them to. */
function lockedVersions(lock: string): ReadonlyMap<string, ReadonlySet<string>> {
	const versions = new Map<string, Set<string>>();
	let current: readonly string[] = [];
	for (const line of lock.split('\n')) {
		if (line !== '' && !line.startsWith(' ') && !line.startsWith('#') && line.endsWith(':')) {
			const specifiers = line.slice(0, -1).split(', ');
			current = specifiers.map((specifier) => {
				const unquoted = specifier.replace(/^"|"$/g, '');
				return unquoted.slice(0, unquoted.lastIndexOf('@'));
			});
			continue;
		}
		const version = /^ {2}version "([^"]+)"$/.exec(line)?.[1];
		if (version === undefined) continue;
		for (const name of current) {
			versions.set(name, (versions.get(name) ?? new Set()).add(version));
		}
		current = [];
	}
	return versions;
}

const singletons = [
	'svelte',
	'@sveltejs/kit',
	'vite',
	'@sveltejs/vite-plugin-svelte',
	'vitest',
	'bits-ui',
	'storybook',
	'@storybook/sveltekit',
	'@storybook/svelte',
	'@storybook/svelte-vite',
	'@storybook/builder-vite',
	'@storybook/addon-vitest',
	'@storybook/addon-a11y',
	'@storybook/addon-docs',
	'@storybook/addon-svelte-csf',
	'@playwright/test',
	'playwright',
	'playwright-core'
] as const;

/** The packages Storybook releases together, at one version. */
const storybookFamily = [
	'storybook',
	'@storybook/sveltekit',
	'@storybook/svelte',
	'@storybook/svelte-vite',
	'@storybook/builder-vite',
	'@storybook/addon-vitest',
	'@storybook/addon-a11y',
	'@storybook/addon-docs',
	'@storybook/react-dom-shim'
] as const;

/** The first Storybook release with both October 2026 advisories fixed. */
const storybookFloor = '10.6.2';

/** Major, minor and patch of a plain release version; undefined for a prerelease or anything else. */
function releaseNumbers(version: string): readonly [number, number, number] | undefined {
	const match = /^(\d+)\.(\d+)\.(\d+)$/.exec(version);
	if (match === null) return undefined;
	return [Number(match[1]), Number(match[2]), Number(match[3])];
}

/** Whether `version` is a release at or above `floor`, compared number by number. */
function atLeast(version: string, floor: string): boolean {
	const numbers = releaseNumbers(version);
	const floorNumbers = releaseNumbers(floor);
	if (numbers === undefined || floorNumbers === undefined) return false;
	const [major, minor, patch] = numbers;
	const [floorMajor, floorMinor, floorPatch] = floorNumbers;
	const order =
		Math.sign(major - floorMajor) || Math.sign(minor - floorMinor) || Math.sign(patch - floorPatch);
	return order >= 0;
}

/** What is wrong with the Storybook family in `lock`: none locked, more than one version, or one under the floor. */
function storybookLockProblems(lock: string): readonly string[] {
	const locked = lockedVersions(lock);
	if (!locked.has('storybook')) return ['storybook is not locked'];
	const versions = new Set(storybookFamily.flatMap((name) => [...(locked.get(name) ?? [])]));
	const several =
		versions.size > 1 ? [`the family is locked at ${[...versions].sort().join(' and ')}`] : [];
	const under = [...versions]
		.filter((version) => !atLeast(version, storybookFloor))
		.map((version) => `${version} is under ${storybookFloor}`);
	return [...several, ...under];
}

/** A lockfile holding one block per `name@version`, in yarn's shape. */
function lockFixture(entries: readonly (readonly [string, string])[]): string {
	const blocks = entries.map(
		([name, version]) => `"${name}@${version}":\n  version "${version}"\n`
	);
	return `# yarn lockfile v1\n\n${blocks.join('\n')}`;
}

const storybookFamilyAt = (version: string) =>
	storybookFamily.map((name) => [name, version] as const);

describe('the frontend workspace', () => {
	it('locks one version each of the shared toolchain', () => {
		const locked = lockedVersions(readFileSync(`${frontend}yarn.lock`, 'utf8'));
		for (const name of singletons) {
			expect({ name, versions: [...(locked.get(name) ?? [])] }).toEqual({
				name,
				versions: [expect.any(String)]
			});
		}
	});

	it('keeps a nested copy of the shared toolchain out of web and ui', () => {
		const nested = ['web', 'ui']
			.flatMap((workspace) => singletons.map((name) => `${workspace}/node_modules/${name}`))
			.filter((relative) => existsSync(`${frontend}${relative}`));
		expect(nested).toEqual([]);
	});

	it('gives every package web and ui both list the same range', () => {
		const web = listedRanges(readJson('web/package.json'));
		const ui = listedRanges(readJson('ui/package.json'));
		const shared = [...ui.keys()].filter((name) => web.has(name));
		expect(shared.length).toBeGreaterThan(0);
		for (const name of shared) {
			const ranges = [...(web.get(name) ?? []), ...(ui.get(name) ?? [])];
			expect({ name, ranges: [...new Set(ranges)] }).toEqual({
				name,
				ranges: [expect.any(String)]
			});
		}
	});

	it('locks the Storybook family at one version, 10.6.2 or later', () => {
		expect(storybookLockProblems(readFileSync(`${frontend}yarn.lock`, 'utf8'))).toEqual([]);
	});

	it('passes a Storybook family fixture at one version at or above the floor', () => {
		expect(storybookLockProblems(lockFixture(storybookFamilyAt('10.6.2')))).toEqual([]);
		expect(storybookLockProblems(lockFixture(storybookFamilyAt('10.10.0')))).toEqual([]);
		expect(storybookLockProblems(lockFixture(storybookFamilyAt('11.0.0')))).toEqual([]);
	});

	it('refuses a Storybook family under 10.6.2', () => {
		expect(storybookLockProblems(lockFixture(storybookFamilyAt('10.6.1')))).toEqual([
			'10.6.1 is under 10.6.2'
		]);
		expect(storybookLockProblems(lockFixture(storybookFamilyAt('9.10.10')))).toEqual([
			'9.10.10 is under 10.6.2'
		]);
		expect(storybookLockProblems(lockFixture(storybookFamilyAt('10.6.2-alpha.1')))).toEqual([
			'10.6.2-alpha.1 is under 10.6.2'
		]);
	});

	it('refuses two versions of one Storybook package', () => {
		const lock = lockFixture([...storybookFamilyAt('10.6.2'), ['@storybook/addon-docs', '10.6.3']]);
		expect(storybookLockProblems(lock)).toEqual(['the family is locked at 10.6.2 and 10.6.3']);
	});

	it('refuses a lock with no storybook', () => {
		const lock = lockFixture(storybookFamilyAt('10.6.2').filter(([name]) => name !== 'storybook'));
		expect(storybookLockProblems(lock)).toEqual(['storybook is not locked']);
	});

	it("pins web's @zurfur/ui to ui's version", () => {
		const ui = readJson('ui/package.json');
		const web = readJson('web/package.json');
		expect(stringMap(web, 'devDependencies')['@zurfur/ui']).toBe(ui.version);
	});

	it('keeps ui private, with one public entry read from source', () => {
		const ui = readJson('ui/package.json');
		expect(ui.name).toBe('@zurfur/ui');
		expect(ui.private).toBe(true);
		expect(ui.exports).toEqual({
			'.': { types: './src/lib/index.ts', svelte: './src/lib/index.ts' },
			'./styles': { types: './src/lib/styles/index.ts', svelte: './src/lib/styles/index.ts' },
			'./tokens.json': './src/lib/styles/tokens.json',
			'./tokens.additions.json': './src/lib/styles/tokens.additions.json'
		});
	});
});
