/**
 * The built-output check: what the gallery build wrote never fetches from
 * another origin. It walks a build folder without following links and
 * refuses, in every `.css` file, a `url()`, `@import` or string that is
 * remote (`http:`, `https:`, `//`), and in every `.html` file a URL-bearing
 * attribute (`href`, `src`, `srcset`, `action`, `formaction`, `poster`, …)
 * that names a scheme or is protocol-relative, along with remote CSS in its
 * `<style>` elements and `style` attributes, and any `http-equiv` attribute
 * on any element but Kit's own `content-security-policy` pragma (its value
 * trimmed, in any letter case; one with no value, or any other value, is
 * refused). A link inside the folder is refused too, since a server would
 * follow it. Every text file, JavaScript, JSON, source maps and Markdown
 * included, is refused if it holds the text `ZURFUR_` or `DATABASE_URL`: an
 * environment variable built into the output. Only fonts and images, by
 * extension, are not read as text.
 *
 * In Storybook mode (`--storybook`, over `storybook-static`), a remote
 * `<a href>` is allowed under `https://storybook.js.org/docs/` alone, for the
 * links on Storybook's own error screens; every other rule is the same.
 *
 * Run by Node from `frontend/ui`: `node src/checks/built-output.ts build`, or
 * `node src/checks/built-output.ts --storybook storybook-static`.
 * Exit 0 clean, 1 findings, 2 unreadable input or bad usage.
 */
import { lstatSync, readdirSync, readFileSync, realpathSync, type Dirent } from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import {
	cssValueParts,
	parseSource,
	type HttpEquivAttribute,
	type ParsedSource,
	type Place,
	type UrlAttribute
} from './scan/parse.ts';
import { describeHttpEquiv, isRemote, namesAnotherPlace, urlsOf } from './scan/network.ts';

export type BuiltRule =
	| 'remote-url'
	| 'remote-string'
	| 'remote-import'
	| 'scheme-attribute'
	| 'http-equiv'
	| 'environment-name'
	| 'link'
	| 'unparsed';

/** Which build the check reads: the Kit gallery's, or Storybook's static build. */
export type BuildMode = 'kit' | 'storybook';

export interface BuiltFinding {
	readonly rule: BuiltRule;
	readonly found: string;
	readonly place: Place;
}

/** What one run found: findings (exit 1) and inputs it could not read (exit 2). */
export interface BuiltReport {
	readonly findings: readonly BuiltFinding[];
	readonly errors: readonly string[];
}

type BuiltKind = 'css' | 'html' | 'text' | 'binary';

/**
 * The extensions, in lower case, of the binary files the check does not read
 * as text: fonts and images. Every other file is read as text.
 */
const binaryExtensions: ReadonlySet<string> = new Set([
	'.woff2',
	'.woff',
	'.ttf',
	'.otf',
	'.eot',
	'.png',
	'.jpg',
	'.jpeg',
	'.gif',
	'.webp',
	'.avif',
	'.ico'
]);

/** The kind of a built file, by its extension in any letter case. */
function builtKind(name: string): BuiltKind {
	const lower = name.toLowerCase();
	if (lower.endsWith('.css')) return 'css';
	if (lower.endsWith('.html')) return 'html';
	const dot = lower.lastIndexOf('.');
	return dot !== -1 && binaryExtensions.has(lower.slice(dot)) ? 'binary' : 'text';
}

interface BuiltFile {
	readonly path: string;
	readonly kind: BuiltKind;
}

interface Walk {
	readonly files: BuiltFile[];
	readonly links: string[];
	readonly errors: string[];
}

function entriesOf(folder: string): readonly Dirent[] | Error {
	try {
		return readdirSync(folder, { withFileTypes: true });
	} catch (error) {
		return error instanceof Error ? error : new Error('unreadable');
	}
}

/** Every file under `folder`, and every link, read without following links. */
function walk(folder: string, found: Walk): void {
	const entries = entriesOf(folder);
	if (entries instanceof Error) {
		found.errors.push(`${folder}: ${entries.message}`);
		return;
	}
	for (const entry of entries) {
		const entryPath = path.join(folder, entry.name);
		if (entry.isSymbolicLink()) {
			found.links.push(entryPath);
		} else if (entry.isDirectory()) {
			walk(entryPath, found);
		} else if (entry.isFile()) {
			found.files.push({ path: entryPath, kind: builtKind(entry.name) });
		} else {
			found.errors.push(`${entryPath}: neither a plain file nor a folder`);
		}
	}
}

