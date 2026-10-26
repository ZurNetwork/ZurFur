/*
 * The page shell's boot script: one inline script in the head, between the
 * two metas and SvelteKit's head, pinned by hash, reading the one storage key
 * and setting the one attribute, to a skin and nothing else, without ever
 * letting an exception out.
 */
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import vm from 'node:vm';
import { describe, expect, it } from 'vitest';
import ts from 'typescript';
import { parse, type AST } from 'svelte/compiler';
import { BOOT_SCRIPT_HASH } from '../gallery/boot-script.ts';
import { SKIN_STORAGE_KEY } from '../gallery/skin-store.ts';
import { SKIN_ATTRIBUTE } from '../lib/styles/environment.ts';

const shell = readFileSync(new URL('../app.html', import.meta.url), 'utf8');

/** The shell's braces read as plain text, one character for one, as the network scan reads them. */
const braceFree = (text: string): string => text.replaceAll('{', '﷐').replaceAll('}', '﷑');
const restoreBraces = (text: string): string => text.replaceAll('﷐', '{').replaceAll('﷑', '}');

function isElement(node: unknown): node is AST.RegularElement {
	return Object(node) === node && Reflect.get(Object(node), 'type') === 'RegularElement';
}

function childElements(fragment: AST.Fragment): readonly AST.RegularElement[] {
	return fragment.nodes.filter(isElement);
}

function attributeText(element: AST.RegularElement, name: string): string | undefined {
	const attribute = element.attributes.find(
		(entry): entry is AST.Attribute => entry.type === 'Attribute' && entry.name === name
	);
	if (attribute === undefined || attribute.value === true) return undefined;
	const parts = Array.isArray(attribute.value) ? attribute.value : [attribute.value];
	return parts.map((part) => (part.type === 'Text' ? part.data : '')).join('');
}

interface ShellReading {
	readonly head: readonly AST.RegularElement[];
	readonly script: AST.RegularElement;
	readonly scriptText: string;
}

function readShell(text: string): ShellReading {
	const root = parse(braceFree(text), { modern: true });
	const html = childElements(root.fragment).find((element) => element.name === 'html');
	const head = html && childElements(html.fragment).find((element) => element.name === 'head');
	if (head === undefined) throw new Error('the shell has no <head>');
	const headElements = childElements(head.fragment);
	const scripts = headElements.filter((element) => element.name === 'script');
	const [script] = scripts;
	if (script === undefined || scripts.length !== 1) throw new Error('the head has no one script');
	const scriptText = restoreBraces(
		script.fragment.nodes.map((node) => (node.type === 'Text' ? node.data : '')).join('')
	);
	return { head: headElements, script, scriptText };
}

/** Base64 of SHA-256 over the text's UTF-8 bytes, as a CSP hash source. */
function cspHash(text: string): string {
	return `sha256-${createHash('sha256').update(text, 'utf8').digest('base64')}`;
}

const reading = readShell(shell);

describe('the page shell', () => {
	it('holds exactly one <script>, in any letter case, anywhere', () => {
		expect(shell.match(/<script\b/gi)).toHaveLength(1);
	});

	it('holds no <meta http-equiv>', () => {
		expect(shell).not.toMatch(/http-equiv/i);
	});

	it('runs the script inline, with no attribute at all', () => {
		expect(reading.script.attributes).toEqual([]);
		expect(reading.scriptText.trim()).not.toBe('');
	});

	it('places the script after the charset and viewport metas and before the head placeholder', () => {
		const charset = reading.head.find(
			(element) => element.name === 'meta' && attributeText(element, 'charset') !== undefined
		);
		const viewport = reading.head.find(
			(element) => element.name === 'meta' && attributeText(element, 'name') === 'viewport'
		);
		const placeholder = shell.indexOf('%sveltekit.head%');
		expect(charset?.end).toBeLessThan(reading.script.start);
		expect(viewport?.end).toBeLessThan(reading.script.start);
		expect(placeholder).toBeGreaterThan(reading.script.end);
	});

	it('serializes the charset within the first 1024 bytes', () => {
		const charsetEnd =
			shell.indexOf('<meta charset="utf-8" />') + '<meta charset="utf-8" />'.length;
		expect(Buffer.byteLength(shell.slice(0, charsetEnd), 'utf8')).toBeLessThanOrEqual(1024);
	});
});

