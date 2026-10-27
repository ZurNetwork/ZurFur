import { describe, expect, it } from 'vitest';
import ts from 'typescript';
import { parse } from 'svelte/compiler';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const srcRoot = fileURLToPath(new URL('../', import.meta.url));
const entryPath = path.join(srcRoot, 'lib', 'index.ts');

/**
 * The packages the controls wrap. No type they declare may cross the public
 * API, by name or reached through another type.
 */
const wrappedPackages = /^(?:bits-ui|@internationalized\/[^/]+)(?:\/|$)/;

/**
 * Where a type may be declared and still cross the public API: ui's own
 * files, Svelte's types and TypeScript's own lib. Anything else in
 * node_modules, bits-ui's own dependencies included, is a wrapped library's
 * type leaking out.
 */
function isAllowedDeclarationFile(fileName: string): boolean {
	const posix = fileName.split(path.sep).join('/');
	const inNodeModules = posix.lastIndexOf('/node_modules/');
	if (inNodeModules === -1) return posix.startsWith(srcRoot.split(path.sep).join('/'));
	const packagePath = posix.slice(inNodeModules + '/node_modules/'.length);
	return packagePath.startsWith('svelte/') || packagePath.startsWith('typescript/lib/');
}

/** The shim that types every `.svelte` import as a plain component. */
const svelteShim =
	"declare module '*.svelte' {\n\timport type { Component } from 'svelte';\n\tconst component: Component<Record<string, unknown>>;\n\texport default component;\n}\n";

const shimPath = path.join(srcRoot, '__public-api-shim__.d.ts');

const compilerOptions: ts.CompilerOptions = {
	target: ts.ScriptTarget.ES2022,
	module: ts.ModuleKind.ESNext,
	moduleResolution: ts.ModuleResolutionKind.Bundler,
	strict: true,
	exactOptionalPropertyTypes: true,
	allowImportingTsExtensions: true,
	resolveJsonModule: true,
	noEmit: true,
	skipLibCheck: true,
	lib: ['lib.es2023.d.ts', 'lib.dom.d.ts', 'lib.dom.iterable.d.ts'],
	types: []
};

interface ExportedComponent {
	/** The name the entry exports it under. */
	readonly name: string;
	/** The `.svelte` file, absolute. */
	readonly file: string;
}

/** Every module specifier the entry imports or re-exports from. */
function entrySpecifiers(entryText: string): readonly string[] {
	return ts.preProcessFile(entryText, true, true).importedFiles.map((file) => file.fileName);
}

/** The components the entry re-exports, by the name it exports them under. */
function exportedComponents(entryText: string): readonly ExportedComponent[] {
	const source = ts.createSourceFile(entryPath, entryText, ts.ScriptTarget.ES2022, true);
	return source.statements.flatMap((statement) => {
		if (!ts.isExportDeclaration(statement)) return [];
		const specifier = statement.moduleSpecifier;
		if (specifier === undefined || !ts.isStringLiteral(specifier)) return [];
		if (!specifier.text.endsWith('.svelte')) return [];
		const file = path.resolve(path.dirname(entryPath), specifier.text);
		const clause = statement.exportClause;
		if (clause === undefined || !ts.isNamedExports(clause)) return [];
		return clause.elements.map((element) => ({ name: element.name.text, file }));
	});
}

/** One generic parameter of a component as a type alias: its constraint, or unknown. */
function genericAlias(parameter: string): string {
	const match = /^\s*([A-Za-z_$][\w$]*)(?:\s+extends\s+([^=]+?))?(?:\s*=\s*.+)?\s*$/s.exec(
		parameter
	);
	if (match === null) throw new Error(`a generic parameter this spec can't read: ${parameter}`);
	const [, name, constraint] = match;
	return `type ${name ?? ''} = ${constraint ?? 'unknown'};`;
}

/** The generics attribute's parameters, split at the commas outside brackets. */
function genericParameters(generics: string): readonly string[] {
	const parameters: string[] = [];
	let depth = 0;
	let current = '';
	for (const character of generics) {
		if ('<([{'.includes(character)) depth += 1;
		if ('>)]}'.includes(character)) depth -= 1;
		if (character === ',' && depth === 0) {
			parameters.push(current);
			current = '';
		} else {
			current += character;
		}
	}
	return current.trim() === '' ? parameters : [...parameters, current];
}

/** The text of an attribute whose value is plain text, or undefined. */
function attributeText(value: unknown): string | undefined {
	if (!Array.isArray(value)) return undefined;
	const parts: readonly unknown[] = value;
	const first = parts[0];
	if (typeof first !== 'object' || first === null || !('data' in first)) return undefined;
	return typeof first.data === 'string' ? first.data : undefined;
}