function reasonOf(error: unknown): string {
	return error instanceof Error ? error.message : 'unreadable';
}

const placeOf = (file: string): Place => ({ path: file, line: 1, column: 1 });

/** Remote CSS: a `url()`, a string or an `@import` that reaches another origin. */
function cssFindings(source: ParsedSource): readonly BuiltFinding[] {
	const inDeclarations = source.declarations.flatMap((declaration) => {
		const { urls, strings } = cssValueParts(declaration.value);
		const remoteUrls = urls.filter(isRemote).map((url): BuiltFinding => ({
			rule: 'remote-url',
			found: `${declaration.property}: url(${url})`,
			place: declaration.place
		}));
		const remoteStrings = strings.filter(isRemote).map((text): BuiltFinding => ({
			rule: 'remote-string',
			found: `${declaration.property}: "${text}"`,
			place: declaration.place
		}));
		return [...remoteUrls, ...remoteStrings];
	});
	const inImports = source.imports.flatMap((cssImport) => {
		const { urls, strings } = cssValueParts(cssImport.prelude);
		return [...urls, ...strings].filter(isRemote).map((url): BuiltFinding => ({
			rule: 'remote-import',
			found: `@import ${url}`,
			place: cssImport.place
		}));
	});
	return [...inDeclarations, ...inImports];
}

/** The one remote place a Storybook build may link to: Storybook's documentation. */
const storybookDocs = 'https://storybook.js.org/docs/';

/**
 * True for an `<a href>` into Storybook's documentation, as written and as a
 * browser reads it, the links Storybook's error screens carry.
 */
function isStorybookDocsLink(attribute: UrlAttribute, url: string): boolean {
	if (attribute.element.toLowerCase() !== 'a' || attribute.attribute !== 'href') return false;
	if (!url.startsWith(storybookDocs) || !URL.canParse(url)) return false;
	const read = new URL(url);
	return read.origin === 'https://storybook.js.org' && read.pathname.startsWith('/docs/');
}

/**
 * A URL-bearing attribute that names a scheme, `data:` included, or another
 * origin; in Storybook mode, a link into Storybook's documentation aside.
 */
function attributeFindings(source: ParsedSource, mode: BuildMode): readonly BuiltFinding[] {
	return source.urlAttributes.flatMap((attribute) =>
		urlsOf(attribute)
			.filter(namesAnotherPlace)
			.filter((url) => mode !== 'storybook' || !isStorybookDocsLink(attribute, url))
			.map((url): BuiltFinding => ({
				rule: 'scheme-attribute',
				found: `<${attribute.element} ${attribute.attribute}="${url.trim()}">`,
				place: attribute.place
			}))
	);
}

/** The one pragma a built page may carry: the policy Kit writes when it is set to. */
const allowedPragma = 'content-security-policy';

/** True only for plain text that, trimmed, is the policy pragma in any ASCII letter case. */
function isAllowedPragma(attribute: HttpEquivAttribute): boolean {
	const { value } = attribute;
	switch (value.kind) {
		case 'absent':
		case 'expression':
			return false;
		case 'text':
			return asciiLowerCase(value.text.trim()) === allowedPragma;
	}
}

/** `text` with only the ASCII capitals lowered, as HTML compares keywords. */
function asciiLowerCase(text: string): string {
	return text.replace(/[A-Z]/g, (letter) => letter.toLowerCase());
}

/** Any `http-equiv` but the policy pragma. */
function httpEquivFindings(source: ParsedSource): readonly BuiltFinding[] {
	return source.httpEquivs
		.filter((attribute) => !isAllowedPragma(attribute))
		.map((attribute): BuiltFinding => ({
			rule: 'http-equiv',
			found: describeHttpEquiv(attribute),
			place: attribute.place
		}));
}

/** Environment variable names no built file may hold: the platform's own, and the database's. */
const environmentNames = ['ZURFUR_', 'DATABASE_URL'] as const;

/** Where `offset` falls in `text`, as a 1-based line and column. */
function placeAt(file: string, text: string, offset: number): Place {
	const before = text.slice(0, offset).split('\n');
	return { path: file, line: before.length, column: (before.at(-1) ?? '').length + 1 };
}

