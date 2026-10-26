/**
 * The one parse the style and network scans share. A `.svelte` or `.html`
 * file goes through Svelte's modern AST, a `.css` file through Svelte's CSS
 * parser, and each yields what the scans read: CSS declarations, `@import`
 * preludes, URL-bearing attributes and `<svelte:head>` blocks. Input never
 * makes it throw: a file, style attribute or style element that doesn't
 * parse comes back as an `Unparsed` record, which both scans refuse.
 */
import { readdirSync, readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { parse, parseCss } from 'svelte/compiler';
import type { AST } from 'svelte/compiler';

/** A source file, by its path relative to the package root (`src/...`, `/`-separated). */
export interface SourceFile {
	readonly path: string;
	readonly text: string;
}

/** A 1-based line and column in a source file. */
export interface Place {
	readonly path: string;
	readonly line: number;
	readonly column: number;
}

/** Where a declaration was written. */
export type DeclarationOrigin =
	'stylesheet' | 'style-element' | 'style-attribute' | 'style-directive';

export interface CssDeclaration {
	readonly property: string;
	readonly value: string;
	readonly origin: DeclarationOrigin;
	readonly place: Place;
}

export interface CssImport {
	readonly prelude: string;
	readonly place: Place;
}

/** The attributes whose value the browser fetches, or sends a request to. */
export const urlAttributeNames = [
	'src',
	'href',
	'srcset',
	'imagesrcset',
	'action',
	'formaction',
	'poster',
	'data',
	'xlink:href',
	'ping',
	'background'
] as const;

export type UrlAttributeName = (typeof urlAttributeNames)[number];

/**
 * A URL-bearing attribute. `values` holds its static text (each expression in
 * it read as `0`) and every string literal or template literal in its
 * expressions.
 */
export interface UrlAttribute {
	readonly element: string;
	readonly attribute: UrlAttributeName;
	readonly values: readonly string[];
	readonly place: Place;
}

/** One node directly inside a `<svelte:head>`. */
export interface HeadChild {
	readonly type: string;
	readonly name: string;
	readonly attributes: readonly string[];
	readonly blank: boolean;
	readonly place: Place;
}

export interface Head {
	readonly place: Place;
	readonly children: readonly HeadChild[];
}

export interface Unparsed {
	readonly reason: string;
	readonly place: Place;
}

export interface ParsedSource {
	readonly path: string;
	readonly declarations: readonly CssDeclaration[];
	readonly imports: readonly CssImport[];
	readonly urlAttributes: readonly UrlAttribute[];
	readonly heads: readonly Head[];
	readonly unparsed: readonly Unparsed[];
}

/** A CSS value split three ways: code with strings, comments and `url()` bodies blanked, and those strings and URLs. */
export interface CssValueParts {
	readonly code: string;
	readonly strings: readonly string[];
	readonly urls: readonly string[];
}

type SourceKind = 'svelte' | 'html' | 'css';

/** What an expression in static text is read as: a digit, so `{d}ms` still reads as a time. */
const expressionPlaceholder = '0';

const packageRoot = fileURLToPath(new URL('../../../', import.meta.url));

/** The folder the scans never read: the checks hold their fixtures in memory. */
const checksFolder = 'src/checks/';

function sourceKind(path: string): SourceKind | undefined {
	if (path.endsWith('.svelte')) return 'svelte';
	if (path.endsWith('.html')) return 'html';
	if (path.endsWith('.css')) return 'css';
	return undefined;
}

/** Every `.svelte`, `.html` and `.css` file under the package's `src/`, `src/checks/` aside, sorted by path. */
export function packageSources(): readonly SourceFile[] {
	return listFiles('src')
		.filter((path) => !path.startsWith(checksFolder) && sourceKind(path) !== undefined)
		.sort()
		.map((path) => ({ path, text: readFileSync(`${packageRoot}${path}`, 'utf8') }));
}

function listFiles(folder: string): readonly string[] {
	return readdirSync(`${packageRoot}${folder}`, { withFileTypes: true }).flatMap((entry) => {
		const path = `${folder}/${entry.name}`;
		if (entry.isDirectory()) return listFiles(path);
		return entry.isFile() ? [path] : [];
	});
}

interface Collected {
	readonly declarations: CssDeclaration[];
	readonly imports: CssImport[];
	readonly urlAttributes: UrlAttribute[];
	readonly heads: Head[];
	readonly unparsed: Unparsed[];
}

/** Parses one source file into what the scans read. */
export function parseSource(source: SourceFile): ParsedSource {
	const collected: Collected = {
		declarations: [],
		imports: [],
		urlAttributes: [],
		heads: [],
		unparsed: []
	};
	const placeAt = placer(source);
	const kind = sourceKind(source.path);
	if (kind === undefined) {
		collected.unparsed.push({ reason: 'not a .svelte, .html or .css file', place: placeAt(0) });
	} else {
		switch (kind) {
			case 'svelte':
			case 'html':
				collectMarkup(source.text, placeAt, collected);
				break;
			case 'css':
				collectCss(source.text, placeAt, 'stylesheet', collected);
				break;
		}
	}
	return { path: source.path, ...collected };
}

type PlaceAt = (offset: number) => Place;

function placer(source: SourceFile): PlaceAt {
	return (offset) => {
		const before = source.text.slice(0, Math.max(0, offset)).split('\n');
		const last = before[before.length - 1] ?? '';
		return { path: source.path, line: before.length, column: last.length + 1 };
	};
}

function failureReason(error: unknown): string {
	return error instanceof Error ? error.message : 'the parser gave up';
}

/** The parser's own offset for a failure, when it gives one. */
function failureOffset(error: unknown): number {
	const position: unknown = error instanceof Error ? Reflect.get(error, 'position') : undefined;
	if (!Array.isArray(position)) return 0;
	const start: unknown = position[0];
	return typeof start === 'number' ? start : 0;
}

function collectMarkup(text: string, placeAt: PlaceAt, collected: Collected): void {
	const root = parseMarkup(text);
	if (!('fragment' in root)) {
		collected.unparsed.push({ reason: root.reason, place: placeAt(root.offset) });
		return;
	}
	if (root.css) walkCss(root.css.children, placeAt, 'stylesheet', collected);
	walkTemplate(root.fragment, placeAt, collected, new Set());
}

interface ParseFailure {
	readonly reason: string;
	readonly offset: number;
}

function parseMarkup(text: string): AST.Root | ParseFailure {
	try {
		return parse(text, { modern: true });
	} catch (error) {
		return { reason: failureReason(error), offset: failureOffset(error) };
	}
}

function parseStyleSheet(text: string): AST.CSS.StyleSheetFile | ParseFailure {
	try {
		return parseCss(text);
	} catch (error) {
		return { reason: failureReason(error), offset: failureOffset(error) };
	}
}

/**
 * Parses `text` as a stylesheet and collects its declarations and imports.
 * `placeAt` maps the stylesheet's own offsets to places in the file.
 */
function collectCss(
	text: string,
	placeAt: PlaceAt,
	origin: DeclarationOrigin,
	collected: Collected
): void {
	const sheet = parseStyleSheet(text);
	if ('reason' in sheet) {
		collected.unparsed.push({ reason: sheet.reason, place: placeAt(sheet.offset) });
		return;
	}
	walkCss(sheet.children, placeAt, origin, collected);
}

type CssNode = AST.CSS.Atrule | AST.CSS.Rule | AST.CSS.Declaration;

function walkCss(
	nodes: readonly CssNode[],
	placeAt: PlaceAt,
	origin: DeclarationOrigin,
	collected: Collected
): void {
	for (const node of nodes) {
		switch (node.type) {
			case 'Declaration':
				collected.declarations.push({
					property: node.property,
					value: node.value,
					origin,
					place: placeAt(node.start)
				});
				break;
			case 'Rule':
				walkCss(node.block.children, placeAt, origin, collected);
				break;
			case 'Atrule':
				if (node.name.toLowerCase() === 'import') {
					collected.imports.push({ prelude: node.prelude, place: placeAt(node.start) });
				}
				if (node.block) walkCss(node.block.children, placeAt, origin, collected);
				break;
		}
	}
}

/** Any object, read field by field. */
type Fields = Readonly<Record<string, unknown>>;

function isFields(value: unknown): value is Fields {
	return Object(value) === value;
}

/** A template node that carries attributes: an element, a component or a `svelte:` element. */
interface ElementLike {
	readonly type: string;
	readonly name: string;
	readonly start: number;
	readonly attributes: readonly unknown[];
	readonly fragment?: unknown;
}

function isElementLike(value: unknown): value is ElementLike {
	return (
		isFields(value) &&
		typeof value.type === 'string' &&
		typeof value.name === 'string' &&
		typeof value.start === 'number' &&
		Array.isArray(value.attributes)
	);
}

function isAttribute(value: unknown): value is AST.Attribute {
	return isFields(value) && value.type === 'Attribute';
}

function isStyleDirective(value: unknown): value is AST.StyleDirective {
	return isFields(value) && value.type === 'StyleDirective';
}

function isSvelteHead(value: unknown): value is AST.SvelteHead {
	return isFields(value) && value.type === 'SvelteHead';
}

function isText(value: unknown): value is AST.Text {
	return isFields(value) && value.type === 'Text';
}

function isUrlAttributeName(name: string): name is UrlAttributeName {
	return urlAttributeNames.some((known) => known === name);
}

/** Visits every object in the template once, acting on elements and heads. */
function walkTemplate(
	value: unknown,
	placeAt: PlaceAt,
	collected: Collected,
	seen: Set<object>
): void {
	if (!isFields(value) || seen.has(value)) return;
	seen.add(value);
	if (isElementLike(value)) collectElement(value, placeAt, collected);
	if (isSvelteHead(value)) collected.heads.push(headOf(value, placeAt));
	for (const child of Object.values(value)) {
		if (Array.isArray(child)) {
			for (const item of child) walkTemplate(item, placeAt, collected, seen);
		} else {
			walkTemplate(child, placeAt, collected, seen);
		}
	}
}

function collectElement(element: ElementLike, placeAt: PlaceAt, collected: Collected): void {
	for (const attribute of element.attributes) {
		if (isAttribute(attribute)) collectAttribute(element, attribute, placeAt, collected);
		if (isStyleDirective(attribute)) collectStyleDirective(attribute, placeAt, collected);
	}
	if (element.type === 'RegularElement' && element.name.toLowerCase() === 'style') {
		collectStyleElement(element, placeAt, collected);
	}
}

function collectAttribute(
	element: ElementLike,
	attribute: AST.Attribute,
	placeAt: PlaceAt,
	collected: Collected
): void {
	const name = attribute.name.toLowerCase();
	const place = placeAt(attribute.start);
	const texts = valueTexts(attribute.value);
	if (name === 'style') {
		for (const text of texts) {
			collectCss(`x{${text}}`, () => place, 'style-attribute', collected);
		}
	}
	if (isUrlAttributeName(name)) {
		collected.urlAttributes.push({ element: element.name, attribute: name, values: texts, place });
	}
}

function collectStyleDirective(
	directive: AST.StyleDirective,
	placeAt: PlaceAt,
	collected: Collected
): void {
	const place = placeAt(directive.start);
	for (const text of valueTexts(directive.value)) {
		collectCss(`x{${directive.name}: ${text}}`, () => place, 'style-directive', collected);
	}
}

/** A `<style>` element inside the markup, which Svelte leaves as text. */
function collectStyleElement(element: ElementLike, placeAt: PlaceAt, collected: Collected): void {
	const place = placeAt(element.start);
	const nodes: unknown = isFields(element.fragment) ? element.fragment.nodes : undefined;
	const parts = Array.isArray(nodes) ? nodes : [];
	const text = parts.map((node) => (isText(node) ? node.data : expressionPlaceholder)).join('');
	collectCss(text, () => place, 'style-element', collected);
}

/**
 * The texts an attribute value can produce statically: its text with each
 * expression read as the placeholder, and every string or template literal
 * inside its expressions.
 */
function valueTexts(value: AST.Attribute['value']): readonly string[] {
	if (value === true) return [];
	const parts = Array.isArray(value) ? value : [value];
	const joined = parts
		.map((part) => (part.type === 'Text' ? part.data : expressionPlaceholder))
		.join('');
	const literals = parts.flatMap((part) =>
		part.type === 'ExpressionTag' ? expressionStrings(part.expression) : []
	);
	const wholeExpression = parts.length === 1 && parts[0]?.type === 'ExpressionTag';
	return wholeExpression ? literals : [joined, ...literals];
}

/** Every string literal in an expression, and every template literal with its holes read as the placeholder. */
function expressionStrings(value: unknown): readonly string[] {
	if (Array.isArray(value)) return value.flatMap(expressionStrings);
	if (!isFields(value)) return [];
	if (value.type === 'Literal') {
		const literal = value.value;
		return typeof literal === 'string' ? [literal] : [];
	}
	if (value.type === 'TemplateLiteral') {
		return [templateText(value.quasis), ...expressionStrings(value.expressions)];
	}
	return Object.entries(value)
		.filter(([key]) => key !== 'loc' && key !== 'metadata')
		.flatMap(([, child]) => expressionStrings(child));
}

function templateText(quasis: unknown): string {
	const parts = Array.isArray(quasis) ? quasis : [];
	return parts.map(quasiText).join(expressionPlaceholder);
}

function quasiText(quasi: unknown): string {
	const value: unknown = isFields(quasi) ? quasi.value : undefined;
	if (!isFields(value)) return '';
	const cooked = value.cooked;
	const raw = value.raw;
	if (typeof cooked === 'string') return cooked;
	return typeof raw === 'string' ? raw : '';
}

function headOf(head: AST.SvelteHead, placeAt: PlaceAt): Head {
	const children = head.fragment.nodes.map((node): HeadChild => {
		const element = isElementLike(node) ? node : undefined;
		const attributes = (element?.attributes ?? []).flatMap((attribute) =>
			isAttribute(attribute) ? [attribute.name.toLowerCase()] : ['{…}']
		);
		return {
			type: node.type,
			name: element?.name.toLowerCase() ?? '',
			attributes,
			blank: isText(node) && node.data.trim() === '',
			place: placeAt(node.start)
		};
	});
	return { place: placeAt(head.start), children };
}

/** Matches the CSS hex escape `\` + 1–6 hex digits + one optional whitespace. */
const hexEscape = /^[0-9a-f]{1,6}[ \t\n]?/i;

/** Reads one escape after a `\` at `index`; returns the character and the index past it. */
function readEscape(text: string, index: number): readonly [string, number] {
	const hex = hexEscape.exec(text.slice(index + 1));
	if (hex) {
		const codePoint = Number.parseInt(hex[0].trim(), 16);
		const valid = codePoint > 0 && codePoint <= 0x10ffff;
		return [valid ? String.fromCodePoint(codePoint) : '�', index + 1 + hex[0].length];
	}
	return [text.charAt(index + 1), index + 2];
}

/** Reads a quoted string starting at the quote at `index`; returns its content and the index past it. */
function readString(text: string, index: number): readonly [string, number] {
	const quote = text.charAt(index);
	let content = '';
	let at = index + 1;
	while (at < text.length) {
		const character = text.charAt(at);
		if (character === quote || character === '\n') return [content, at + 1];
		if (character === '\\') {
			const [escaped, next] = readEscape(text, at);
			content += escaped;
			at = next;
		} else {
			content += character;
			at += 1;
		}
	}
	return [content, at];
}

/** Reads a `url(` body starting just past the `(`; returns the URL and the index past the `)`. */
function readUrl(text: string, index: number): readonly [string, number] {
	let at = index;
	while (/\s/.test(text.charAt(at))) at += 1;
	const first = text.charAt(at);
	if (first === '"' || first === "'") {
		const [content, next] = readString(text, at);
		const close = text.indexOf(')', next);
		return [content, close === -1 ? text.length : close + 1];
	}
	let content = '';
	while (at < text.length && text.charAt(at) !== ')') {
		if (text.charAt(at) === '\\') {
			const [escaped, next] = readEscape(text, at);
			content += escaped;
			at = next;
		} else {
			content += text.charAt(at);
			at += 1;
		}
	}
	return [content.trim(), at + 1];
}

/** True when a `url(` starts at `index`, as a whole function name. */
function urlStartsAt(text: string, index: number): boolean {
	if (text.slice(index, index + 4).toLowerCase() !== 'url(') return false;
	return index === 0 || !/[\w-]/.test(text.charAt(index - 1));
}

/** Splits a CSS value into its code (strings, comments and `url()` bodies blanked), strings and URLs. */
export function cssValueParts(value: string): CssValueParts {
	let code = '';
	const strings: string[] = [];
	const urls: string[] = [];
	let at = 0;
	while (at < value.length) {
		const character = value.charAt(at);
		if (value.startsWith('/*', at)) {
			const close = value.indexOf('*/', at + 2);
			const next = close === -1 ? value.length : close + 2;
			code += ' '.repeat(next - at);
			at = next;
		} else if (character === '"' || character === "'") {
			const [content, next] = readString(value, at);
			strings.push(content);
			code += ' '.repeat(next - at);
			at = next;
		} else if (urlStartsAt(value, at)) {
			const [url, next] = readUrl(value, at + 4);
			urls.push(url);
			code += `url(${' '.repeat(Math.max(0, next - at - 5))})`;
			at = next;
		} else {
			code += character;
			at += 1;
		}
	}
	return { code, strings, urls };
}
