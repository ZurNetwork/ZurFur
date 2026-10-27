/**
 * The readers the story checks and the composition check share: the
 * components the public entry exports, and what a stories file's
 * `defineMeta` names (its component, read back to its name in `@zurfur/ui`,
 * and its tags). Input never makes them throw: a file that does not parse
 * reads as naming nothing.
 */
import ts from 'typescript';
import { parse } from 'svelte/compiler';
import path from 'node:path';
import type { SourceFile } from './parse';

/** The package's public entry, relative to the package. */
export const entryPath = 'src/lib/index.ts';

/** A component the public entry exports as `export { default as Name } from '….svelte'`. */
export interface ExportedComponent {
	readonly name: string;
	/** The component's file, relative to the package. */
	readonly file: string;
}

/** Every component the public entry exports by default from a `.svelte` file. */
export function exportedComponents(entry: string): readonly ExportedComponent[] {
	const source = ts.createSourceFile(entryPath, entry, ts.ScriptTarget.Latest, true);
	return source.statements.flatMap((statement) => {
		if (!ts.isExportDeclaration(statement) || statement.isTypeOnly) return [];
		const specifier = statement.moduleSpecifier;
		const clause = statement.exportClause;
		if (specifier === undefined || !ts.isStringLiteral(specifier)) return [];
		if (!specifier.text.endsWith('.svelte') || clause === undefined) return [];
		if (!ts.isNamedExports(clause)) return [];
		const file = path.posix.join(path.posix.dirname(entryPath), specifier.text);
		return clause.elements.flatMap((element) =>
			element.propertyName?.text === 'default' && !element.isTypeOnly
				? [{ name: element.name.text, file }]
				: []
		);
	});
}

type Fields = Readonly<Record<string, unknown>>;

function isFields(value: unknown): value is Fields {
	return Object(value) === value;
}

/** Every object in a parsed tree, each once. */
function nodesOf(root: unknown): readonly Fields[] {
	const seen = new Set<object>();
	const found: Fields[] = [];
	const visit = (value: unknown): void => {
		if (Array.isArray(value)) {
			for (const item of value) visit(item);
			return;
		}
		if (!isFields(value) || seen.has(value)) return;
		seen.add(value);
		found.push(value);
		for (const child of Object.values(value)) visit(child);
	};
	visit(root);
	return found;
}

/** An identifier's name, or a string literal's text. */
function nameOf(node: unknown): string | undefined {
	if (!isFields(node)) return undefined;
	if (node.type === 'Identifier') return typeof node.name === 'string' ? node.name : undefined;
	if (node.type === 'Literal') return typeof node.value === 'string' ? node.value : undefined;
	return undefined;
}

function moduleScript(text: string): unknown {
	try {
		const root: unknown = parse(text, { modern: true });
		return isFields(root) && isFields(root.module) ? root.module.content : undefined;
	} catch {
		return undefined;
	}
}

/** What a stories file's tags hold: no `tags` key, a list of plain strings, or something else. */
export type StoryTags =
	| { readonly kind: 'absent' }
	| { readonly kind: 'list'; readonly tags: readonly string[] }
	| { readonly kind: 'unread' };

/** What a stories file's one `defineMeta({ … })` names. */
export interface StoryMeta {
	/** The component, by its name in `@zurfur/ui`; undefined when it names none, or one from elsewhere. */
	readonly component: string | undefined;
	readonly tags: StoryTags;
}

/** The value of a non-computed key of an object literal, each time it is written. */
function propertyValues(argument: unknown, key: string): readonly unknown[] {
	const properties: readonly unknown[] =
		isFields(argument) && Array.isArray(argument.properties) ? argument.properties : [];
	return properties.flatMap((property) => {
		const matches =
			isFields(property) &&
			property.type === 'Property' &&
			property.computed !== true &&
			nameOf(property.key) === key;
		return matches ? [property.value] : [];
	});
}

/** An array literal of plain strings, read as its strings; anything else is unread. */
function tagsOf(values: readonly unknown[]): StoryTags {
	if (values.length === 0) return { kind: 'absent' };
	const [value] = values;
	if (values.length > 1 || !isFields(value) || value.type !== 'ArrayExpression') {
		return { kind: 'unread' };
	}
	const elements: readonly unknown[] = Array.isArray(value.elements) ? value.elements : [];
	const tags = elements.flatMap((element) => {
		const isString = isFields(element) && element.type === 'Literal';
		return isString && typeof element.value === 'string' ? [element.value] : [];
	});
	return tags.length === elements.length ? { kind: 'list', tags } : { kind: 'unread' };
}

/**
 * What a stories file's `defineMeta` names. With no `defineMeta`, or more
 * than one, it names no component and its tags are unread.
 */
export function storyMeta(file: SourceFile): StoryMeta {
	const nodes = nodesOf(moduleScript(file.text));
	const fromPackage = new Map<string, string>();
	for (const node of nodes) {
		if (node.type !== 'ImportDeclaration' || nameOf(node.source) !== '@zurfur/ui') continue;
		const specifiers = Array.isArray(node.specifiers) ? node.specifiers : [];
		for (const specifier of specifiers) {
			if (!isFields(specifier) || specifier.type !== 'ImportSpecifier') continue;
			const local = nameOf(specifier.local);
			const imported = nameOf(specifier.imported);
			if (local !== undefined && imported !== undefined) fromPackage.set(local, imported);
		}
	}
	const metas = nodes.filter(
		(node) => node.type === 'CallExpression' && nameOf(node.callee) === 'defineMeta'
	);
	const [meta] = metas;
	if (meta === undefined || metas.length > 1) {
		return { component: undefined, tags: { kind: 'unread' } };
	}
	const argument: unknown = Array.isArray(meta.arguments) ? meta.arguments[0] : undefined;
	const named = propertyValues(argument, 'component').flatMap((value) => {
		const local = nameOf(value);
		const imported = local === undefined ? undefined : fromPackage.get(local);
		return imported === undefined ? [] : [imported];
	});
	return {
		component: named.length === 1 ? named[0] : undefined,
		tags: tagsOf(propertyValues(argument, 'tags'))
	};
}

/**
 * The component a stories file's `defineMeta({ component })` names, read back
 * to its name in `@zurfur/ui`; undefined when it names none, or one imported
 * from anywhere else.
 */
export function storiedComponent(file: SourceFile): string | undefined {
	return storyMeta(file).component;
}
