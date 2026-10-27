import { describe, expect, it } from 'vitest';
import ts from 'typescript';
import { parse, parseCss } from 'svelte/compiler';
import { existsSync, readdirSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
	EXCEPTIONS,
	FAMILIES,
	OWNED_ELEMENTS,
	PAGE_ELEMENT_FOLDERS,
	PAGE_ELEMENTS,
	TABLE,
	UNLISTED_BITS_NAMESPACES,
	UNOWNED_BITS_PARTS,
	type CompositionTable,
	type Exception,
	type Family,
	type Kind,
	type OwnedElement
} from './composition-table';
import { entryPath, exportedComponents, storyMeta, type ExportedComponent } from './scan/exports';
import type { SourceFile } from './scan/parse';

/**
 * The exceptions the table may hold: each row's file, owned elements and
 * families. The list only shrinks; a row, element or family outside it fails.
 */
const KNOWN_EXCEPTIONS: readonly Pick<Exception, 'file' | 'elements' | 'families'>[] = [
	{
		file: 'src/lib/controls/Menu.svelte',
		elements: ['button'],
		families: ['zf-btn', 'zf-iconbtn']
	},
	{ file: 'src/lib/controls/DateField.svelte', elements: [], families: ['zf-input'] },
	{ file: 'src/lib/styles/controls/menu.css', elements: [], families: ['zf-icon'] },
	{ file: 'src/lib/controls/Select.svelte', elements: [], families: ['zf-menu'] }
];

export type CompositionRefusalReason =
	| 'does not parse'
	| 'an exported component with no kind'
	| 'a kind for a name the entry does not export'
	| 'a library component neither exported nor internal'
	| 'a simple component that holds another component'
	| 'a simple component with no family of its own'
	| 'a composite that imports no library component'
	| 'a composite that renders an owned element'
	| 'an owned element in a file not listed for it'
	| 'a dynamic svelte:element in a file not listed for it'
	| 'a page element in the library or the stories'
	| "a zf- class outside the file's families"
	| 'a family stylesheet with no row'
	| 'a stories file whose tags do not carry exactly its kind'
	| 'a stories file whose tags cannot be read'
	| 'an exception outside the known list'
	| 'an exception the file no longer needs'
	| 'an owner row the file does not use'
	| 'a bits-ui name not listed for the file'
	| 'a bits-ui row that lists Avatar or Button'
	| 'a bits-ui row the file does not use'
	| 'the whole of bits-ui imported'
	| 'a default import of bits-ui'
	| 'a deep bits-ui path'
	| 'a computed import'
	| 'a bits-ui link or image part'
	| 'a bits-ui namespace used other than by its parts'
	| 'a family listed by two stylesheet rows'
	| 'a stylesheet row for a family a component styles itself'
	| 'a Spinner rendered other than as a glyph'
	| 'an import of the package entry from inside the library';

/** Why a file, the entry or the table is refused, and what it named. */
export interface CompositionRefusal {
	readonly path: string;
	readonly reason: CompositionRefusalReason;
	readonly detail: string;
}

/** A refusal before it is pinned to a file. */
type Finding = Pick<CompositionRefusal, 'reason' | 'detail'>;

export interface CompositionInput {
	/** The public entry's text. */
	readonly entry: string;
	/** The `.svelte` and `.ts` files the check reads, by path relative to the package. */
	readonly sources: readonly SourceFile[];
	/** The stylesheets under `src/lib/`: one in a styles folder needs its row, any other writes only the shared families. */
	readonly stylesheets: readonly SourceFile[];
	readonly table: CompositionTable;
	readonly knownExceptions: readonly Pick<Exception, 'file' | 'elements' | 'families'>[];
}

type Fields = Readonly<Record<string, unknown>>;

function isFields(value: unknown): value is Fields {
	return Object(value) === value;
}

/** How one import is written. */
type ImportForm =
	| 'named'
	| 'default'
	| 'namespace'
	| 'bare'
	| 'reexport'
	| 'reexport-all'
	| 'dynamic'
	| 'type-query';

/** One import or re-export: its specifier (undefined when computed), its form and the names it binds. */
interface ImportRecord {
	readonly source: string | undefined;
	readonly form: ImportForm;
	readonly typeOnly: boolean;
	/** Each name taken: the module's name for it and the local name. */
	readonly names: readonly { readonly imported: string; readonly local: string }[];
}

/** What the rules read off one source file. */
interface Facts {
	readonly path: string;
	readonly unparsed: string | undefined;
	/** Owned elements rendered, by their owned name (an HTML `<image>` reads as `img`). */
	readonly owned: ReadonlySet<OwnedElement>;
	readonly dynamicElements: number;
	/** `link`, `base`, `svelte:head` and nested `style` elements rendered. */
	readonly pageElements: ReadonlySet<string>;
	/** Every `zf-` class token the file writes. */
	readonly classes: ReadonlySet<string>;
	/** The classes a `.svelte` file's own `<style>` block names. */
	readonly styled: ReadonlySet<string>;
	readonly imports: readonly ImportRecord[];
	/** What the namespace walk found wrong with how bits-ui's names are used. */
	readonly bitsUses: readonly Finding[];
	/** Each `<Spinner>` tag not given the literal `kind="glyph"`. */
	readonly spinnerTags: readonly string[];
	/** Each use of a Spinner import as a value rather than as a tag. */
	readonly spinnerValues: readonly string[];
}

/** Spinner, the one glyph with a second form. */
const SPINNER_FILE = 'src/lib/surfaces/Spinner.svelte';

/** Where the package entry resolves from inside the library: the folder, `index`, `index.ts` and `index.js`. */
const ENTRY_TARGETS: readonly string[] = [
	'src/lib',
	'src/lib/index',
	'src/lib/index.ts',
	'src/lib/index.js'
];

/** Every `zf-` class token in a text: `zf-` and what follows, not inside a longer word. */
const CLASS_TOKEN = /(?<![\w-])zf-[\w-]*/g;

function classTokens(text: string): readonly string[] {
	return [...text.matchAll(CLASS_TOKEN)].map((match) => match[0]);
}

/** The text of a string literal, or of a template literal with no expressions. */
function staticTextOf(node: unknown): string | undefined {
	if (!isFields(node)) return undefined;
	if (node.type === 'Literal') return typeof node.value === 'string' ? node.value : undefined;
	if (node.type === 'TSLiteralType') return staticTextOf(node.literal);
	if (node.type !== 'TemplateLiteral') return undefined;
	const { expressions, quasis } = node;
	if (!Array.isArray(expressions) || expressions.length > 0 || !Array.isArray(quasis)) {
		return undefined;
	}
	return quasiText(quasis[0]);
}

function quasiText(quasi: unknown): string | undefined {
	const value = isFields(quasi) ? quasi.value : undefined;
	if (!isFields(value)) return undefined;
	if (typeof value.cooked === 'string') return value.cooked;
	return typeof value.raw === 'string' ? value.raw : undefined;
}

/** An identifier's name, or a string literal's text. */
function nameOf(node: unknown): string | undefined {
	if (!isFields(node)) return undefined;
	if (node.type === 'Identifier') return typeof node.name === 'string' ? node.name : undefined;
	return staticTextOf(node);
}

function isOwnedElement(name: string): name is OwnedElement {
	return OWNED_ELEMENTS.some((owned) => owned === name);
}

/**
 * An element's owned name: lowercased, read after its last `:` (so `svg:image`
 * is SVG's `image`), and an `<image>` outside `<svg>` read as `img`, as the
 * HTML parser does.
 */
function elementName(raw: string, inSvg: boolean): string {
	const local = raw.toLowerCase().split(':').at(-1) ?? '';
	return local === 'image' && !inSvg ? 'img' : local;
}

/** The classes in every selector of a stylesheet, `:is()`, `:not()` and `:where()` included. */
function selectorClasses(node: unknown, into: Set<string>): void {
	if (Array.isArray(node)) {
		for (const item of node) selectorClasses(item, into);
		return;
	}
	if (!isFields(node)) return;
	if (node.type === 'ClassSelector' && typeof node.name === 'string') into.add(node.name);
	for (const [key, child] of Object.entries(node)) {
		if (key !== 'metadata') selectorClasses(child, into);
	}
}

/** A stylesheet's classes, or why it does not parse. */
function stylesheetClasses(text: string): ReadonlySet<string> | string {
	try {
		const classes = new Set<string>();
		selectorClasses(parseCss(text).children, classes);
		return classes;
	} catch (error) {
		return error instanceof Error ? error.message : 'the CSS parser gave up';
	}
}

/** The text inside an element Svelte keeps as text, such as a nested `<style>`. */
function elementText(element: Fields): string {
	const fragment = element.fragment;
	const nodes: unknown = isFields(fragment) ? fragment.nodes : undefined;
	const parts: readonly unknown[] = Array.isArray(nodes) ? nodes : [];
	return parts
		.map((node) => (isFields(node) && typeof node.data === 'string' ? node.data : ''))
		.join('');
}

/** The static text parts of an attribute's value. */
function attributeTexts(value: unknown): readonly string[] {
	const parts: readonly unknown[] = Array.isArray(value) ? value : [value];
	return parts.flatMap((part) =>
		isFields(part) && part.type === 'Text' && typeof part.data === 'string' ? [part.data] : []
	);
}