/**
 * A component's instance script as a TypeScript module, its generic
 * parameters declared as aliases of their constraints, so its props type
 * resolves the way the component's users see it.
 */
function componentModule(svelteText: string): string {
	const ast = parse(svelteText, { modern: true });
	// The compiler answers undefined, not the null its types promise, for a missing block.
	const instance = ast.instance ?? undefined;
	if (instance === undefined) return 'export {};\n';
	const program = instance.content;
	if (!('start' in program) || typeof program.start !== 'number') {
		throw new Error('a svelte script has no start');
	}
	if (!('end' in program) || typeof program.end !== 'number') {
		throw new Error('a svelte script has no end');
	}
	const generics = instance.attributes.find((attribute) => attribute.name === 'generics');
	const genericText = generics === undefined ? undefined : attributeText(generics.value);
	const aliases = genericText === undefined ? [] : genericParameters(genericText).map(genericAlias);
	return `${svelteText.slice(program.start, program.end)}\n${aliases.join('\n')}\nexport {};\n`;
}

/** The path a component's script is read at: beside it, so its relative imports resolve. */
const moduleOf = (svelteFile: string): string => `${svelteFile}.__props__.ts`;

/** The type annotation of a `let { … }: Props = $props()` declaration, or undefined. */
function propsTypeNode(source: ts.SourceFile): ts.TypeNode | undefined {
	let found: ts.TypeNode | undefined;
	const visit = (node: ts.Node): void => {
		if (found !== undefined) return;
		const isPropsCall =
			ts.isVariableDeclaration(node) &&
			node.initializer !== undefined &&
			ts.isCallExpression(node.initializer) &&
			ts.isIdentifier(node.initializer.expression) &&
			node.initializer.expression.text === '$props';
		if (isPropsCall && ts.isVariableDeclaration(node)) {
			found = node.type;
			return;
		}
		ts.forEachChild(node, visit);
	};
	visit(source);
	return found;
}

