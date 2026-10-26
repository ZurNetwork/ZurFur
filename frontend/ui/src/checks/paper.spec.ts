import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { parseCss } from 'svelte/compiler';
import type { AST } from 'svelte/compiler';
import { PAPER_MOVES } from '../lib/motion/paper-moves';
import { PAPER_CLASSES } from '../lib/styles/paper-classes';

const folder = new URL('../lib/', import.meta.url);

function read(relative: string): string {
	return readFileSync(new URL(relative, folder), 'utf8');
}

const paperCss = read('styles/paper.css');
const movesCss = read('motion/paper-moves.css');
const baseCss = read('styles/base.css');
const tokensCss = read('styles/tokens.css');

/** The custom properties a paper rule reads that the caller sets: not tokens. */
const localHooks: readonly string[] = ['--k', '--ring', '--pile-top', '--life'];

interface Declaration {
	readonly property: string;
	readonly value: string;
}

interface Scan {
	readonly classes: Set<string>;
	readonly keyframes: Set<string>;
	readonly declarations: Declaration[];
}

type CssNode = AST.CSS.Atrule | AST.CSS.Rule | AST.CSS.Declaration;

/** Every class name inside a selector, the ones in `:not()` and `:where()` included. */
function collectClasses(node: unknown, into: Set<string>): void {
	if (Array.isArray(node)) {
		for (const item of node) collectClasses(item, into);
		return;
	}
	if (Object(node) !== node) return;
	const fields = Object(node) as Readonly<Record<string, unknown>>;
	if (fields.type === 'ClassSelector' && typeof fields.name === 'string') into.add(fields.name);
	for (const [key, child] of Object.entries(fields)) {
		if (key !== 'metadata') collectClasses(child, into);
	}
}

function visit(nodes: readonly CssNode[], scan: Scan, inKeyframes: boolean): void {
	for (const node of nodes) {
		switch (node.type) {
			case 'Declaration':
				scan.declarations.push({ property: node.property, value: node.value });
				break;
			case 'Rule':
				if (!inKeyframes) collectClasses(node.prelude, scan.classes);
				visit(node.block.children, scan, inKeyframes);
				break;
			case 'Atrule': {
				const isKeyframes = node.name.toLowerCase().endsWith('keyframes');
				if (isKeyframes) scan.keyframes.add(node.prelude.trim());
				if (node.block) visit(node.block.children, scan, inKeyframes || isKeyframes);
				break;
			}
		}
	}
}

function scanCss(css: string): Scan {
	const scan: Scan = { classes: new Set(), keyframes: new Set(), declarations: [] };
	visit(parseCss(css).children, scan, false);
	return scan;
}

const animationKeywords = new Set([
	'none',
	'infinite',
	'forwards',
	'backwards',
	'both',
	'reverse',
	'normal',
	'alternate',
	'alternate-reverse',
	'running',
	'paused',
	'ease',
	'ease-in',
	'ease-out',
	'ease-in-out',
	'linear',
	'step-start',
	'step-end'
]);

/** A value with every `var(...)` taken out, innermost first, so commas inside a fallback never split it. */
function withoutVars(value: string): string {
	let current = value;
	for (;;) {
		const next = current.replace(/var\([^()]*\)/g, ' ');
		if (next === current) return current;
		current = next;
	}
}

/** The keyframes names one `animation` or `animation-name` declaration asks for, or why it can't be read. */
function namesIn(declaration: Declaration): { names: string[]; problems: string[] } {
	const names: string[] = [];
	const problems: string[] = [];
	const segments = withoutVars(declaration.value).split(',');
	for (const segment of segments) {
		const words = segment.split(/\s+/).filter((word) => word !== '');
		if (words.includes('none')) continue;
		const isShorthand = declaration.property === 'animation';
		const candidates = isShorthand
			? words.filter((word) => /^[a-z_][\w-]*$/i.test(word) && !animationKeywords.has(word))
			: words;
		if (candidates.length === 1 && candidates[0] !== undefined) {
			names.push(candidates[0]);
		} else {
			problems.push(`${declaration.property}: cannot read a name in "${segment.trim()}"`);
		}
	}
	return { names, problems };
}