/** The imports an ESTree program part declares, and the dynamic ones anywhere in it. */
function estreeImport(node: Fields): ImportRecord | undefined {
	const typeOnly = node.importKind === 'type' || node.exportKind === 'type';
	switch (node.type) {
		case 'ImportDeclaration': {
			const source = staticTextOf(node.source);
			const specifiers: readonly unknown[] = Array.isArray(node.specifiers) ? node.specifiers : [];
			if (specifiers.length === 0) return { source, form: 'bare', typeOnly, names: [] };
			const namespace = specifiers.some(
				(specifier) => isFields(specifier) && specifier.type === 'ImportNamespaceSpecifier'
			);
			if (namespace) return { source, form: 'namespace', typeOnly, names: [] };
			const named = specifiers.flatMap((specifier) => {
				if (!isFields(specifier) || specifier.type !== 'ImportSpecifier') return [];
				const imported = nameOf(specifier.imported);
				const local = nameOf(specifier.local);
				return imported === undefined || local === undefined ? [] : [{ imported, local }];
			});
			const hasDefault = specifiers.some(
				(specifier) => isFields(specifier) && specifier.type === 'ImportDefaultSpecifier'
			);
			const defaults = specifiers.flatMap((specifier) => {
				if (!isFields(specifier) || specifier.type !== 'ImportDefaultSpecifier') return [];
				const local = nameOf(specifier.local);
				return local === undefined ? [] : [{ imported: 'default', local }];
			});
			return {
				source,
				form: hasDefault ? 'default' : 'named',
				typeOnly,
				names: [...defaults, ...named]
			};
		}
		case 'ExportNamedDeclaration': {
			if (node.source === null || node.source === undefined) return undefined;
			const specifiers: readonly unknown[] = Array.isArray(node.specifiers) ? node.specifiers : [];
			const names = specifiers.flatMap((specifier) => {
				if (!isFields(specifier)) return [];
				const imported = nameOf(specifier.local);
				const local = nameOf(specifier.exported);
				return imported === undefined || local === undefined ? [] : [{ imported, local }];
			});
			return { source: staticTextOf(node.source), form: 'reexport', typeOnly, names };
		}
		case 'ExportAllDeclaration':
			return { source: staticTextOf(node.source), form: 'reexport-all', typeOnly, names: [] };
		case 'ImportExpression':
			return { source: staticTextOf(node.source), form: 'dynamic', typeOnly: false, names: [] };
		case 'TSImportType':
			return {
				source: staticTextOf(node.argument),
				form: 'type-query',
				typeOnly: true,
				names: []
			};
		default:
			return undefined;
	}
}

/** One visit of the template and script walk: a node, its parent and the key it sits under. */
interface Visit {
	readonly node: Fields;
	readonly parent: Fields | undefined;
	readonly key: string;
	readonly inSvg: boolean;
}

/** Every node of a parsed `.svelte` file once, with its parent, skipping the compiler's metadata. */
function visitsOf(root: Fields): readonly Visit[] {
	const seen = new Set<object>();
	const visits: Visit[] = [];
	const walk = (value: unknown, parent: Fields | undefined, key: string, inSvg: boolean): void => {
		if (Array.isArray(value)) {
			for (const item of value) walk(item, parent, key, inSvg);
			return;
		}
		if (!isFields(value) || seen.has(value)) return;
		seen.add(value);
		visits.push({ node: value, parent, key, inSvg });
		const name = typeof value.name === 'string' ? elementName(value.name, true) : '';
		const childInSvg = inSvg || (value.type === 'RegularElement' && name === 'svg');
		for (const [childKey, child] of Object.entries(value)) {
			if (childKey !== 'metadata' && childKey !== 'loc') walk(child, value, childKey, childInSvg);
		}
	};
	walk(root, undefined, '', false);
	return visits;
}

/** The part a member access off a bits-ui namespace reads, when its name is written out. */
function memberName(node: Fields): string | undefined {
	if (node.type === 'MemberExpression') {
		return node.computed === true ? staticTextOf(node.property) : nameOf(node.property);
	}
	if (node.type === 'TSQualifiedName') return nameOf(node.right);
	return undefined;
}

function isUnownedPart(namespace: string, part: string): boolean {
	return UNOWNED_BITS_PARTS.some((entry) => entry.namespace === namespace && entry.part === part);
}

/**
 * How a file uses the names it takes from bits-ui: a namespace is used only
 * as `Namespace.Part` (in script, in a type or as a component), never
 * whole, and never to reach a link or image part.
 */
function bitsUsesOf(
	visits: readonly Visit[],
	locals: ReadonlyMap<string, string>
): readonly Finding[] {
	const uses: Finding[] = [];
	const partUse = (namespace: string, part: string, written: string): void => {
		if (isUnownedPart(namespace, part)) {
			uses.push({
				reason: 'a bits-ui link or image part',
				detail: `${namespace}.${part} as ${written}`
			});
		}
	};
	for (const { node, parent, key } of visits) {
		if (node.type === 'Component' && typeof node.name === 'string') {
			const [head, part] = node.name.split('.');
			const namespace = head === undefined ? undefined : locals.get(head);
			if (namespace === undefined) continue;
			if (part === undefined) {
				uses.push({
					reason: 'a bits-ui namespace used other than by its parts',
					detail: `<${node.name}>`
				});
			} else {
				partUse(namespace, part, node.name);
			}
			continue;
		}
		if (node.type !== 'Identifier' || typeof node.name !== 'string') continue;
		const namespace = locals.get(node.name);
		if (namespace === undefined || parent === undefined) continue;
		const declares = ['ImportSpecifier', 'ImportDefaultSpecifier', 'ImportNamespaceSpecifier'];
		if (declares.includes(String(parent.type)) || parent.type === 'ExportSpecifier') continue;
		const isMemberObject =
			(parent.type === 'MemberExpression' && key === 'object') ||
			(parent.type === 'TSQualifiedName' && key === 'left');
		const isPropertyName =
			(parent.type === 'MemberExpression' && key === 'property' && parent.computed !== true) ||
			(parent.type === 'TSQualifiedName' && key === 'right') ||
			(parent.type === 'Property' &&
				key === 'key' &&
				parent.computed !== true &&
				parent.shorthand !== true);
		if (isPropertyName) continue;
		if (!isMemberObject) {
			uses.push({ reason: 'a bits-ui namespace used other than by its parts', detail: node.name });
			continue;
		}
		const part = memberName(parent);
		if (part === undefined) {
			uses.push({
				reason: 'a bits-ui namespace used other than by its parts',
				detail: `${node.name}[…]`
			});
		} else {
			partUse(namespace, part, `${node.name}.${part}`);
		}
	}
	return uses;
}

/** A local `export { Name }` of a bits-ui namespace hands the whole namespace on. */
function exportedLocals(
	visits: readonly Visit[],
	locals: ReadonlyMap<string, string>
): readonly Finding[] {
	return visits.flatMap(({ node }) => {
		const isLocalExport =
			node.type === 'ExportNamedDeclaration' && (node.source === null || node.source === undefined);
		const specifiers: readonly unknown[] =
			isLocalExport && Array.isArray(node.specifiers) ? node.specifiers : [];
		return specifiers.flatMap((specifier) => {
			const local = isFields(specifier) ? nameOf(specifier.local) : undefined;
			return local !== undefined && locals.has(local)
				? [
						{
							reason: 'a bits-ui namespace used other than by its parts' as const,
							detail: `export { ${local} }`
						}
					]
				: [];
		});
	});
}

function parseSvelte(text: string): Fields | string {
	try {
		const root: unknown = parse(text, { modern: true });
		return isFields(root) ? root : 'the parser gave no tree';
	} catch (error) {
		return error instanceof Error ? error.message : 'the parser gave up';
	}
}

function emptyFacts(filePath: string, unparsed: string | undefined): Facts {
	return {
		path: filePath,
		unparsed,
		owned: new Set(),
		dynamicElements: 0,
		pageElements: new Set(),
		classes: new Set(),
		styled: new Set(),
		imports: [],
		bitsUses: [],
		spinnerTags: [],
		spinnerValues: []
	};
}

