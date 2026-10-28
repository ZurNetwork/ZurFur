/*
 * The fixture pictures and the folder that serves them. Kit serves `static/`
 * at the site root and Storybook serves `static/fixtures` at `/fixtures`, both
 * following links and showing dotfiles, so the folder is read here with lstat,
 * never following a link: `static/` holds only `fixtures/`, a real folder,
 * which holds only plain `.svg` files named on the fixture list, and each
 * one is read against the fixtures' allow-list of elements and attributes.
 */
import { afterAll, describe, expect, it } from 'vitest';
import {
	lstatSync,
	mkdirSync,
	mkdtempSync,
	readdirSync,
	readFileSync,
	rmSync,
	symlinkSync,
	writeFileSync
} from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { FIXTURE_FOLDER, FIXTURE_NAMES, fixtureSvgRefusals } from './built-output.ts';

const staticRoot = fileURLToPath(new URL('../../static/', import.meta.url));

type EntryKind = 'file' | 'folder' | 'link' | 'other';

/** What `entry` is in itself, read with lstat so a link is never followed. */
function kindOf(entry: string): EntryKind {
	const stats = lstatSync(entry);
	if (stats.isSymbolicLink()) return 'link';
	if (stats.isFile()) return 'file';
	if (stats.isDirectory()) return 'folder';
	return 'other';
}

/** The names in `folder`, read without following links. */
const namesIn = (folder: string): readonly string[] => readdirSync(folder).sort();

/** Why one fixtures entry is not a plain, listed SVG that passes the allow-list. */
function fixtureRefusals(folder: string, name: string): readonly string[] {
	const entry = path.join(folder, name);
	const where = `${FIXTURE_FOLDER}/${name}`;
	const kind = kindOf(entry);
	if (name.startsWith('.')) return [`${where} is a dotfile`];
	if (kind !== 'file') return [`${where} is a ${kind}, not a plain file`];
	if (!FIXTURE_NAMES.includes(name)) return [`${where} is not on the fixture list`];
	if (!name.endsWith('.svg')) return [`${where} is not an .svg`];
	return fixtureSvgRefusals(readFileSync(entry, 'utf8')).map((reason) => `${where}: ${reason}`);
}

/**
 * Every refusal of a `static/` folder: it is a real folder holding only
 * `fixtures/`, itself a real folder; no sibling's name starts with `fixtures`;
 * and every entry of `fixtures/` passes `fixtureRefusals`.
 */
function staticRefusals(root: string): readonly string[] {
	const rootKind = kindOf(root);
	if (rootKind !== 'folder') return [`static/ is a ${rootKind}, not a real folder`];
	const siblings = namesIn(root).filter((name) => name !== FIXTURE_FOLDER);
	const lookalikes = siblings
		.filter((name) => name.toLowerCase().startsWith(FIXTURE_FOLDER))
		.map((name) => `${name} beside fixtures/ starts with "fixtures"`);
	const others = siblings.map((name) => `static/${name} is not the fixtures folder`);
	const fixtures = path.join(root, FIXTURE_FOLDER);
	if (!namesIn(root).includes(FIXTURE_FOLDER)) return [...lookalikes, ...others, 'no fixtures/'];
	const fixturesKind = kindOf(fixtures);
	if (fixturesKind !== 'folder') {
		return [...lookalikes, ...others, `fixtures/ is a ${fixturesKind}, not a real folder`];
	}
	const entries = namesIn(fixtures).flatMap((name) => fixtureRefusals(fixtures, name));
	return [...lookalikes, ...others, ...entries];
}

/** Every planted folder lives under one temporary folder, removed after the run. */
const scratch = mkdtempSync(path.join(tmpdir(), 'ui-fixtures-'));

afterAll(() => {
	rmSync(scratch, { recursive: true, force: true });
});

let plantCount = 0;

/** A plain avatar like the real one. */
const avatar = readFileSync(path.join(staticRoot, FIXTURE_FOLDER, 'avatar.svg'), 'utf8');

/** A `static/` folder holding `files` (path to text) and the real avatar, returned by path. */
function plantedStatic(files: Readonly<Record<string, string>> = {}): string {
	plantCount += 1;
	const root = path.join(scratch, `static-${String(plantCount)}`);
	const all = { [`${FIXTURE_FOLDER}/avatar.svg`]: avatar, ...files };
	for (const [relative, text] of Object.entries(all)) {
		const file = path.join(root, relative);
		mkdirSync(path.dirname(file), { recursive: true });
		writeFileSync(file, text);
	}
	return root;
}

/** An SVG holding `inner` inside a plain root. */
const svg = (inner: string): string =>
	`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 8 8">${inner}</svg>\n`;

