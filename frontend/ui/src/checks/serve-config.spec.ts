import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import config, { galleryPort } from '../../vite.config.ts';

const loopback = '127.0.0.1';

/** Keys whose presence lets any website read the dev or preview server. */
const openingKeys = ['allowedHosts', 'cors'] as const;

type Fields = Readonly<Record<string, unknown>>;

function isFields(value: unknown): value is Fields {
	return Object(value) === value;
}

/** Why a `server` or `preview` block would be reachable beyond loopback, or bind the wrong port. */
function serverRefusals(block: unknown, expectedPort: number): readonly string[] {
	if (!isFields(block)) return ['the block is missing'];
	const refusals: string[] = [];
	if (block.host !== loopback) refusals.push(`host is ${JSON.stringify(block.host)}`);
	if (block.strictPort !== true) refusals.push('strictPort is not true');
	if (block.port !== expectedPort) refusals.push(`port is ${JSON.stringify(block.port)}`);
	for (const key of openingKeys) {
		if (Object.hasOwn(block, key)) refusals.push(`${key} is set`);
	}
	return refusals;
}

const hostOrPortFlag = /--(?:host|port)/i;

/** The header and body lines of a Justfile recipe, or undefined when it has none. */
function recipeLines(justfile: string, recipe: string): readonly string[] | undefined {
	const lines = justfile.split('\n');
	const header = new RegExp(`^@?${recipe}(?:\\s[^:]*)?:(?!=)`);
	const start = lines.findIndex((line) => header.test(line));
	if (start === -1) return undefined;
	const rest = lines.slice(start + 1);
	const end = rest.findIndex((line) => line.trim() !== '' && !/^\s/.test(line));
	const body = end === -1 ? rest : rest.slice(0, end);
	return [lines[start] ?? '', ...body];
}

function justfileRefusals(justfile: string): readonly string[] {
	const lines = recipeLines(justfile, 'gallery');
	if (lines === undefined) return ['the gallery recipe is missing'];
	return lines.filter((line) => hostOrPortFlag.test(line)).map((line) => line.trim());
}

const servingScripts = ['dev', 'build', 'preview'] as const;

function scriptRefusals(manifest: unknown): readonly string[] {
	const scripts = isFields(manifest) ? manifest.scripts : undefined;
	if (!isFields(scripts)) return ['scripts are missing'];
	return servingScripts.flatMap((name) => {
		const script = scripts[name];
		if (typeof script !== 'string') return [`${name} is missing`];
		return hostOrPortFlag.test(script) ? [`${name}: ${script}`] : [];
	});
}

const repoJustfile = readFileSync(new URL('../../../../Justfile', import.meta.url), 'utf8');
const uiManifest: unknown = JSON.parse(
	readFileSync(new URL('../../package.json', import.meta.url), 'utf8')
);

const port = galleryPort(process.env.ZURFUR_GALLERY_PORT);

describe('galleryPort', () => {
	it.each([undefined, '', 'abc', '0', '-1', '65536', '5190.5', '0x1F90', ' 6000', '6000 '])(
		'falls back to 5190 for %j',
		(raw) => {
			expect(galleryPort(raw)).toBe(5190);
		}
	);

	it.each([
		['6000', 6000],
		['1', 1],
		['65535', 65535]
	])('reads %j as %i', (raw, expected) => {
		expect(galleryPort(raw)).toBe(expected);
	});
});

describe('the serve config', () => {
	it.each(['server', 'preview'] as const)('binds %s to loopback on the gallery port', (key) => {
		expect(serverRefusals(config[key], port)).toEqual([]);
	});

	const goodBlock = { host: loopback, port: 5190, strictPort: true };

	it.each([
		['every interface', { ...goodBlock, host: '0.0.0.0' }],
		['host: true', { ...goodBlock, host: true }],
		['localhost', { ...goodBlock, host: 'localhost' }],
		['IPv6 any', { ...goodBlock, host: '::' }],
		['no host', { port: 5190, strictPort: true }],
		['a loose port', { ...goodBlock, strictPort: false }],
		['no strictPort', { host: loopback, port: 5190 }],
		['another port', { ...goodBlock, port: 5191 }],
		['allowedHosts: true', { ...goodBlock, allowedHosts: true }],
		['an allowedHosts list', { ...goodBlock, allowedHosts: ['.example.org'] }],
		['cors: true', { ...goodBlock, cors: true }],
		['cors: false', { ...goodBlock, cors: false }],
		['no block', undefined]
	])('refuses %s', (_name, block) => {
		expect(serverRefusals(block, 5190)).not.toEqual([]);
	});

	it('passes a loopback block', () => {
		expect(serverRefusals(goodBlock, 5190)).toEqual([]);
	});
});

describe('the gallery recipe', () => {
	it('exists and passes no host or port', () => {
		expect(recipeLines(repoJustfile, 'gallery')).toBeDefined();
		expect(justfileRefusals(repoJustfile)).toEqual([]);
	});

	const recipe = (line: string): string =>
		`other:\n    echo --host\n\ngallery:\n    just frontend-install\n${line}\n\nlater:\n    echo --port\n`;

	it.each([
		'    yarn --cwd frontend/ui run preview --host',
		'    yarn --cwd frontend/ui run preview --host=0.0.0.0',
		'    yarn --cwd frontend/ui run preview --host 0.0.0.0',
		'    yarn --cwd frontend/ui run preview --port 80',
		'    yarn --cwd frontend/ui run preview --port=80',
		'\tyarn --cwd frontend/ui run preview --HOST'
	])('refuses %j', (line) => {
		expect(justfileRefusals(recipe(line))).not.toEqual([]);
	});

	it('refuses a flag on the recipe line itself', () => {
		expect(justfileRefusals('gallery: (serve "--host")\n    echo\n')).not.toEqual([]);
	});

	it('refuses a missing recipe', () => {
		expect(justfileRefusals('galleryx:\n    echo\ngallery-build:\n    echo\n')).toEqual([
			'the gallery recipe is missing'
		]);
	});

	it("passes flags in other recipes and the recipe's own clean lines", () => {
		expect(justfileRefusals(recipe('    yarn --cwd frontend/ui run preview'))).toEqual([]);
	});
});

describe("ui's serving scripts", () => {
	it('pass no host or port', () => {
		expect(scriptRefusals(uiManifest)).toEqual([]);
	});

	const manifest = { scripts: { dev: 'vite dev', build: 'vite build', preview: 'vite preview' } };

	it.each([
		['dev', 'vite dev --host'],
		['build', 'vite build --port=1'],
		['preview', 'vite preview --host=0.0.0.0'],
		['preview', 'vite preview --port 80']
	])('refuses %s: %j', (name, script) => {
		const flagged = { scripts: { ...manifest.scripts, [name]: script } };
		expect(scriptRefusals(flagged)).toEqual([`${name}: ${script}`]);
	});

	it('refuses a missing script', () => {
		expect(scriptRefusals({ scripts: { dev: 'vite dev', build: 'vite build' } })).toEqual([
			'preview is missing'
		]);
	});

	it('passes the plain scripts', () => {
		expect(scriptRefusals(manifest)).toEqual([]);
	});
});