/** Every place a refused environment name appears in a built file's text. */
function environmentFindings(file: string, text: string): readonly BuiltFinding[] {
	return environmentNames.flatMap((name) => {
		const findings: BuiltFinding[] = [];
		let at = text.indexOf(name);
		while (at !== -1) {
			const found = text.slice(Math.max(0, at - 20), at + name.length + 20).replace(/\s+/g, ' ');
			findings.push({ rule: 'environment-name', found, place: placeAt(file, text, at) });
			at = text.indexOf(name, at + name.length);
		}
		return findings;
	});
}

/** The findings in one built file's text; `mode` defaults to the Kit gallery's build. */
export function builtFileFindings(
	file: string,
	text: string,
	mode: BuildMode = 'kit'
): readonly BuiltFinding[] {
	const kind = builtKind(file);
	const inText = environmentFindings(file, text);
	if (kind !== 'css' && kind !== 'html') return inText;
	const source = parseSource({ path: file, text });
	const unparsed = source.unparsed.map((failure): BuiltFinding => ({
		rule: 'unparsed',
		found: failure.reason,
		place: failure.place
	}));
	const inHtml =
		kind === 'html' ? [...attributeFindings(source, mode), ...httpEquivFindings(source)] : [];
	return [...unparsed, ...cssFindings(source), ...inHtml, ...inText];
}

/** Checks a build folder: every file in it but its fonts and images, and no link. */
export function checkBuild(folder: string, mode: BuildMode = 'kit'): BuiltReport {
	let stat;
	try {
		stat = lstatSync(folder);
	} catch (error) {
		return { findings: [], errors: [`${folder}: ${reasonOf(error)}`] };
	}
	if (!stat.isDirectory()) {
		return { findings: [], errors: [`${folder}: not a folder (a link is not followed)`] };
	}
	const found: Walk = { files: [], links: [], errors: [] };
	walk(folder, found);
	const errors = [...found.errors];
	if (errors.length === 0 && !found.files.some((file) => file.kind === 'html')) {
		errors.push(`${folder}: holds no .html file, so it is not a gallery build`);
	}
	const linkFindings = found.links.map((link): BuiltFinding => ({
		rule: 'link',
		found: 'a link the check does not follow, which a server would',
		place: placeOf(link)
	}));
	const fileFindings = found.files.flatMap((file) => {
		if (file.kind === 'binary') return [];
		let text;
		try {
			text = readFileSync(file.path, 'utf8');
		} catch (error) {
			errors.push(`${file.path}: ${reasonOf(error)}`);
			return [];
		}
		return builtFileFindings(file.path, text, mode);
	});
	return { findings: [...linkFindings, ...fileFindings], errors };
}

/** Where the check writes: a stream's `write`, so a spec can collect the text. */
interface Output {
	write(text: string): unknown;
}

const usage = 'usage: node src/checks/built-output.ts [--storybook] <build folder>';

/** The flag that reads the folder as Storybook's static build. */
const storybookFlag = '--storybook';

/** Formats one finding as the line the check prints. */
export function formatFinding(finding: BuiltFinding): string {
	const { path: file, line, column } = finding.place;
	return `${file}:${String(line)}:${String(column)}: [${finding.rule}] ${finding.found}`;
}

/** Runs the check as the CLI does and returns its exit code; it never throws. */
export function main(argv: readonly string[], out: Output, err: Output): number {
	try {
		const mode: BuildMode = argv[0] === storybookFlag ? 'storybook' : 'kit';
		const [folder, ...extra] = mode === 'storybook' ? argv.slice(1) : argv;
		if (folder === undefined || extra.length > 0 || folder.startsWith('--')) {
			err.write(`built-output: ${usage}\n`);
			return 2;
		}
		const { findings, errors } = checkBuild(folder, mode);
		for (const finding of findings) out.write(`${formatFinding(finding)}\n`);
		for (const error of errors) err.write(`built-output: ${error}\n`);
		if (errors.length > 0) return 2;
		if (findings.length > 0) {
			err.write(`built-output: ${String(findings.length)} finding(s)\n`);
			return 1;
		}
		return 0;
	} catch (error) {
		err.write(`built-output: internal error: ${reasonOf(error)}\n`);
		return 2;
	}
}

/** True when Node runs this file as its entry point, not when a spec imports it. */
function isEntryPoint(): boolean {
	const entry = process.argv[1];
	if (entry === undefined) return false;
	try {
		const self = realpathSync(fileURLToPath(import.meta.url));
		return pathToFileURL(realpathSync(entry)).href === pathToFileURL(self).href;
	} catch {
		return false;
	}
}

if (isEntryPoint()) process.exitCode = main(process.argv.slice(2), process.stdout, process.stderr);