/** Animations that name no defined keyframes, and keyframes nothing uses. */
function keyframeProblems(scan: Scan): string[] {
	const used = new Set<string>();
	const problems: string[] = [];
	const animations = scan.declarations.filter(
		(declaration) =>
			declaration.property === 'animation' || declaration.property === 'animation-name'
	);
	for (const declaration of animations) {
		const read = namesIn(declaration);
		problems.push(...read.problems);
		for (const name of read.names) {
			used.add(name);
			if (!scan.keyframes.has(name)) problems.push(`${declaration.property} names ${name}`);
		}
	}
	for (const name of scan.keyframes) {
		if (!used.has(name)) problems.push(`@keyframes ${name} is unused`);
	}
	return problems;
}

/** The names of the custom properties `tokens.css` defines. */
function definedTokens(css: string): Set<string> {
	return new Set([...css.matchAll(/(--[a-z0-9-]+)\s*:/g)].map((match) => match[1] ?? ''));
}

/** Every `var(--x)` that is neither a token nor a local hook. */
function unknownVars(scan: Scan, tokens: ReadonlySet<string>): string[] {
	const known = new Set([...tokens, ...localHooks]);
	return scan.declarations
		.flatMap((declaration) =>
			[...declaration.value.matchAll(/var\(\s*(--[\w-]+)/g)].map((match) => match[1] ?? '')
		)
		.filter((name) => !known.has(name));
}

function listed(list: readonly string[]): string[] {
	return [...list].sort();
}

const tokenNames = definedTokens(tokensCss);

describe('the class lists', () => {
	it('lists exactly the classes paper.css defines', () => {
		expect(listed(PAPER_CLASSES)).toEqual([...scanCss(paperCss).classes].sort());
	});

	it('lists exactly the classes paper-moves.css defines', () => {
		expect(listed(PAPER_MOVES)).toEqual([...scanCss(movesCss).classes].sort());
	});

	it('lists a class once, and no class in both lists', () => {
		const all = [...PAPER_CLASSES, ...PAPER_MOVES];
		expect(new Set(all).size).toBe(all.length);
	});

	it('finds the classes in a selector, inside :not() too, and none in a keyframe', () => {
		const scan = scanCss('.a:hover:not(.b) { color: red; } @keyframes k { 0% { top: 0; } }');
		expect([...scan.classes].sort()).toEqual(['a', 'b']);
	});

	it('turns away a list that misses a class the file defines', () => {
		const defined = [...scanCss('.one { top: 0; } .two { top: 0; }').classes].sort();
		expect(listed(['one'])).not.toEqual(defined);
	});
});

describe('the keyframes', () => {
	it('are all defined and all used, in the moves and in the look', () => {
		expect(keyframeProblems(scanCss(movesCss))).toEqual([]);
		expect(keyframeProblems(scanCss(paperCss))).toEqual([]);
	});

	it.each([
		['an animation naming no keyframes', '.a { animation: gone 1s; }', ['animation names gone']],
		[
			'an animation-name naming no keyframes',
			'.a { animation-name: gone; }',
			['animation-name names gone']
		],
		[
			'a keyframes nothing uses',
			'@keyframes idle { to { top: 0; } }',
			['@keyframes idle is unused']
		],
		[
			'a name behind a var() fallback with a comma',
			'@keyframes k { to { top: 0; } } .a { animation: k var(--x, var(--y)) forwards, gone var(--z); }',
			['animation names gone']
		]
	])('turn away %s', (_label, css, expected) => {
		expect(keyframeProblems(scanCss(css))).toEqual(expected);
	});
});

describe('the custom properties', () => {
	it('read tokens or the declared hooks only', () => {
		expect(tokenNames.size).toBeGreaterThan(50);
		for (const css of [paperCss, movesCss, baseCss]) {
			expect(unknownVars(scanCss(css), tokenNames)).toEqual([]);
		}
	});

	it('turn away a name that is neither', () => {
		const scan = scanCss('.a { color: var(--color-nope); top: var(--k); }');
		expect(unknownVars(scan, tokenNames)).toEqual(['--color-nope']);
	});
});

describe('the motion mode', () => {
	it('never names the lively mode in a selector', () => {
		for (const css of [paperCss, movesCss]) {
			expect(css).not.toMatch(/\[data-motion[^\]]*lively/i);
		}
	});
});