describe('the fixture pictures', () => {
	it('holds the listed fixtures, plain and on the allow-list', () => {
		expect(namesIn(path.join(staticRoot, FIXTURE_FOLDER))).toEqual([...FIXTURE_NAMES].sort());
		expect(staticRefusals(staticRoot)).toEqual([]);
	});

	it('passes a planted copy of the real folder', () => {
		expect(staticRefusals(plantedStatic())).toEqual([]);
	});

	it.each([
		['a dotfile', { 'fixtures/.env': 'X=1' }, 'dotfile'],
		['a subfolder', { 'fixtures/deep/avatar.svg': avatar }, 'folder'],
		['an unlisted name', { 'fixtures/probe2.svg': avatar }, 'not on the fixture list'],
		['a sibling named like fixtures', { 'fixtures-old/x.svg': avatar }, 'starts with "fixtures"'],
		['another file in static/', { 'robots.txt': 'x' }, 'not the fixtures folder']
	] as const)('refuses %s', (_label, files, reason) => {
		expect(staticRefusals(plantedStatic(files))).toContainEqual(expect.stringContaining(reason));
	});

	it('refuses a file link in the fixtures folder', () => {
		const root = plantedStatic();
		const outside = path.join(scratch, `secret-${String(plantCount)}`);
		writeFileSync(outside, 'secret');
		rmSync(path.join(root, FIXTURE_FOLDER, 'avatar.svg'));
		symlinkSync(outside, path.join(root, FIXTURE_FOLDER, 'avatar.svg'));
		expect(staticRefusals(root)).toEqual(['fixtures/avatar.svg is a link, not a plain file']);
	});

	it('refuses the fixtures folder as a link', () => {
		const root = plantedStatic();
		const home = plantedStatic();
		rmSync(path.join(root, FIXTURE_FOLDER), { recursive: true });
		symlinkSync(path.join(home, FIXTURE_FOLDER), path.join(root, FIXTURE_FOLDER));
		expect(staticRefusals(root)).toEqual(['fixtures/ is a link, not a real folder']);
	});

	it('refuses static/ as a link', () => {
		const link = path.join(scratch, `static-link-${String(plantCount)}`);
		symlinkSync(plantedStatic(), link);
		expect(staticRefusals(link)).toEqual(['static/ is a link, not a real folder']);
	});

	it.each([
		['onload=', svg('<rect width="8" height="8" onload="alert(1)" />'), 'onload'],
		['ONCLICK=', svg('<rect width="8" height="8" ONCLICK="x" />'), 'ONCLICK'],
		['an external href', svg('<rect width="8" height="8" href="https://x.example/a" />'), 'href'],
		[
			'an external xlink:href',
			svg('<rect width="8" height="8" xlink:href="//x.example/a" />'),
			'xlink:href'
		],
		['<a>', svg('<a href="#x"><rect width="8" height="8" /></a>'), '<a>'],
		[
			'<set>',
			svg('<rect width="8" height="8"><set attributeName="fill" to="red" /></rect>'),
			'<set>'
		],
		[
			'<animate>',
			svg('<rect width="8" height="8"><animate attributeName="x" to="1" /></rect>'),
			'<animate>'
		],
		[
			'<animateTransform>',
			svg('<rect width="8" height="8"><animateTransform attributeName="transform" /></rect>'),
			'<animateTransform>'
		],
		['<style>', svg('<style>rect { fill: red; }</style>'), '<style>'],
		['<script>', svg('<script>alert(1)</script>'), '<script>'],
		[
			'<foreignObject>',
			svg('<foreignObject width="8" height="8"><p>x</p></foreignObject>'),
			'<foreignObject>'
		],
		['<use>', svg('<use href="#x" />'), '<use>'],
		['<image>', svg('<image width="8" height="8" />'), '<image>'],
		['<feImage>', svg('<filter id="f"><feImage /></filter>'), 'feImage'],
		['<mask>', svg('<mask id="m"><rect width="8" height="8" /></mask>'), '<mask>'],
		['a DOCTYPE', `<!DOCTYPE svg>\n${svg('<rect width="8" height="8" />')}`, 'DOCTYPE'],
		[
			'an ENTITY',
			`<!DOCTYPE svg [<!ENTITY x "y">]>\n${svg('<rect width="8" height="8" />')}`,
			'ENTITY'
		],
		[
			'an entity reference',
			svg('<rect width="8" height="8" fill="&#114;ed" />'),
			'an entity or character reference'
		],
		['a comment', svg('<!-- x --><rect width="8" height="8" />'), 'a comment'],
		['a remote url(', svg('<rect width="8" height="8" fill="url(https://x/a)" />'), 'url('],
		['a style attribute', svg('<rect width="8" height="8" style="fill: red" />'), 'style'],
		['a class attribute', svg('<rect width="8" height="8" class="x" />'), 'class'],
		['text outside a title', svg('<g>words</g>'), 'text outside'],
		['a single-quoted attribute', svg("<rect width='8' />"), 'does not read'],
		['an unclosed element', '<svg xmlns="http://www.w3.org/2000/svg"><g>', 'unclosed'],
		['two roots', `${svg('')}${svg('')}`, '2 root elements'],
		['another root', '<g></g>\n', 'not <svg>'],
		['another namespace', '<svg xmlns="http://www.w3.org/1999/xhtml"></svg>', 'xmlns']
	])('refuses %s in a fixture SVG', (_label, text, reason) => {
		const root = plantedStatic({ 'fixtures/avatar.svg': text });
		expect(staticRefusals(root)).toContainEqual(expect.stringContaining(reason));
	});

	it.each([
		[
			'a gradient by fragment',
			svg(
				'<defs><linearGradient id="g"><stop offset="0" stop-color="#fff" /></linearGradient></defs><rect width="8" height="8" fill="url(#g)" />'
			)
		],
		[
			'a clip path by fragment',
			svg('<clipPath id="c"><circle r="4" /></clipPath><g clip-path="url(#c)" />')
		],
		['a title', svg('<title>Avatar</title><circle cx="4" cy="4" r="4" />')]
	])('allows %s', (_label, text) => {
		expect(staticRefusals(plantedStatic({ 'fixtures/avatar.svg': text }))).toEqual([]);
	});
});