describe('the boot script', () => {
	it('is pinned by its hash', () => {
		expect(cspHash(reading.scriptText)).toBe(BOOT_SCRIPT_HASH);
	});

	const program = ts.createSourceFile('boot.js', reading.scriptText, ts.ScriptTarget.Latest, true);

	function calls(name: string): readonly ts.CallExpression[] {
		const found: ts.CallExpression[] = [];
		const visit = (node: ts.Node): void => {
			if (
				ts.isCallExpression(node) &&
				ts.isPropertyAccessExpression(node.expression) &&
				node.expression.name.text === name
			) {
				found.push(node);
			}
			ts.forEachChild(node, visit);
		};
		visit(program);
		return found;
	}

	const literalArguments = (call: ts.CallExpression): readonly string[] =>
		call.arguments.map((argument) =>
			ts.isStringLiteral(argument) ? argument.text : '<not a literal>'
		);

	it('reads the store key', () => {
		expect(calls('getItem').map(literalArguments)).toEqual([[SKIN_STORAGE_KEY]]);
	});

	it('sets the skin attribute to a literal skin, once per skin', () => {
		expect(calls('setAttribute').map(literalArguments)).toEqual([
			[SKIN_ATTRIBUTE, 'dark'],
			[SKIN_ATTRIBUTE, 'light']
		]);
	});
});

interface Run {
	readonly set: readonly (readonly [string, string])[];
	readonly escaped: unknown;
}

/** A store that answers only the skin key, as the real one would. */
function storeHolding(value: string | undefined): object {
	return {
		getItem: (key: string) => (key === SKIN_STORAGE_KEY ? (value ?? null) : null)
	};
}

/** Runs the boot script in a fresh context whose window's `localStorage` is read through `readStore`. */
function runBoot(readStore: () => unknown): Run {
	const set: (readonly [string, string])[] = [];
	const documentElement = {
		setAttribute: (name: string, value: string) => {
			set.push([name, value]);
		}
	};
	const window = {
		get localStorage() {
			return readStore();
		}
	};
	const context = vm.createContext({ window, document: { documentElement } });
	let escaped: unknown = undefined;
	try {
		vm.runInContext(reading.scriptText, context, { timeout: 1000 });
	} catch (error) {
		escaped = error;
	}
	return { set, escaped };
}

describe('the boot script, run', () => {
	it.each(['dark', 'light'])('sets the attribute for a stored %j', (stored) => {
		expect(runBoot(() => storeHolding(stored))).toEqual({
			set: [[SKIN_ATTRIBUTE, stored]],
			escaped: undefined
		});
	});

	it.each([
		['a missing value', undefined],
		['LIGHT', 'LIGHT'],
		['a trailing space', 'light '],
		['markup', 'light"><script>'],
		['constructor', 'constructor'],
		['__proto__', '__proto__'],
		['toString', 'toString'],
		['an empty string', '']
	])('sets nothing for %s', (_label, stored) => {
		expect(runBoot(() => storeHolding(stored))).toEqual({ set: [], escaped: undefined });
	});

	it.each([
		[
			'reading localStorage throws',
			() => {
				throw new Error('SecurityError');
			}
		],
		[
			'getItem throws',
			() => ({
				getItem: () => {
					throw new Error('SecurityError');
				}
			})
		],
		['the store is null', () => null],
		['getItem is missing', () => ({})]
	])('sets nothing and lets nothing escape when %s', (_label, readStore) => {
		expect(runBoot(readStore)).toEqual({ set: [], escaped: undefined });
	});
});
