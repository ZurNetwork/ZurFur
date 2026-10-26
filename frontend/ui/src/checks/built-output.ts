/**
 * The built-output check: what the gallery build wrote never fetches from
 * another origin. It walks a build folder without following links and
 * refuses, in every `.css` file, a `url()`, `@import` or string that is
 * remote (`http:`, `https:`, `//`), and in every `.html` file a URL-bearing
 * attribute (`href`, `src`, `srcset`, `action`, `formaction`, `poster`, …)
 * that names a scheme or is protocol-relative, along with remote CSS in its
 * `<style>` elements and `style` attributes. A link inside the folder is
 * refused too, since a server would follow it.
 *
 * Run by Node from `frontend/ui`: `node src/checks/built-output.ts build`.
 * Exit 0 clean, 1 findings, 2 unreadable input or bad usage.
 */
import { lstatSync, readdirSync, readFileSync, realpathSync, type Dirent } from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { cssValueParts, parseSource, type ParsedSource, type Place } from './scan/parse.ts';
import { isRemote, namesAnotherPlace, urlsOf } from './scan/network.ts';

export type BuiltRule =
	'remote-url' | 'remote-string' | 'remote-import' | 'scheme-attribute' | 'link' | 'unparsed';

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

type BuiltKind = 'css' | 'html';

/** The kind of a built file the check reads, by its extension in any letter case. */
function builtKind(name: string): BuiltKind | undefined {
	const lower = name.toLowerCase();
	if (lower.endsWith('.css')) return 'css';
	if (lower.endsWith('.html')) return 'html';
	return undefined;
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

/** Every `.css` and `.html` file under `folder`, and every link, read without following links. */
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
			const kind = builtKind(entry.name);
			if (kind !== undefined) found.files.push({ path: entryPath, kind });
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

/** A URL-bearing attribute that names a scheme, `data:` included, or another origin. */
function attributeFindings(source: ParsedSource): readonly BuiltFinding[] {
	return source.urlAttributes.flatMap((attribute) =>
		urlsOf(attribute)
			.filter(namesAnotherPlace)
			.map((url): BuiltFinding => ({
				rule: 'scheme-attribute',
				found: `<${attribute.element} ${attribute.attribute}="${url.trim()}">`,
				place: attribute.place
			}))
	);
}

/** The findings in one built file's text. */
export function builtFileFindings(file: string, text: string): readonly BuiltFinding[] {
	const source = parseSource({ path: file, text });
	const unparsed = source.unparsed.map((failure): BuiltFinding => ({
		rule: 'unparsed',
		found: failure.reason,
		place: failure.place
	}));
	const inHtml = builtKind(file) === 'html' ? attributeFindings(source) : [];
	return [...unparsed, ...cssFindings(source), ...inHtml];
}

/** Checks a build folder: every `.css` and `.html` file in it, and no link. */
export function checkBuild(folder: string): BuiltReport {
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
		let text;
		try {
			text = readFileSync(file.path, 'utf8');
		} catch (error) {
			errors.push(`${file.path}: ${reasonOf(error)}`);
			return [];
		}
		return builtFileFindings(file.path, text);
	});
	return { findings: [...linkFindings, ...fileFindings], errors };
}

/** Where the check writes: a stream's `write`, so a spec can collect the text. */
interface Output {
	write(text: string): unknown;
}

const usage = 'usage: node src/checks/built-output.ts <build folder>';

/** Formats one finding as the line the check prints. */
export function formatFinding(finding: BuiltFinding): string {
	const { path: file, line, column } = finding.place;
	return `${file}:${String(line)}:${String(column)}: [${finding.rule}] ${finding.found}`;
}

/** Runs the check as the CLI does and returns its exit code; it never throws. */
export function main(argv: readonly string[], out: Output, err: Output): number {
	try {
		const [folder, ...extra] = argv;
		if (folder === undefined || extra.length > 0) {
			err.write(`built-output: ${usage}\n`);
			return 2;
		}
		const { findings, errors } = checkBuild(folder);
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