/** Where a `$lib` or relative specifier points, as a path from the package; undefined for any other. */
function resolvedPath(from: string, source: string): string | undefined {
	const bare = source.split(/[?#]/)[0] ?? '';
	const isLib = bare === '$lib' || bare.startsWith('$lib/');
	if (!isLib && !bare.startsWith('.')) return undefined;
	const joined = isLib
		? path.posix.join('src/lib', bare.slice('$lib'.length))
		: path.posix.join(path.posix.dirname(from), bare);
	return joined.replace(/\/+$/, '');
}

/** True for a specifier that reaches the package entry: `@zurfur/ui`, or a path to it. */
function isEntrySpecifier(from: string, source: string): boolean {
	return source === '@zurfur/ui' || ENTRY_TARGETS.includes(resolvedPath(from, source) ?? '');
}

/** The local names a file binds Spinner to, from its own file or from the package entry. */
function spinnerLocals(filePath: string, imports: readonly ImportRecord[]): ReadonlySet<string> {
	const locals = imports.flatMap(({ source, typeOnly, names }) => {
		if (typeOnly || source === undefined) return [];
		const fromEntry = isEntrySpecifier(filePath, source);
		const fromFile = resolvedPath(filePath, source) === SPINNER_FILE;
		return names
			.filter(
				({ imported }) =>
					(fromEntry && imported === 'Spinner') || (fromFile && imported === 'default')
			)
			.map(({ local }) => local);
	});
	return new Set(locals);
}

/** True when a tag's attributes hold no spread and exactly one `kind`, the literal `kind="glyph"`. */
function isLiteralGlyph(attributes: unknown): boolean {
	const list: readonly unknown[] = Array.isArray(attributes) ? attributes : [];
	if (list.some((attribute) => isFields(attribute) && attribute.type === 'SpreadAttribute')) {
		return false;
	}
	const kinds = list.filter((attribute) => isFields(attribute) && attribute.name === 'kind');
	const [kind] = kinds;
	if (kinds.length !== 1 || !isFields(kind) || kind.type !== 'Attribute') return false;
	const parts: readonly unknown[] = Array.isArray(kind.value) ? kind.value : [];
	return parts.length === 1 && attributeTexts(parts).join('') === 'glyph';
}

/** Each Spinner tag not given the literal glyph, and each use of a Spinner import as a value. */
function spinnerUsesOf(
	visits: readonly Visit[],
	locals: ReadonlySet<string>
): Pick<Facts, 'spinnerTags' | 'spinnerValues'> {
	const spinnerTags: string[] = [];
	const spinnerValues: string[] = [];
	const declares = ['ImportSpecifier', 'ImportDefaultSpecifier', 'ImportNamespaceSpecifier'];
	for (const { node, parent, key } of visits) {
		const name = typeof node.name === 'string' ? node.name : '';
		if (node.type === 'Component') {
			const isSpinner = locals.has(name) || name === 'Spinner' || name.endsWith('.Spinner');
			if (isSpinner && !isLiteralGlyph(node.attributes)) spinnerTags.push(`<${name}>`);
			continue;
		}
		if (node.type !== 'Identifier' || !locals.has(name) || parent === undefined) continue;
		const isTypeQuery = parent.type === 'TSTypeQuery' && key === 'exprName';
		const isPropertyName =
			(parent.type === 'MemberExpression' && key === 'property' && parent.computed !== true) ||
			(parent.type === 'Property' &&
				key === 'key' &&
				parent.computed !== true &&
				parent.shorthand !== true);
		if (declares.includes(String(parent.type)) || isTypeQuery || isPropertyName) continue;
		spinnerValues.push(name);
	}
	return { spinnerTags, spinnerValues };
}

/** The classes a parsed `.svelte` file's own `<style>` block names. */
function ownStyleClasses(root: Fields): ReadonlySet<string> {
	const styled = new Set<string>();
	const sheet = root.css;
	selectorClasses(isFields(sheet) ? sheet.children : [], styled);
	return styled;
}

/** What the rules read off a `.svelte` file. */
function svelteFacts(file: SourceFile): Facts {
	const root = parseSvelte(file.text);
	if (typeof root === 'string') return emptyFacts(file.path, root);
	const visits = visitsOf(root);
	const owned = new Set<OwnedElement>();
	const pageElements = new Set<string>();
	const classes = new Set<string>();
	const imports: ImportRecord[] = [];
	let dynamicElements = 0;
	let unparsed: string | undefined;

	const renders = (name: string): void => {
		if (isOwnedElement(name)) owned.add(name);
		if (name === 'link' || name === 'base' || name === 'style') pageElements.add(name);
	};
	const addClasses = (text: string): void => {
		for (const token of classTokens(text)) classes.add(token);
	};

	for (const { node, inSvg } of visits) {
		const rawName = typeof node.name === 'string' ? node.name : '';
		switch (node.type) {
			case 'RegularElement': {
				const name = elementName(rawName, inSvg);
				renders(name);
				if (name === 'style') {
					const nested = stylesheetClasses(elementText(node));
					if (typeof nested === 'string') unparsed = nested;
					else for (const name of nested) addClasses(name);
				}
				break;
			}
			case 'SvelteElement': {
				const tag = staticTextOf(node.tag);
				if (tag === undefined) dynamicElements += 1;
				else renders(elementName(tag, inSvg));
				break;
			}
			case 'SvelteHead':
				pageElements.add('svelte:head');
				break;
			case 'Attribute':
				for (const text of attributeTexts(node.value)) addClasses(text);
				break;
			case 'ClassDirective':
			case 'ClassSelector':
				addClasses(rawName);
				break;
			case 'Literal':
				if (typeof node.value === 'string') addClasses(node.value);
				break;
			case 'TemplateElement':
				addClasses(quasiText(node) ?? '');
				break;
			default: {
				const record = estreeImport(node);
				if (record !== undefined) imports.push(record);
			}
		}
	}

	const locals = new Map(
		imports
			.filter((record) => record.source === 'bits-ui')
			.flatMap((record) => record.names.map((name) => [name.local, name.imported] as const))
	);
	const bitsUses = [...bitsUsesOf(visits, locals), ...exportedLocals(visits, locals)];
	return {
		path: file.path,
		unparsed,
		owned,
		dynamicElements,
		pageElements,
		classes,
		styled: ownStyleClasses(root),
		imports,
		bitsUses,
		...spinnerUsesOf(visits, spinnerLocals(file.path, imports))
	};
}

/** The import a TypeScript statement or expression makes, if any. */
function tsImport(node: ts.Node): ImportRecord | undefined {
	const text = (specifier: ts.Node | undefined): string | undefined =>
		specifier !== undefined && ts.isStringLiteralLike(specifier) ? specifier.text : undefined;
	if (ts.isImportDeclaration(node)) {
		const clause = node.importClause;
		const source = text(node.moduleSpecifier);
		const typeOnly = clause?.phaseModifier === ts.SyntaxKind.TypeKeyword;
		if (clause === undefined) return { source, form: 'bare', typeOnly, names: [] };
		const bindings = clause.namedBindings;
		if (bindings !== undefined && ts.isNamespaceImport(bindings)) {
			return { source, form: 'namespace', typeOnly, names: [] };
		}
		const named = (bindings?.elements ?? []).map((element) => ({
			imported: (element.propertyName ?? element.name).text,
			local: element.name.text
		}));
		const defaults =
			clause.name === undefined ? [] : [{ imported: 'default', local: clause.name.text }];
		const form = clause.name === undefined ? 'named' : 'default';
		return { source, form, typeOnly, names: [...defaults, ...named] };
	}
	if (ts.isExportDeclaration(node) && node.moduleSpecifier !== undefined) {
		const source = text(node.moduleSpecifier);
		const clause = node.exportClause;
		if (clause === undefined || ts.isNamespaceExport(clause)) {
			return { source, form: 'reexport-all', typeOnly: node.isTypeOnly, names: [] };
		}
		const names = clause.elements.map((element) => ({
			imported: (element.propertyName ?? element.name).text,
			local: element.name.text
		}));
		return { source, form: 'reexport', typeOnly: node.isTypeOnly, names };
	}
	if (ts.isImportEqualsDeclaration(node) && ts.isExternalModuleReference(node.moduleReference)) {
		return {
			source: text(node.moduleReference.expression),
			form: 'namespace',
			typeOnly: false,
			names: []
		};
	}
	const isDynamic =
		ts.isCallExpression(node) &&
		(node.expression.kind === ts.SyntaxKind.ImportKeyword ||
			(ts.isIdentifier(node.expression) && node.expression.text === 'require'));
	if (isDynamic)
		return { source: text(node.arguments[0]), form: 'dynamic', typeOnly: false, names: [] };
	if (ts.isImportTypeNode(node)) {
		const argument = node.argument;
		const literal = ts.isLiteralTypeNode(argument) ? argument.literal : undefined;
		return { source: text(literal), form: 'type-query', typeOnly: true, names: [] };
	}
	return undefined;
}

/** What the rules read off a `.ts` or `.js` file: its strings' classes and its imports. */
function scriptFacts(file: SourceFile): Facts {
	const source = ts.createSourceFile(file.path, file.text, ts.ScriptTarget.Latest, true);
	const classes = new Set<string>();
	const imports: ImportRecord[] = [];
	const visit = (node: ts.Node): void => {
		const isText =
			ts.isStringLiteralLike(node) ||
			ts.isTemplateHead(node) ||
			ts.isTemplateMiddle(node) ||
			ts.isTemplateTail(node);
		if (isText) {
			for (const token of classTokens(node.text)) classes.add(token);
		}
		const record = tsImport(node);
		if (record !== undefined) imports.push(record);
		ts.forEachChild(node, visit);
	};
	visit(source);
	return { ...emptyFacts(file.path, undefined), classes, imports };
}

function factsOf(file: SourceFile): Facts {
	return file.path.endsWith('.svelte') ? svelteFacts(file) : scriptFacts(file);
}

/** True when `token` is family `family` itself, or one of its `__` parts or `--` forms. */
function inFamily(token: string, family: Family): boolean {
	return token === family || token.startsWith(`${family}__`) || token.startsWith(`${family}--`);
}

/** The value a table record keeps for a file, or undefined. */
function rowFor<T>(record: Readonly<Record<string, T>>, filePath: string): T | undefined {
	return Object.entries(record).find(([file]) => file === filePath)?.[1];
}

/** True for a Svelte CSF stories file under src/stories/. */
function isStoriesFile(filePath: string): boolean {
	return filePath.startsWith('src/stories/') && filePath.endsWith('.stories.svelte');
}

/** The library `.svelte` files a file imports to render, read back to their paths. */
function libraryImports(facts: Facts, exported: readonly ExportedComponent[]): readonly string[] {
	return facts.imports.flatMap((record) => {
		if (record.typeOnly || record.source === undefined) return [];
		if (record.source === '@zurfur/ui') {
			return record.names.flatMap(
				(name) => exported.find((component) => component.name === name.imported)?.file ?? []
			);
		}
		const target = resolvedPath(facts.path, record.source);
		const isLibrarySvelte = target?.startsWith('src/lib/') === true && target.endsWith('.svelte');
		return isLibrarySvelte ? [target] : [];
	});
}

/** The specifiers by which a library file imports the package entry. */
function entryImports(facts: Facts): readonly string[] {
	if (!facts.path.startsWith('src/lib/')) return [];
	return facts.imports.flatMap(({ source }) =>
		source !== undefined && isEntrySpecifier(facts.path, source) ? [source] : []
	);
}

/** The refusals of the bits-ui rule in one file's imports. */
function bitsImportRefusals(facts: Facts, table: CompositionTable): readonly Finding[] {
	const listed = rowFor(table.bitsUi, facts.path) ?? [];
	return facts.imports.flatMap((record): readonly Finding[] => {
		const { source } = record;
		if (source === undefined) {
			return record.form === 'dynamic' || record.form === 'type-query'
				? [{ reason: 'a computed import', detail: 'import(…)' }]
				: [];
		}
		if (source !== 'bits-ui') {
			return /(?:^|\/)bits-ui(?:\/|$)/.test(source)
				? [{ reason: 'a deep bits-ui path', detail: source }]
				: [];
		}
		switch (record.form) {
			case 'namespace':
			case 'reexport-all':
			case 'dynamic':
			case 'type-query':
				return [{ reason: 'the whole of bits-ui imported', detail: record.form }];
			case 'bare':
				return [];
			case 'default':
			case 'named':
			case 'reexport':
				return record.names.flatMap((name): readonly Finding[] => {
					if (name.imported === 'default') {
						return [{ reason: 'a default import of bits-ui', detail: name.local }];
					}
					const allowed = listed.includes(name.imported) && record.form !== 'reexport';
					return allowed
						? []
						: [{ reason: 'a bits-ui name not listed for the file', detail: name.imported }];
				});
		}
	});
}

/**
 * Every refusal of the lego rule: each export has a kind, a simple component
 * holds only glyphs and internal pieces, a composite composes and draws no
 * owned element, owned elements and `zf-` families stay with the files the
 * table lists, each family has one stylesheet row and none for a family a
 * component styles itself, a simple file holds Spinner only as the glyph, no
 * library file imports the package entry, stories compose and carry their
 * component's kind, bits-ui names stay with their files and its link and image
 * parts with none, and the exceptions only shrink.
 */
export function compositionRefusals(input: CompositionInput): readonly CompositionRefusal[] {
	const { table } = input;
	const refusals: CompositionRefusal[] = [];
	const refuse = (filePath: string, reason: CompositionRefusalReason, detail: string): void => {
		refusals.push({ path: filePath, reason, detail });
	};
	const exported = exportedComponents(input.entry);
	const exportedFiles = new Map(exported.map((component) => [component.file, component.name]));
	const internalFiles = new Set<string>(table.internal.map((row) => row.file));
	const glyphFiles = new Set<string>(
		exported.filter((component) => table.glyphs.includes(component.name)).map((c) => c.file)
	);
	/** The simple library files, exported or internal, that may hold Spinner only as the glyph. */
	const glyphOnlyFiles = new Set<string>(
		[
			...exported
				.filter((component) => rowFor(table.kinds, component.name) === 'simple')
				.map((component) => component.file),
			...internalFiles
		].filter((path) => path !== SPINNER_FILE)
	);
	const exceptionsFor = (filePath: string): readonly Exception[] =>
		table.exceptions.filter((exception) => exception.file === filePath);

	// The kinds.
	for (const component of exported) {
		if (rowFor(table.kinds, component.name) === undefined) {
			refuse(entryPath, 'an exported component with no kind', component.name);
		}
	}
	for (const name of Object.keys(table.kinds)) {
		if (!exported.some((component) => component.name === name)) {
			refuse(entryPath, 'a kind for a name the entry does not export', name);
		}
	}

	// The table's own rows.
	for (const exception of table.exceptions) {
		const known = input.knownExceptions.find((row) => row.file === exception.file);
		const grants = [...exception.elements, ...exception.families];
		const knownGrants: readonly string[] =
			known === undefined ? [] : [...known.elements, ...known.families];
		for (const grant of grants) {
			if (!knownGrants.includes(grant)) {
				refuse(exception.file, 'an exception outside the known list', grant);
			}
		}
	}
	for (const [file, names] of Object.entries(table.bitsUi)) {
		for (const name of names) {
			if (UNLISTED_BITS_NAMESPACES.includes(name)) {
				refuse(file, 'a bits-ui row that lists Avatar or Button', name);
			}
		}
	}
	const stylesheetRows = Object.entries(table.families).filter(([file]) => file.endsWith('.css'));
	const homes = new Map<Family, string>();
	for (const [file, families] of stylesheetRows) {
		for (const family of families) {
			const home = homes.get(family);
			if (home === undefined) homes.set(family, file);
			else refuse(file, 'a family listed by two stylesheet rows', `${family}, also in ${home}`);
		}
	}

	// Each source file.
	for (const file of input.sources) {
		const facts = factsOf(file);
		if (facts.unparsed !== undefined) {
			refuse(file.path, 'does not parse', facts.unparsed);
			continue;
		}
		const exceptions = exceptionsFor(file.path);
		const isLibrarySvelte = file.path.startsWith('src/lib/') && file.path.endsWith('.svelte');

		for (const element of facts.owned) {
			const listed =
				table.owners[element].some((row) => row.file === file.path) ||
				exceptions.some((exception) => exception.elements.includes(element));
			if (!listed) refuse(file.path, 'an owned element in a file not listed for it', element);
		}
		if (facts.dynamicElements > 0 && !table.dynamicElement.some((row) => row.file === file.path)) {
			refuse(file.path, 'a dynamic svelte:element in a file not listed for it', 'svelte:element');
		}
		if (PAGE_ELEMENT_FOLDERS.some((folder) => file.path.startsWith(folder))) {
			for (const element of PAGE_ELEMENTS) {
				if (facts.pageElements.has(element)) {
					refuse(file.path, 'a page element in the library or the stories', element);
				}
			}
		}

		const families = [
			...(rowFor(table.families, file.path) ?? []),
			...exceptions.flatMap((exception) => exception.families),
			...(isLibrarySvelte ? table.sharedFamilies : [])
		];
		for (const token of facts.classes) {
			if (!families.some((family) => inFamily(token, family))) {
				refuse(file.path, "a zf- class outside the file's families", token);
			}
		}

		for (const refusal of bitsImportRefusals(facts, table)) {
			refuse(file.path, refusal.reason, refusal.detail);
		}
		for (const use of facts.bitsUses) refuse(file.path, use.reason, use.detail);
		for (const source of entryImports(facts)) {
			refuse(file.path, 'an import of the package entry from inside the library', source);
		}

		// A family a component styles in its own <style> has no stylesheet row.
		for (const [sheet, sheetFamilies] of stylesheetRows) {
			for (const family of sheetFamilies) {
				if ([...facts.styled].some((name) => inFamily(name, family))) {
					refuse(
						sheet,
						'a stylesheet row for a family a component styles itself',
						`${family}, styled in ${file.path}`
					);
				}
			}
		}

		// A simple file holds Spinner only as the glyph, by a tag that says so in plain text.
		if (glyphOnlyFiles.has(file.path)) {
			for (const tag of facts.spinnerTags) {
				refuse(file.path, 'a Spinner rendered other than as a glyph', tag);
			}
			for (const name of facts.spinnerValues) {
				refuse(file.path, 'a Spinner rendered other than as a glyph', name);
			}
		}

		// The rows that name this file must still be needed.
		for (const exception of exceptions) {
			for (const element of exception.elements) {
				if (!facts.owned.has(element)) {
					refuse(file.path, 'an exception the file no longer needs', element);
				}
			}
			for (const family of exception.families) {
				if (![...facts.classes].some((token) => inFamily(token, family))) {
					refuse(file.path, 'an exception the file no longer needs', family);
				}
			}
		}
		for (const element of OWNED_ELEMENTS) {
			const isOwner = table.owners[element].some((row) => row.file === file.path);
			if (isOwner && !facts.owned.has(element)) {
				refuse(file.path, 'an owner row the file does not use', element);
			}
		}
		const bitsNames = new Set(
			facts.imports
				.filter((record) => record.source === 'bits-ui')
				.flatMap((record) => record.names.map((name) => name.imported))
		);
		for (const name of rowFor(table.bitsUi, file.path) ?? []) {
			if (!bitsNames.has(name)) refuse(file.path, 'a bits-ui row the file does not use', name);
		}

		// The kinds, file by file.
		if (isLibrarySvelte) {
			const name = exportedFiles.get(file.path);
			const isInternal = internalFiles.has(file.path);
			const exportedKind = name === undefined ? undefined : rowFor(table.kinds, name);
			const kind: Kind | undefined = isInternal ? 'simple' : exportedKind;
			if (name === undefined && !isInternal) {
				refuse(file.path, 'a library component neither exported nor internal', file.path);
			}
			const imported = libraryImports(facts, exported);
			if (kind === 'simple') {
				for (const target of imported) {
					if (!glyphFiles.has(target) && !internalFiles.has(target)) {
						refuse(file.path, 'a simple component that holds another component', target);
					}
				}
				if ((rowFor(table.families, file.path) ?? []).length === 0) {
					refuse(file.path, 'a simple component with no family of its own', file.path);
				}
			}
			if (kind === 'composite') {
				if (!imported.some((target) => exportedFiles.has(target))) {
					refuse(file.path, 'a composite that imports no library component', file.path);
				}
				for (const element of facts.owned) {
					refuse(file.path, 'a composite that renders an owned element', element);
				}
			}
		}

		// Stories carry their component's kind.
		if (isStoriesFile(file.path)) {
			const meta = storyMeta(file);
			const expected =
				meta.component === undefined ? undefined : rowFor(table.kinds, meta.component);
			if (meta.tags.kind === 'unread') {
				refuse(file.path, 'a stories file whose tags cannot be read', 'tags');
			} else {
				const tags = meta.tags.kind === 'list' ? meta.tags.tags : [];
				const kindTags = tags.filter((tag) => tag === 'simple' || tag === 'composite');
				const carriesExactly =
					expected === undefined
						? kindTags.length === 0
						: kindTags.length === 1 && kindTags[0] === expected;
				if (!carriesExactly) {
					refuse(
						file.path,
						'a stories file whose tags do not carry exactly its kind',
						`${expected ?? 'no kind'}: [${tags.join(', ')}]`
					);
				}
			}
		}
	}

	// The stylesheets: one in a styles folder by its row, any other by the shared families.
	for (const sheet of input.stylesheets) {
		const listed =
			rowFor(table.families, sheet.path) ??
			(isFamilyStylesheet(sheet.path) ? undefined : table.sharedFamilies);
		if (listed === undefined) {
			refuse(sheet.path, 'a family stylesheet with no row', sheet.path);
			continue;
		}
		const classes = stylesheetClasses(sheet.text);
		if (typeof classes === 'string') {
			refuse(sheet.path, 'does not parse', classes);
			continue;
		}
		const exceptions = exceptionsFor(sheet.path);
		const families = [...listed, ...exceptions.flatMap((exception) => exception.families)];
		const written = [...classes].filter((name) => name.startsWith('zf-'));
		for (const name of written) {
			if (!families.some((family) => inFamily(name, family))) {
				refuse(sheet.path, "a zf- class outside the file's families", name);
			}
		}
		for (const exception of exceptions) {
			for (const family of exception.families) {
				if (!written.some((name) => inFamily(name, family))) {
					refuse(sheet.path, 'an exception the file no longer needs', family);
				}
			}
		}
	}

	return refusals;
}

// ---------------------------------------------------------------------------
// The real tree.
// ---------------------------------------------------------------------------

const uiRoot = fileURLToPath(new URL('../../', import.meta.url));

function listFiles(folder: string): readonly string[] {
	return readdirSync(`${uiRoot}${folder}`, { withFileTypes: true }).flatMap((entry) => {
		const childPath = `${folder}/${entry.name}`;
		if (entry.isDirectory()) return listFiles(childPath);
		return entry.isFile() ? [childPath] : [];
	});
}

/**
 * True for a file the check reads: every `.svelte`, `.ts` and `.js` file
 * under src/ but the specs and the checks themselves, the spec harnesses in
 * `src/checks/testing/` included.
 */
function isReadSource(filePath: string): boolean {
	const isCode = /\.(?:svelte|ts|js)$/.test(filePath);
	const isSpec = /\.spec\.(?:ts|js)$/.test(filePath);
	const isCheck = filePath.startsWith('src/checks/') && !filePath.startsWith('src/checks/testing/');
	return isCode && !isSpec && !isCheck;
}

/** True for a family stylesheet: a `.css` file in any folder under `src/lib/styles/`. */
function isFamilyStylesheet(filePath: string): boolean {
	return /^src\/lib\/styles\/.+\/[^/]+\.css$/.test(filePath);
}

/** True for a stylesheet the check reads: every `.css` file under `src/lib/`. */
function isLibraryStylesheet(filePath: string): boolean {
	return filePath.startsWith('src/lib/') && filePath.endsWith('.css');
}

function read(filePath: string): SourceFile {
	return { path: filePath, text: readFileSync(`${uiRoot}${filePath}`, 'utf8') };
}

const allFiles = [...listFiles('src')].sort();
const realSources = allFiles.filter(isReadSource).map(read);
const realStylesheets = allFiles.filter(isLibraryStylesheet).map(read);
const realEntry = readFileSync(`${uiRoot}${entryPath}`, 'utf8');

/** The check over `sources` and `stylesheets`, with the real entry, table and known list unless given. */
function check(overrides: Partial<CompositionInput>): readonly CompositionRefusal[] {
	return compositionRefusals({
		entry: realEntry,
		sources: [],
		stylesheets: [],
		table: TABLE,
		knownExceptions: KNOWN_EXCEPTIONS,
		...overrides
	});
}

function reasons(refusals: readonly CompositionRefusal[]): readonly CompositionRefusalReason[] {
	return refusals.map((refusal) => refusal.reason);
}

/** A real file's text, with `from` replaced by `to` (which must change it). */
function realWith(filePath: string, from: string, to: string): SourceFile {
	const { text } = read(filePath);
	const changed = text.replace(from, to);
	expect(changed).not.toBe(text);
	return { path: filePath, text: changed };
}

/** A `.svelte` file with `script` in its instance script and `markup` after it. */
function svelteFile(filePath: string, script: string, markup: string): SourceFile {
	return { path: filePath, text: `<script lang="ts">\n${script}\n</script>\n\n${markup}\n` };
}

// ---------------------------------------------------------------------------
// A synthetic composite, and the table and entry that know it.
// ---------------------------------------------------------------------------

const bannerPath = 'src/lib/surfaces/Banner.svelte';
const bannerEntry = `${realEntry}export { default as Banner } from './surfaces/Banner.svelte';\n`;
const bannerTable: CompositionTable = {
	...TABLE,
	kinds: { ...TABLE.kinds, Banner: 'composite' },
	families: { ...TABLE.families, [bannerPath]: ['zf-inline'] }
};

const bannerScript = [
	"\timport Button from '../controls/Button.svelte';",
	"\timport IconButton from '../controls/IconButton.svelte';",
	"\timport Icon from '../icons/Icon.svelte';"
].join('\n');

const bannerMarkup = [
	'<div class="zf-inline">',
	'\t<Icon name="info" />',
	'\t<p class="zf-inline__title">Saved</p>',
	'\t<span class="zf-sr">Notice</span>',
	'\t<Button onPress={() => {}}>Undo</Button>',
	'\t<IconButton icon="x" label="Dismiss" onPress={() => {}} />',
	'</div>'
].join('\n');

function checkBanner(script: string, markup: string): readonly CompositionRefusalReason[] {
	const banner = svelteFile(bannerPath, script, markup);
	return reasons(check({ entry: bannerEntry, table: bannerTable, sources: [banner] }));
}

describe('the lego check', () => {
	it('passes the real tree', () => {
		expect(check({ sources: realSources, stylesheets: realStylesheets })).toEqual([]);
	});

	it('reads the whole of src/, the gallery, the routes and the spec harnesses included', () => {
		const folders = new Set(
			realSources.map((source) => source.path.split('/').slice(0, 2).join('/'))
		);
		expect([...folders].sort()).toEqual(
			expect.arrayContaining(['src/checks', 'src/gallery', 'src/lib', 'src/routes', 'src/stories'])
		);
		expect(realSources.some((source) => source.path.startsWith('src/checks/testing/'))).toBe(true);
		const sheets = realStylesheets.map((sheet) => sheet.path);
		expect(sheets).toEqual(
			expect.arrayContaining([
				'src/lib/styles/base.css',
				'src/lib/motion/paper-moves.css',
				'src/lib/styles/controls/button.css',
				'src/lib/styles/surfaces/dialog.css'
			])
		);
	});

	it('reads Spinner from the file the entry exports it from', () => {
		expect(exportedComponents(realEntry)).toContainEqual({ name: 'Spinner', file: SPINNER_FILE });
	});

	it('names only files that exist', () => {
		const named = [
			...Object.keys(TABLE.families),
			...Object.keys(TABLE.bitsUi),
			...Object.values(TABLE.owners)
				.flat()
				.map((row) => row.file),
			...TABLE.internal.map((row) => row.file),
			...TABLE.exceptions.map((row) => row.file),
			...TABLE.dynamicElement.map((row) => row.file)
		];
		expect(named.filter((file) => !existsSync(`${uiRoot}${file}`))).toEqual([]);
	});

	it('gives each family one library file and keeps rows to the library', () => {
		const svelteRows = Object.entries(FAMILIES).filter(([file]) => file.endsWith('.svelte'));
		const owned = svelteRows.flatMap(([, families]) => families);
		expect(owned.filter((family, index) => owned.indexOf(family) !== index)).toEqual([]);
		expect(Object.keys(FAMILIES).filter((file) => !file.startsWith('src/lib/'))).toEqual([]);
	});

	it('lists every family a library file writes in a family stylesheet or its own style block', () => {
		const stylesheetFamilies = Object.entries(FAMILIES)
			.filter(([file]) => file.endsWith('.css'))
			.flatMap(([, families]) => families);
		const unstyled = Object.entries(FAMILIES)
			.filter(([file]) => file.endsWith('.svelte'))
			.flatMap(([file, families]) => {
				const styled = factsOf(read(file)).styled;
				return families.filter(
					(family) =>
						!stylesheetFamilies.includes(family) &&
						![...styled].some((name) => inFamily(name, family))
				);
			});
		expect(unstyled).toEqual([]);
	});

	it("reads a component's own <style> with the parser, so Icon styles zf-icon itself", () => {
		expect([...factsOf(read('src/lib/icons/Icon.svelte')).styled]).toContain('zf-icon');
	});

	it('keeps the known exceptions to the table, so a fixed one leaves both', () => {
		const rows = EXCEPTIONS.map(({ file, elements, families }) => ({ file, elements, families }));
		expect(rows).toEqual(KNOWN_EXCEPTIONS);
	});

	it('passes a composite that lays out library components, its own family and plain text', () => {
		expect(checkBanner(bannerScript, bannerMarkup)).toEqual([]);
	});

	it('passes a composite that shows a loading status, as a tag or handed on as a value', () => {
		const script = `${bannerScript}\n\timport Spinner from './Spinner.svelte';\n\tconst Ring = Spinner;`;
		const markup = `${bannerMarkup}\n<Spinner kind="status" label="Saving" active />\n<Ring kind="glyph" />`;
		expect(checkBanner(script, markup)).toEqual([]);
	});

	it('passes a composite built from another composite', () => {
		const entry = `${bannerEntry}export { default as Notice } from './surfaces/Notice.svelte';\n`;
		const table: CompositionTable = {
			...bannerTable,
			kinds: { ...bannerTable.kinds, Notice: 'composite' }
		};
		const notice = svelteFile(
			'src/lib/surfaces/Notice.svelte',
			"\timport Banner from './Banner.svelte';",
			'<section><Banner /></section>'
		);
		expect(check({ entry, table, sources: [notice] })).toEqual([]);
	});
});

describe('the plants', () => {
	it('refuses an export with no kind', () => {
		const entry = `${realEntry}export { default as Banner } from './surfaces/Banner.svelte';\n`;
		expect(check({ entry })).toEqual([
			{ path: entryPath, reason: 'an exported component with no kind', detail: 'Banner' }
		]);
	});

	it('refuses a kind for a component the entry does not export', () => {
		const table: CompositionTable = { ...TABLE, kinds: { ...TABLE.kinds, Banner: 'composite' } };
		expect(reasons(check({ table }))).toEqual(['a kind for a name the entry does not export']);
	});

	it.each([
		[
			'a <button> in a stories file',
			realWith(
				'src/stories/controls/Button.stories.svelte',
				'<Story name="Secondary"',
				'<button type="button">x</button>\n\n<Story name="Secondary"'
			),
			['an owned element in a file not listed for it']
		],
		[
			'a <button> in a story helper',
			realWith(
				'src/stories/controls/ToggleHolder.svelte',
				'<Toggle ',
				'<button type="button">x</button>\n<Toggle '
			),
			['an owned element in a file not listed for it']
		],
		[
			'a zf- class in a story helper',
			realWith(
				'src/stories/controls/ToggleHolder.svelte',
				'<Toggle ',
				'<div class="zf-btn"></div>\n<Toggle '
			),
			["a zf- class outside the file's families"]
		],
		[
			"a composite tag on Button's stories",
			realWith(
				'src/stories/controls/Button.stories.svelte',
				"tags: ['autodocs', 'simple']",
				"tags: ['autodocs', 'composite']"
			),
			['a stories file whose tags do not carry exactly its kind']
		],
		[
			"Button's stories without their kind",
			realWith(
				'src/stories/controls/Button.stories.svelte',
				"tags: ['autodocs', 'simple']",
				"tags: ['autodocs']"
			),
			['a stories file whose tags do not carry exactly its kind']
		],
		[
			"both kinds on Button's stories",
			realWith(
				'src/stories/controls/Button.stories.svelte',
				"tags: ['autodocs', 'simple']",
				"tags: ['autodocs', 'simple', 'composite']"
			),
			['a stories file whose tags do not carry exactly its kind']
		],
		[
			"tags written as a variable on Button's stories",
			realWith(
				'src/stories/controls/Button.stories.svelte',
				"tags: ['autodocs', 'simple']",
				'tags: [...kindTags]'
			),
			['a stories file whose tags cannot be read']
		],
		[
			'a kind tag on a token page',
			realWith(
				'src/stories/tokens/Colour.stories.svelte',
				"title: 'Tokens and Paper/Colour',",
				"title: 'Tokens and Paper/Colour',\n\t\ttags: ['simple'],"
			),
			['a stories file whose tags do not carry exactly its kind']
		],
		[
			'an <a> in an unlisted gallery file',
			realWith('src/gallery/GalleryPage.svelte', '<style>', '<a href="/">x</a>\n\n<style>'),
			['an owned element in a file not listed for it']
		],
		[
			'an <img> in an unlisted route',
			svelteFile('src/routes/components/rogue/+page.svelte', '', '<img src="/a.png" alt="" />'),
			['an owned element in a file not listed for it']
		],
		[
			'an <input> in a spec harness',
			realWith(
				'src/checks/testing/AcceptingMenu.svelte',
				'<Menu ',
				'<input type="text" />\n<Menu '
			),
			['an owned element in a file not listed for it']
		],
		[
			'<iMg> in a library file',
			realWith(
				'src/lib/controls/Toggle.svelte',
				'<div class="zf-toggle">',
				'<div class="zf-toggle"><iMg src="/a.png" alt="" />'
			),
			['an owned element in a file not listed for it']
		],
		[
			'<image src> outside <svg>',
			realWith(
				'src/lib/controls/Toggle.svelte',
				'<div class="zf-toggle">',
				'<div class="zf-toggle"><image src="/a.png" />'
			),
			['an owned element in a file not listed for it']
		],
		[
			'an SVG <image>',
			realWith(
				'src/lib/controls/Toggle.svelte',
				'<div class="zf-toggle">',
				'<div class="zf-toggle"><svg><image href="/a.png" /></svg>'
			),
			['an owned element in a file not listed for it']
		],
		[
			'a static <svelte:element this="img">',
			realWith(
				'src/lib/controls/Toggle.svelte',
				'<div class="zf-toggle">',
				'<div class="zf-toggle"><svelte:element this="img" src="/a.png" />'
			),
			['an owned element in a file not listed for it']
		],
		[
			'a dynamic <svelte:element>',
			realWith(
				'src/lib/controls/Toggle.svelte',
				'<div class="zf-toggle">',
				"<div class=\"zf-toggle\"><svelte:element this={label === '' ? 'img' : 'span'} />"
			),
			['a dynamic svelte:element in a file not listed for it']
		],
		[
			'a <video> in Button',
			realWith(
				'src/lib/controls/Button.svelte',
				'{#if href !== undefined}',
				'<video></video>\n{#if href !== undefined}'
			),
			['an owned element in a file not listed for it']
		]
	] as const)('refuses %s', (_label, file, expected) => {
		expect(reasons(check({ sources: [file] }))).toEqual(expected);
	});

	it.each([
		['<link>', '<link rel="stylesheet" href="/a.css" />', 'link'],
		['<base>', '<base href="/" />', 'base'],
		['<svelte:head>', '<svelte:head><title>x</title></svelte:head>', 'svelte:head'],
		['a nested <style>', '<div><style>p { margin: 0; }</style></div>', 'style']
	] as const)('refuses %s in a library file and in a stories file', (_label, markup, element) => {
		const lib = realWith(
			'src/lib/controls/Toggle.svelte',
			'<div class="zf-toggle">',
			`${markup}\n<div class="zf-toggle">`
		);
		const stories = realWith(
			'src/stories/controls/Toggle.stories.svelte',
			'{#snippet holder',
			`${markup}\n\n{#snippet holder`
		);
		for (const file of [lib, stories]) {
			expect(check({ sources: [file] })).toContainEqual({
				path: file.path,
				reason: 'a page element in the library or the stories',
				detail: element
			});
		}
	});

	it('leaves the gallery host its <svelte:head>', () => {
		expect(check({ sources: [read('src/routes/+layout.svelte')] })).toEqual([]);
	});

	it('refuses a new row in EXCEPTIONS', () => {
		const table: CompositionTable = {
			...TABLE,
			exceptions: [
				...EXCEPTIONS,
				{ file: 'src/lib/controls/Toggle.svelte', elements: ['a'], families: [], reason: 'a link' }
			]
		};
		expect(check({ table })).toEqual([
			{
				path: 'src/lib/controls/Toggle.svelte',
				reason: 'an exception outside the known list',
				detail: 'a'
			}
		]);
	});

	it('refuses a wider known row, and an exception the file no longer needs', () => {
		const table: CompositionTable = {
			...TABLE,
			exceptions: EXCEPTIONS.map((row) =>
				row.file === 'src/lib/controls/Menu.svelte'
					? { ...row, families: [...row.families, 'zf-menu'] }
					: row
			)
		};
		expect(reasons(check({ table }))).toEqual(['an exception outside the known list']);
		const fixedMenu = realWith(
			'src/lib/controls/Menu.svelte',
			'class="zf-iconbtn zf-iconbtn--quiet pick"',
			'class="pick"'
		);
		expect(check({ sources: [fixedMenu] })).toEqual([
			{
				path: fixedMenu.path,
				reason: 'an exception the file no longer needs',
				detail: 'zf-iconbtn'
			}
		]);
	});

	it('refuses an owner row the file does not use', () => {
		const table: CompositionTable = {
			...TABLE,
			owners: {
				...TABLE.owners,
				a: [...TABLE.owners.a, { file: 'src/lib/controls/Toggle.svelte', reason: 'x' }]
			}
		};
		expect(check({ table, sources: [read('src/lib/controls/Toggle.svelte')] })).toEqual([
			{
				path: 'src/lib/controls/Toggle.svelte',
				reason: 'an owner row the file does not use',
				detail: 'a'
			}
		]);
	});

	it('refuses a composite drawing its own dismiss <button class="zf-iconbtn">', () => {
		const markup = bannerMarkup.replace(
			'<IconButton icon="x" label="Dismiss" onPress={() => {}} />',
			'<button type="button" class="zf-iconbtn"><Icon name="x" /></button>'
		);
		expect(checkBanner(bannerScript, markup)).toEqual([
			'an owned element in a file not listed for it',
			"a zf- class outside the file's families",
			'a composite that renders an owned element'
		]);
	});

	it('refuses class="zf-btn" in a composite, in every way a class is written', () => {
		const written = [
			'<div class="zf-btn"></div>',
			'<div class:zf-btn={true}></div>',
			"<div class={['zf-btn--primary']}></div>",
			'<div class={`zf-btn--${String(1)}`}></div>',
			"<div class={{ 'zf-btn': true }}></div>"
		];
		for (const markup of written) {
			expect(checkBanner(bannerScript, `${bannerMarkup}\n${markup}`)).toEqual([
				"a zf- class outside the file's families"
			]);
		}
		const inScript = `${bannerScript}\n\tconst classes = ['zf-iconbtn'];`;
		expect(checkBanner(inScript, `${bannerMarkup}\n<div class={classes}></div>`)).toEqual([
			"a zf- class outside the file's families"
		]);
		const inStyle = `${bannerMarkup}\n<style>\n\t:global(.zf-btn) { margin: 0; }\n</style>`;
		expect(checkBanner(bannerScript, inStyle)).toEqual([
			"a zf- class outside the file's families",
			'a stylesheet row for a family a component styles itself'
		]);
		const dynamicFamily = `${bannerMarkup}\n<div class={\`zf-\${String(1)}\`}></div>`;
		expect(checkBanner(bannerScript, dynamicFamily)).toEqual([
			"a zf- class outside the file's families"
		]);
	});

	it('refuses a composite that imports no library component', () => {
		expect(checkBanner('', '<div class="zf-inline"><p>Saved</p></div>')).toEqual([
			'a composite that imports no library component'
		]);
	});

	it.each([
		['by a relative path', "import Button from './Button.svelte';", []],
		['through $lib', "import Button from '$lib/controls/Button.svelte';", []],
		[
			'through the package',
			"import { Button } from '@zurfur/ui';",
			['an import of the package entry from inside the library']
		]
	] as const)(
		'refuses a simple component that holds another component %s',
		(_label, line, alsoRefused) => {
			const toggle = realWith(
				'src/lib/controls/Toggle.svelte',
				"import { Switch } from 'bits-ui';",
				`import { Switch } from 'bits-ui';\n\t${line}`
			);
			expect(reasons(check({ sources: [toggle] }))).toEqual([
				...alsoRefused,
				'a simple component that holds another component'
			]);
		}
	);

	it.each([
		['../index.ts', 'src/lib/surfaces/Badge.svelte'],
		['../index.js', 'src/lib/surfaces/Badge.svelte'],
		['../index', 'src/lib/surfaces/Badge.svelte'],
		['..', 'src/lib/surfaces/Badge.svelte'],
		['../', 'src/lib/surfaces/Badge.svelte'],
		['$lib', 'src/lib/surfaces/Badge.svelte'],
		['$lib/index.ts', 'src/lib/surfaces/Badge.svelte'],
		['../index.ts', 'src/lib/surfaces/choices.ts'],
		['.', 'src/lib/extra.ts'],
		['./', 'src/lib/extra.ts'],
		['./index.ts', 'src/lib/extra.ts']
	] as const)('refuses an import of the package entry by %s in %s', (specifier, filePath) => {
		const forms = [
			`import { Toggle } from '${specifier}';`,
			`import type { EntryShape } from '${specifier}';`,
			`export { Toggle } from '${specifier}';`,
			`export * from '${specifier}';`
		];
		for (const line of forms) {
			const file = filePath.endsWith('.svelte')
				? realWith(filePath, "import Icon from '../icons/Icon.svelte';", line)
				: { path: filePath, text: `${line}\n` };
			expect(check({ sources: [file] })).toContainEqual({
				path: filePath,
				reason: 'an import of the package entry from inside the library',
				detail: specifier
			});
		}
	});

	it('refuses a simple component holding a Toggle through the package entry', () => {
		const badge = realWith(
			'src/lib/surfaces/Badge.svelte',
			"import Icon from '../icons/Icon.svelte';",
			"import Icon from '../icons/Icon.svelte';\n\timport { Toggle } from '../index.ts';"
		);
		const holding = {
			path: badge.path,
			text: badge.text.replace('<span>{label}</span>', '<span>{label}</span>\n\t<Toggle />')
		};
		expect(reasons(check({ sources: [holding] }))).toEqual([
			'an import of the package entry from inside the library'
		]);
	});

	it('leaves a stories file its import of the package', () => {
		const stories = read('src/stories/surfaces/Spinner.stories.svelte');
		expect(stories.text).toContain("from '@zurfur/ui'");
		expect(check({ sources: [stories] })).toEqual([]);
	});

	it('refuses a library component that is neither exported nor internal', () => {
		const helper = svelteFile('src/lib/controls/Helper.svelte', '', '<span></span>');
		expect(reasons(check({ sources: [helper] }))).toEqual([
			'a library component neither exported nor internal'
		]);
	});

	it('refuses .zf-btn in surfaces/dialog.css, and allows a combinator over its own parts', () => {
		const dialog = (css: string): SourceFile => ({
			path: 'src/lib/styles/surfaces/dialog.css',
			text: css
		});
		expect(check({ stylesheets: [dialog('.zf-dialog__actions > * { margin: 0; }')] })).toEqual([]);
		const refused = [
			'.zf-dialog .zf-btn { margin: 0; }',
			'.zf-dialog:is(.zf-btn--primary) { margin: 0; }',
			'.zf-dialog:not(.zf-iconbtn) { margin: 0; }',
			':where(.zf-btn) .zf-dialog__actions { margin: 0; }',
			'@media (min-width: 1px) { .zf-dialog > .zf-btn { margin: 0; } }'
		];
		for (const css of refused) {
			expect(reasons(check({ stylesheets: [dialog(css)] }))).toEqual([
				"a zf- class outside the file's families"
			]);
		}
	});

	it('refuses a family stylesheet with no row', () => {
		const sheet = { path: 'src/lib/styles/surfaces/sheet.css', text: '.zf-sheet { margin: 0; }' };
		expect(reasons(check({ stylesheets: [sheet] }))).toEqual(['a family stylesheet with no row']);
	});

	it('refuses a stylesheet in a third styles folder', () => {
		const shelf = {
			path: 'src/lib/styles/layout/shelf.css',
			text: '.zf-shelf .zf-btn { min-height: 99px; }'
		};
		expect(reasons(check({ stylesheets: [shelf] }))).toEqual(['a family stylesheet with no row']);
	});

	it('holds a stylesheet outside the styles folders to the shared families', () => {
		for (const filePath of ['src/lib/styles/paper.css', 'src/lib/motion/paper-moves.css']) {
			const sheet = (css: string): SourceFile => ({ path: filePath, text: css });
			expect(check({ stylesheets: [sheet('.zf-sr { margin: 0; }')] })).toEqual([]);
			expect(check({ stylesheets: [sheet('.placed .zf-btn { margin: 0; }')] })).toEqual([
				{ path: filePath, reason: "a zf- class outside the file's families", detail: 'zf-btn' }
			]);
		}
	});

	it('refuses a family listed by two stylesheet rows', () => {
		const messageCss = 'src/lib/styles/surfaces/message.css';
		const table: CompositionTable = {
			...TABLE,
			families: { ...TABLE.families, [messageCss]: ['zf-inline', 'zf-empty', 'zf-btn'] }
		};
		expect(check({ table })).toEqual([
			{
				path: messageCss,
				reason: 'a family listed by two stylesheet rows',
				detail: 'zf-btn, also in src/lib/styles/controls/button.css'
			}
		]);
	});

	it('refuses a stylesheet row for a family a component styles in its own <style>', () => {
		const menuCss = 'src/lib/styles/controls/menu.css';
		const table: CompositionTable = {
			...TABLE,
			families: { ...TABLE.families, [menuCss]: ['zf-menu', 'zf-icon'] },
			exceptions: EXCEPTIONS.filter((row) => row.file !== menuCss)
		};
		expect(check({ table, sources: [read('src/lib/icons/Icon.svelte')] })).toEqual([
			{
				path: menuCss,
				reason: 'a stylesheet row for a family a component styles itself',
				detail: 'zf-icon, styled in src/lib/icons/Icon.svelte'
			}
		]);
	});

	it.each([
		['the status form', '<Spinner kind="status" label="x" />'],
		['the active status form', '<Spinner kind="status" label="x" active />'],
		['no kind', '<Spinner />'],
		['the glyph as an expression', "<Spinner kind={'glyph'} />"],
		['a spread beside the glyph', '<Spinner kind="glyph" {...{ kind: \'status\' }} />'],
		['the glyph by a binding', '<Spinner bind:kind />']
	] as const)('refuses Button holding Spinner by %s', (_label, tag) => {
		const button = realWith('src/lib/controls/Button.svelte', '<Spinner kind="glyph" />', tag);
		expect(check({ sources: [button] })).toEqual([
			{ path: button.path, reason: 'a Spinner rendered other than as a glyph', detail: '<Spinner>' }
		]);
	});

	it('refuses Spinner held under another name, as a tag or as a value', () => {
		const renamed = realWith(
			'src/lib/controls/Button.svelte',
			"import Spinner from '../surfaces/Spinner.svelte';",
			"import Ring from '../surfaces/Spinner.svelte';"
		);
		const aliased = {
			path: renamed.path,
			text: renamed.text.replace('<Spinner kind="glyph" />', '<Ring kind="status" label="x" />')
		};
		expect(check({ sources: [aliased] })).toEqual([
			{ path: aliased.path, reason: 'a Spinner rendered other than as a glyph', detail: '<Ring>' }
		]);
		const handedOn = realWith(
			'src/lib/controls/Button.svelte',
			"import Spinner from '../surfaces/Spinner.svelte';",
			"import Spinner from '../surfaces/Spinner.svelte';\n\tconst Ring = Spinner;"
		);
		expect(check({ sources: [handedOn] })).toEqual([
			{ path: handedOn.path, reason: 'a Spinner rendered other than as a glyph', detail: 'Spinner' }
		]);
	});

	it('leaves a stories file the status Spinner as a tag', () => {
		const stories = realWith(
			'src/stories/surfaces/Spinner.stories.svelte',
			'<Spinner kind="glyph" />',
			'<Spinner kind="status" label="x" active />'
		);
		expect(check({ sources: [stories] })).toEqual([]);
	});

	it('refuses the status Spinner in an internal piece, which is simple', () => {
		const [internal] = TABLE.internal;
		expect(internal).toBeDefined();
		if (internal === undefined) return;
		const piece = read(internal.file);
		const script = "\timport Spinner from '../surfaces/Spinner.svelte';";
		const planted = {
			path: piece.path,
			text: piece.text
				.replace(/<script([^>]*)>/, `<script$1>\n${script}`)
				.concat('\n<Spinner kind="status" label="x" active />\n')
		};
		expect(planted.text).not.toBe(piece.text);
		expect(check({ sources: [planted] })).toContainEqual({
			path: piece.path,
			reason: 'a Spinner rendered other than as a glyph',
			detail: '<Spinner>'
		});
	});

	it.each([
		[
			'a prefixed static svelte:element in Card',
			'src/lib/surfaces/Card.svelte',
			'<svg><svelte:element this={\'svg:image\'} href="/a.png" /></svg>'
		],
		[
			'a prefixed tag in Card',
			'src/lib/surfaces/Card.svelte',
			'<svg><svg:image href="/a.png" /></svg>'
		],
		[
			'a prefixed static link in Card',
			'src/lib/surfaces/Card.svelte',
			'<svg><svelte:element this="svg:a" href="/" /></svg>'
		]
	] as const)('refuses %s by the name after its prefix', (_label, filePath, markup) => {
		const card = realWith(
			filePath,
			'<svelte:element this={element}',
			`${markup}\n<svelte:element this={element}`
		);
		expect(reasons(check({ sources: [card] }))).toEqual([
			'an owned element in a file not listed for it'
		]);
	});

	it.each([
		['Avatar by name', "\timport { Avatar } from 'bits-ui';", '<Avatar.Image src="/a.png" />'],
		['Avatar by alias', "\timport { Avatar as A } from 'bits-ui';", '<A.Image src="/a.png" />'],
		[
			'Button.Root by name',
			"\timport { Button } from 'bits-ui';",
			'<Button.Root href="/">x</Button.Root>'
		],
		[
			'Button.Root by alias',
			"\timport { Button as B } from 'bits-ui';",
			'<B.Root href="/">x</B.Root>'
		]
	] as const)('refuses %s', (_label, script, markup) => {
		const file = svelteFile('src/lib/controls/Toggle.svelte', script, markup);
		const found = reasons(check({ sources: [file] }));
		expect(found).toContain('a bits-ui name not listed for the file');
		expect(found).toContain('a bits-ui link or image part');
	});

	it.each([
		['by a member in script', 'const Image = Toolbar.Link;', '<Image />'],
		['by a computed member', "const Image = Toolbar['Link'];", '<Image />'],
		['in a type', 'type Linked = typeof Toolbar.Link;', '<span></span>'],
		['as a component', '', '<Toolbar.Link href="/">x</Toolbar.Link>']
	] as const)(
		'refuses a link part %s, even with its namespace listed',
		(_label, script, markup) => {
			const table: CompositionTable = {
				...TABLE,
				bitsUi: { ...TABLE.bitsUi, 'src/lib/controls/Toggle.svelte': ['Switch', 'Toolbar'] }
			};
			const file = svelteFile(
				'src/lib/controls/Toggle.svelte',
				`\timport { Switch, Toolbar } from 'bits-ui';\n\t${script}\n\tconst thumb = Switch.Thumb;`,
				`${markup}\n<button class="zf-toggle">{thumb}</button>`
			);
			expect(reasons(check({ table, sources: [file] }))).toEqual(['a bits-ui link or image part']);
		}
	);

	it.each([
		['aliased whole', 'const T = Tooltip;'],
		['read by a computed name', 'const part = Tooltip[String(1)];'],
		['destructured', 'const { Root } = Tooltip;'],
		['exported whole', 'export { Tooltip };']
	] as const)('refuses a bits-ui namespace %s', (_label, line) => {
		const tooltip = realWith(
			'src/lib/controls/Tooltip.svelte',
			"import { Tooltip } from 'bits-ui';",
			`import { Tooltip } from 'bits-ui';\n\t${line}`
		);
		expect(reasons(check({ sources: [tooltip] }))).toEqual([
			'a bits-ui namespace used other than by its parts'
		]);
	});

	it.each([
		['a namespace import', "import * as Bits from 'bits-ui';", ['the whole of bits-ui imported']],
		['a default import', "import Bits from 'bits-ui';", ['a default import of bits-ui']],
		[
			'a deep path',
			"import Image from 'bits-ui/dist/bits/avatar/components/avatar-image.svelte';",
			['a deep bits-ui path']
		],
		[
			'a path through node_modules',
			"import { Avatar } from '../../node_modules/bits-ui';",
			['a deep bits-ui path']
		],
		[
			'a re-export',
			"export { Avatar } from 'bits-ui';",
			['a bits-ui name not listed for the file']
		],
		['a re-export of everything', "export * from 'bits-ui';", ['the whole of bits-ui imported']],
		['a dynamic import', "const bits = import('bits-ui');", ['the whole of bits-ui imported']],
		[
			'a computed import',
			"const name = 'bits-ui';\n\tconst bits = import(name);",
			['a computed import']
		],
		['a type query', "type Bits = typeof import('bits-ui');", ['the whole of bits-ui imported']],
		[
			'an unlisted name',
			"import { Dialog } from 'bits-ui';",
			['a bits-ui name not listed for the file']
		]
	] as const)('refuses %s of bits-ui in a library file', (_label, line, expected) => {
		const tooltip = realWith(
			'src/lib/controls/Tooltip.svelte',
			"import { Tooltip } from 'bits-ui';",
			`import { Tooltip } from 'bits-ui';\n\t${line}`
		);
		expect(reasons(check({ sources: [tooltip] }))).toEqual(expected);
	});

	it.each([
		[
			'a namespace import',
			"import * as Bits from 'bits-ui';\nexport const b = Bits;",
			['the whole of bits-ui imported']
		],
		[
			'a named import',
			"import { Switch } from 'bits-ui';\nexport const s = Switch;",
			['a bits-ui name not listed for the file']
		],
		['a deep path', "export { default } from 'bits-ui/dist/index.js';", ['a deep bits-ui path']],
		['an import-equals', "import Bits = require('bits-ui');", ['the whole of bits-ui imported']]
	] as const)('refuses %s of bits-ui in a .ts file', (_label, text, expected) => {
		expect(reasons(check({ sources: [{ path: 'src/lib/controls/bits.ts', text }] }))).toEqual(
			expected
		);
	});

	it('refuses a bits-ui import in a stories file, the gallery and a spec harness', () => {
		for (const filePath of [
			'src/stories/controls/Rogue.svelte',
			'src/gallery/Rogue.svelte',
			'src/checks/testing/Rogue.svelte'
		]) {
			const file = svelteFile(filePath, "\timport { Switch } from 'bits-ui';", '<Switch.Root />');
			expect(reasons(check({ sources: [file] }))).toEqual([
				'a bits-ui name not listed for the file'
			]);
		}
	});

	it('refuses a bits-ui row that lists Avatar or Button, or a name the file no longer imports', () => {
		const listing = (names: readonly string[]): CompositionTable => ({
			...TABLE,
			bitsUi: { ...TABLE.bitsUi, 'src/lib/controls/Toggle.svelte': names }
		});
		expect(reasons(check({ table: listing(['Switch', 'Avatar']) }))).toEqual([
			'a bits-ui row that lists Avatar or Button'
		]);
		expect(reasons(check({ table: listing(['Switch', 'Button']) }))).toEqual([
			'a bits-ui row that lists Avatar or Button'
		]);
		const toggle = read('src/lib/controls/Toggle.svelte');
		expect(check({ table: listing(['Switch', 'Tabs']), sources: [toggle] })).toEqual([
			{ path: toggle.path, reason: 'a bits-ui row the file does not use', detail: 'Tabs' }
		]);
	});

	it('refuses a file that does not parse', () => {
		const broken = {
			path: 'src/lib/controls/Toggle.svelte',
			text: '<script lang="ts">\n\timport {'
		};
		expect(reasons(check({ sources: [broken] }))).toEqual(['does not parse']);
	});
});