function hasProps(source: ts.SourceFile): boolean {
	return /\$props\s*\(/.test(source.text);
}

/** One place a wrapped type crosses the public API. */
interface Finding {
	/** The export it crosses through, then the path to the type. */
	readonly trail: string;
	/** What was found: the declaring file relative to node_modules, or why the type can't be read. */
	readonly what: string;
}

/** The files `type` is declared in, through its own symbol and its alias. */
function declarationFiles(type: ts.Type): readonly string[] {
	const symbols = [type.aliasSymbol, type.getSymbol()].filter(
		(symbol): symbol is ts.Symbol => symbol !== undefined
	);
	return symbols.flatMap((symbol) =>
		(symbol.declarations ?? []).map((declaration) => declaration.getSourceFile().fileName)
	);
}

function isTypeReference(type: ts.Type): type is ts.TypeReference {
	return (
		(type.flags & ts.TypeFlags.Object) !== 0 &&
		((type as ts.ObjectType).objectFlags & ts.ObjectFlags.Reference) !== 0
	);
}

/**
 * Walks `type` and everything it reaches, recording each declaration in a
 * file the public API may not name and each type it can't read. It descends
 * into the members of ui's own and anonymous types; another library's type
 * is checked by its declaration and its type arguments only, since none it
 * declares can reach a wrapped package.
 */
function walkType(
	checker: ts.TypeChecker,
	type: ts.Type,
	trail: string,
	seen: Set<ts.Type>,
	findings: Finding[]
): void {
	if (seen.has(type)) return;
	seen.add(type);
	if ((type.flags & ts.TypeFlags.Any) !== 0) {
		findings.push({ trail, what: 'a type that does not resolve' });
		return;
	}
	const files = declarationFiles(type);
	for (const file of files) {
		if (!isAllowedDeclarationFile(file)) {
			const posix = file.split(path.sep).join('/');
			findings.push({ trail, what: posix.slice(posix.lastIndexOf('/node_modules/') + 1) });
		}
	}
	const next = (inner: ts.Type, step: string): void => {
		walkType(checker, inner, `${trail}${step}`, seen, findings);
	};
	for (const argument of type.aliasTypeArguments ?? []) next(argument, '<…>');
	if (isTypeReference(type)) {
		for (const argument of checker.getTypeArguments(type)) next(argument, '<…>');
	}
	if (type.isUnionOrIntersection()) {
		for (const member of type.types) next(member, ' | …');
		return;
	}
	const isOwn = files.every((file) => file.startsWith(srcRoot) && !file.endsWith('.d.ts'));
	if (!isOwn) return;
	for (const property of checker.getPropertiesOfType(type)) {
		next(checker.getTypeOfSymbol(property), `.${property.name}`);
	}
	const signatures = [
		...checker.getSignaturesOfType(type, ts.SignatureKind.Call),
		...checker.getSignaturesOfType(type, ts.SignatureKind.Construct)
	];
	for (const signature of signatures) {
		for (const parameter of signature.getParameters()) {
			next(checker.getTypeOfSymbol(parameter), `(${parameter.name})`);
		}
		next(signature.getReturnType(), '()');
	}
	for (const info of checker.getIndexInfosOfType(type)) next(info.type, '[…]');
}

/**
 * What crosses ui's public API from a wrapped package: each component's
 * props type and each other export's type, walked through everything they
 * reach. `overrides` replaces files' text by absolute path, the entry's and
 * the components' included, so planted text can stand in for real files.
 */
function publicApiFindings(overrides: ReadonlyMap<string, string> = new Map()): readonly Finding[] {
	const read = (file: string): string => overrides.get(file) ?? readFileSync(file, 'utf8');
	const entryText = read(entryPath);
	const components = exportedComponents(entryText);
	const virtual = new Map<string, string>([
		[entryPath, entryText],
		[shimPath, svelteShim],
		...components.map(({ file }) => [moduleOf(file), componentModule(read(file))] as const)
	]);
	const host = ts.createCompilerHost(compilerOptions, true);
	const baseGetSourceFile = host.getSourceFile.bind(host);
	const baseFileExists = host.fileExists.bind(host);
	const baseReadFile = host.readFile.bind(host);
	host.getSourceFile = (fileName, languageVersion, onError, shouldCreate) => {
		const text = virtual.get(fileName) ?? overrides.get(fileName);
		return text === undefined
			? baseGetSourceFile(fileName, languageVersion, onError, shouldCreate)
			: ts.createSourceFile(fileName, text, languageVersion, true);
	};
	host.fileExists = (fileName) =>
		virtual.has(fileName) || overrides.has(fileName) || baseFileExists(fileName);
	host.readFile = (fileName) =>
		virtual.get(fileName) ?? overrides.get(fileName) ?? baseReadFile(fileName);
	const program = ts.createProgram([...virtual.keys()], compilerOptions, host);
	const checker = program.getTypeChecker();
	const findings: Finding[] = [];
	const seen = new Set<ts.Type>();

	const wrappedSpecifiers = entrySpecifiers(entryText).filter((specifier) =>
		wrappedPackages.test(specifier)
	);
	for (const specifier of wrappedSpecifiers) {
		findings.push({ trail: 'src/lib/index.ts', what: `a re-export from ${specifier}` });
	}

	for (const { name, file } of components) {
		const source = program.getSourceFile(moduleOf(file));
		if (source === undefined) throw new Error(`no script for ${file}`);
		const node = propsTypeNode(source);
		if (node === undefined) {
			if (hasProps(source)) findings.push({ trail: name, what: 'props with no type annotation' });
			continue;
		}
		walkType(checker, checker.getTypeFromTypeNode(node), `${name} props`, seen, findings);
	}

	const entry = program.getSourceFile(entryPath);
	const entrySymbol = entry === undefined ? undefined : checker.getSymbolAtLocation(entry);
	if (entrySymbol === undefined) throw new Error('the entry has no module symbol');
	const componentNames = new Set(components.map((component) => component.name));
	for (const exported of checker.getExportsOfModule(entrySymbol)) {
		if (componentNames.has(exported.name)) continue;
		const target =
			(exported.flags & ts.SymbolFlags.Alias) !== 0 ? checker.getAliasedSymbol(exported) : exported;
		const isType = (target.flags & ts.SymbolFlags.Type) !== 0;
		const isValue = (target.flags & ts.SymbolFlags.Value) !== 0;
		if (isType)
			walkType(checker, checker.getDeclaredTypeOfSymbol(target), exported.name, seen, findings);
		if (isValue) walkType(checker, checker.getTypeOfSymbol(target), exported.name, seen, findings);
		if (!isType && !isValue)
			findings.push({ trail: exported.name, what: 'an export that does not resolve' });
	}
	return findings;
}

/** A component file planted beside the real ones, never written to disk. */
const plantedComponent = path.join(srcRoot, 'lib', 'controls', 'Planted.svelte');

/** The real entry with one more line, naming the planted component. */
function entryWith(line: string): string {
	return `${readFileSync(entryPath, 'utf8')}${line}\n`;
}

const exportsPlanted = entryWith("export { default as Planted } from './controls/Planted.svelte';");

/** A planted component whose script is `script`, generic over `generics` when given. */
function planted(script: string, generics?: string): ReadonlyMap<string, string> {
	const attributes = generics === undefined ? 'lang="ts"' : `lang="ts" generics="${generics}"`;
	return new Map([
		[entryPath, exportsPlanted],
		[plantedComponent, `<script ${attributes}>\n${script}\n</script>\n\n<p>x</p>\n`]
	]);
}

const whats = (findings: readonly Finding[]): readonly string[] =>
	findings.map((finding) => finding.what);

describe('the public API', { timeout: 120_000 }, () => {
	it('lets no wrapped type cross it in the real tree', () => {
		expect(publicApiFindings()).toEqual([]);
	});

	it('reads the components the real entry exports', () => {
		const names = exportedComponents(readFileSync(entryPath, 'utf8')).map(({ name }) => name);
		expect(names).toContain('Icon');
	});

	it.each([
		[
			'a DateValue in the props',
			"\timport type { DateValue } from '@internationalized/date';\n\tinterface Props {\n\t\treadonly value: DateValue;\n\t}\n\tlet { value }: Props = $props();"
		],
		[
			'a CalendarDate reached through a local type',
			"\timport type { CalendarDate } from '@internationalized/date';\n\ttype Day = { readonly at: CalendarDate };\n\tinterface Props {\n\t\treadonly days: readonly Day[];\n\t}\n\tlet { days }: Props = $props();"
		],
		[
			'a DateValue handed to a callback',
			"\timport type { DateValue } from '@internationalized/date';\n\tinterface Props {\n\t\treadonly onChange: (value: DateValue) => void;\n\t}\n\tlet { onChange }: Props = $props();"
		],
		[
			'a DateValue inside a snippet',
			"\timport type { Snippet } from 'svelte';\n\timport type { DateValue } from '@internationalized/date';\n\tinterface Props {\n\t\treadonly day: Snippet<[DateValue]>;\n\t}\n\tlet { day }: Props = $props();"
		],
		[
			'a bits-ui props type',
			"\timport type { SwitchRootProps } from 'bits-ui';\n\tinterface Props {\n\t\treadonly root: SwitchRootProps;\n\t}\n\tlet { root }: Props = $props();"
		],
		[
			'a bits-ui type as a generic constraint',
			"\timport type { SwitchRootProps } from 'bits-ui';\n\tinterface Props<T> {\n\t\treadonly root: T;\n\t}\n\tlet { root }: Props<Root> = $props();",
			'Root extends SwitchRootProps'
		]
	] as const)('refuses %s', (...[, script, generics]: readonly [string, string, string?]) => {
		const found = whats(publicApiFindings(planted(script, generics)));
		expect(found.some((what) => /node_modules\/(?:bits-ui|@internationalized)\//.test(what))).toBe(
			true
		);
	});

	it('refuses props with no type annotation', () => {
		const found = whats(publicApiFindings(planted('\tlet props = $props();')));
		expect(found).toContain('props with no type annotation');
	});

	it.each([
		["export { Switch } from 'bits-ui'", "export { Switch } from 'bits-ui';"],
		[
			"export type { DateValue } from '@internationalized/date'",
			"export type { DateValue } from '@internationalized/date';"
		]
	])('refuses %s in the entry', (_label, line) => {
		const found = whats(publicApiFindings(new Map([[entryPath, entryWith(line)]])));
		expect(found.some((what) => what.startsWith('a re-export from '))).toBe(true);
		expect(found.some((what) => /node_modules\/(?:bits-ui|@internationalized)\//.test(what))).toBe(
			true
		);
	});

	it('allows props built from ui and Svelte types alone', () => {
		const script =
			"\timport type { Snippet } from 'svelte';\n\timport type { IconName } from '../icons/icons';\n\ttype Option<Id extends string> = { readonly id: Id; readonly label: string; readonly icon?: IconName };\n\tinterface Props {\n\t\treadonly label: string;\n\t\treadonly options: readonly Option<Value>[];\n\t\treadonly value: Value;\n\t\treadonly panel: Snippet<[Value]>;\n\t\treadonly onChange: (id: Value) => void;\n\t\treadonly testid?: string | undefined;\n\t}\n\tlet { label, options, value, panel, onChange, testid }: Props = $props();";
		expect(publicApiFindings(planted(script, 'Value extends string'))).toEqual([]);
	});
});
