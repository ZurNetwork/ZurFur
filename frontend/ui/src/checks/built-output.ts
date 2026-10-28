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
 * Every `.svg` file is refused if it holds a `<script>`, an `on…` attribute,
 * a `<foreignObject>`, an `href` (with any prefix) that is not a `#fragment`,
 * a DOCTYPE or ENTITY declaration, an `<?xml-stylesheet>`, a `url(` that is
 * not `url(#…)`, or an `a`, `set`, `animate…`, `style`, `image`, `feImage`,
 * `iframe`, `embed`, `object`, `handler` or `listener` element. Under the
 * build's `fixtures/` folder only the fixture names may sit, each a plain
 * file, with no folder beside them, and each fixture is also read against
 * the fixtures' allow-list of elements and attributes (`fixtureSvgRefusals`).
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
	| 'svg'
	| 'fixture'
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

type BuiltKind = 'css' | 'html' | 'svg' | 'text' | 'binary';

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
	if (lower.endsWith('.svg')) return 'svg';
	const dot = lower.lastIndexOf('.');
	return dot !== -1 && binaryExtensions.has(lower.slice(dot)) ? 'binary' : 'text';
}

interface BuiltFile {
	readonly path: string;
	readonly kind: BuiltKind;
}

interface Walk {
	readonly files: BuiltFile[];
	readonly folders: string[];
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
			found.folders.push(entryPath);
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

// --- SVG files and the fixtures ---------------------------------------------

/**
 * The pictures the stories load by URL, by file name: the only files the
 * fixtures folder may hold, in `static/` and in a build.
 */
export const FIXTURE_NAMES: readonly string[] = ['avatar.svg'];

/** The fixtures folder's name, in `static/` and at a build's root. */
export const FIXTURE_FOLDER = 'fixtures';

/** Elements no SVG holds, with any prefix and in any letter case. */
const refusedSvgElements =
	/<\s*(?:[\w.-]+:)?(a|set|animate[\w-]*|style|image|feimage|iframe|embed|object|handler|listener)(?=[\s/>])/gi;

/** An `href` attribute with any prefix, and its value, quoted or not. */
const hrefAttribute =
	/(?:^|[\s"'/])((?:[^\s=<>"'/]*:)?href)\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s>]*))/gi;

/** An attribute named `on…`, in any letter case. */
const handlerAttribute = /(?:^|[\s"'/])(on[^\s=<>"'/]*)\s*=/gi;

/** Every `url(` and what follows it. */
const urlCall = /url\s*\(\s*(['"]?)\s*(.?)/gi;

/**
 * Why an SVG file could run script, load anything or navigate: what every
 * `.svg` under a build is refused for. Read on the text as written, so markup
 * the check can't parse is still seen.
 */
export function svgRefusals(text: string): readonly string[] {
	const fixed: readonly (readonly [RegExp, string])[] = [
		[/<\s*(?:[\w.-]+:)?script/i, 'a <script> element'],
		[/<\s*(?:[\w.-]+:)?foreignObject/i, 'a <foreignObject> element'],
		[/<!DOCTYPE/i, 'a DOCTYPE'],
		[/<!ENTITY/i, 'an ENTITY declaration'],
		[/<\?xml-stylesheet/i, 'an <?xml-stylesheet?> instruction']
	];
	const named = fixed.filter(([pattern]) => pattern.test(text)).map(([, reason]) => reason);
	const elements = [...text.matchAll(refusedSvgElements)].map(
		(match) => `a <${match[1] ?? ''}> element`
	);
	const handlers = [...text.matchAll(handlerAttribute)].map(
		(match) => `an ${match[1] ?? 'on…'} attribute`
	);
	const hrefs = [...text.matchAll(hrefAttribute)].flatMap((match) => {
		const value = match[2] ?? match[3] ?? match[4] ?? '';
		return value.startsWith('#')
			? []
			: [`an ${match[1] ?? 'href'} that is not a #fragment: ${value}`];
	});
	const urls = [...text.matchAll(urlCall)].flatMap((match) =>
		match[2] === '#' ? [] : [`a url() that is not url(#…): ${match[0]}`]
	);
	return [...named, ...elements, ...handlers, ...hrefs, ...urls];
}

/** The elements a fixture SVG may hold. */
const fixtureElements: ReadonlySet<string> = new Set([
	'svg',
	'g',
	'title',
	'circle',
	'ellipse',
	'rect',
	'path',
	'line',
	'polyline',
	'polygon',
	'defs',
	'linearGradient',
	'radialGradient',
	'stop',
	'clipPath'
]);

/** The attributes a fixture SVG may hold: geometry, presentation and ids. */
const fixtureAttributes: ReadonlySet<string> = new Set([
	'viewBox',
	'width',
	'height',
	'preserveAspectRatio',
	'x',
	'y',
	'x1',
	'y1',
	'x2',
	'y2',
	'cx',
	'cy',
	'r',
	'rx',
	'ry',
	'fx',
	'fy',
	'd',
	'points',
	'transform',
	'fill',
	'fill-opacity',
	'fill-rule',
	'stroke',
	'stroke-width',
	'stroke-linecap',
	'stroke-linejoin',
	'stroke-opacity',
	'stroke-dasharray',
	'stroke-dashoffset',
	'stroke-miterlimit',
	'opacity',
	'offset',
	'stop-color',
	'stop-opacity',
	'gradientUnits',
	'gradientTransform',
	'spreadMethod',
	'clip-path',
	'clip-rule',
	'clipPathUnits',
	'id'
]);

/** The one namespace a fixture names, on its root alone. */
const svgNamespace = 'http://www.w3.org/2000/svg';

const fixtureStartTag =
	/^<([A-Za-z][A-Za-z0-9]*)((?:\s+[A-Za-z][A-Za-z0-9:-]*\s*=\s*"[^"<]*")*)\s*(\/?)>/;
const fixtureEndTag = /^<\/([A-Za-z][A-Za-z0-9]*)\s*>/;
const fixtureAttribute = /\s+([A-Za-z][A-Za-z0-9:-]*)\s*=\s*"([^"<]*)"/g;

/** The attributes of one start tag, refused when off the list. */
function fixtureAttributeRefusals(name: string, attributes: string, isRoot: boolean): string[] {
	return [...attributes.matchAll(fixtureAttribute)].flatMap((match) => {
		const attribute = match[1] ?? '';
		const value = match[2] ?? '';
		if (attribute === 'xmlns') {
			return isRoot && value === svgNamespace ? [] : [`xmlns="${value}" on <${name}>`];
		}
		return fixtureAttributes.has(attribute) ? [] : [`the attribute ${attribute} on <${name}>`];
	});
}

/**
 * Why a fixture SVG holds more than plain shapes: read as XML with no entity,
 * comment, declaration or instruction, one `<svg>` root naming the SVG
 * namespace, every element and attribute on the fixtures' lists, attribute
 * values in double quotes, text only inside a `<title>`, and every element
 * closed. Anything it can't read is refused.
 */
function fixtureShapeRefusals(text: string): readonly string[] {
	const refusals: string[] = [];
	if (text.includes('&')) refusals.push('an entity or character reference');
	if (/<[!?]/.test(text)) refusals.push('a comment, declaration or instruction');
	const open: string[] = [];
	let roots = 0;
	let rest = text;
	while (rest.length > 0) {
		const next = rest.indexOf('<');
		const between = next === -1 ? rest : rest.slice(0, next);
		if (between.trim() !== '' && open.at(-1) !== 'title') {
			refusals.push(`text outside a <title>: ${between.trim().slice(0, 30)}`);
		}
		if (next === -1) break;
		rest = rest.slice(next);
		const end = fixtureEndTag.exec(rest);
		if (end !== null) {
			const name = end[1] ?? '';
			if (open.pop() !== name) refusals.push(`a </${name}> that closes nothing open`);
			rest = rest.slice(end[0].length);
			continue;
		}
		const start = fixtureStartTag.exec(rest);
		if (start === null) {
			refusals.push(`markup that does not read as a plain tag: ${rest.slice(0, 30)}`);
			break;
		}
		const [whole, name = '', attributes = '', selfClosing = ''] = start;
		const isRoot = open.length === 0;
		if (isRoot) roots += 1;
		if (isRoot && name !== 'svg') refusals.push(`a root <${name}> that is not <svg>`);
		if (!fixtureElements.has(name)) refusals.push(`the element <${name}>`);
		refusals.push(...fixtureAttributeRefusals(name, attributes, isRoot));
		if (selfClosing === '') open.push(name);
		rest = rest.slice(whole.length);
	}
	if (open.length > 0) refusals.push(`unclosed <${open.join('>, <')}>`);
	if (roots !== 1) refusals.push(`${String(roots)} root elements, not one <svg>`);
	return refusals;
}

/** Why a fixture SVG is refused: every SVG's refusals and the fixtures' allow-list. */
export function fixtureSvgRefusals(text: string): readonly string[] {
	return [...svgRefusals(text), ...fixtureShapeRefusals(text)];
}

/** The findings in one built file's text; `mode` defaults to the Kit gallery's build. */
export function builtFileFindings(
	file: string,
	text: string,
	mode: BuildMode = 'kit'
): readonly BuiltFinding[] {
	const kind = builtKind(file);
	const inText = environmentFindings(file, text);
	if (kind === 'svg') {
		const inSvg = svgRefusals(text).map((found): BuiltFinding => ({
			rule: 'svg',
			found,
			place: placeOf(file)
		}));
		return [...inSvg, ...inText];
	}
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

/**
 * The fixtures folder of a build: only the fixture names, as plain files right
 * inside it, no folder in it, and each fixture held to the allow-list.
 */
function fixtureFindings(folder: string, found: Walk, errors: string[]): readonly BuiltFinding[] {
	const relativeOf = (entry: string): string =>
		path.relative(folder, entry).split(path.sep).join('/');
	const inFixtures = (entry: string): boolean => relativeOf(entry).startsWith(`${FIXTURE_FOLDER}/`);
	const folders = found.folders.filter(inFixtures).map((entry): BuiltFinding => ({
		rule: 'fixture',
		found: 'a folder inside the fixtures folder',
		place: placeOf(entry)
	}));
	const files = found.files.filter((file) => inFixtures(file.path));
	const isListed = (file: BuiltFile): boolean =>
		FIXTURE_NAMES.some((name) => relativeOf(file.path) === `${FIXTURE_FOLDER}/${name}`);
	const unlisted = files
		.filter((file) => !isListed(file))
		.map((file): BuiltFinding => ({
			rule: 'fixture',
			found: 'a file the fixture list does not name',
			place: placeOf(file.path)
		}));
	const shapes = files.filter(isListed).flatMap((file) => {
		let text;
		try {
			text = readFileSync(file.path, 'utf8');
		} catch (error) {
			errors.push(`${file.path}: ${reasonOf(error)}`);
			return [];
		}
		return fixtureShapeRefusals(text).map((found): BuiltFinding => ({
			rule: 'svg',
			found,
			place: placeOf(file.path)
		}));
	});
	return [...folders, ...unlisted, ...shapes];
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
	const found: Walk = { files: [], folders: [], links: [], errors: [] };
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
	const inFixtures = fixtureFindings(folder, found, errors);
	return { findings: [...linkFindings, ...fileFindings, ...inFixtures], errors };
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
