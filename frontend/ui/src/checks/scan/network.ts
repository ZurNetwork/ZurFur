/**
 * The network scan: markup and CSS never fetch from another origin. It
 * refuses a `url()` or a string in any declaration (which covers
 * `image-set()`) that is remote or names a scheme other than `data:`, an
 * `@import` of anything but a relative path (`./`, `../`), a remote value in
 * a URL-bearing attribute (`src`, `href`, `srcset`, `action`, …; static text
 * and the string literals in its expressions), any `http-equiv` attribute
 * on any element, in any letter case and whatever its value (a refresh
 * pragma navigates the page), any `<svelte:head>` outside the gallery host
 * (`src/routes/`, `src/gallery/`), and anything in the host's head but
 * `<title>` and `<meta name>` without `http-equiv`. In an `.html` file, the page
 * shell, a URL-bearing attribute names no scheme at all, and no string in a
 * `<script>` is remote. Remote means `http:`, `https:` or protocol-relative
 * (`//`, `\\`), read the way a browser reads a URL.
 *
 * Doc comments under `src/lib/` (JSDoc in TypeScript and in Svelte scripts,
 * and Svelte `@component` comments) hold no URL (`http:`, `https:`, `//`),
 * no HTML tag outside an inline code span, and no Markdown link, image,
 * reference or autolink destination that reads as remote (`/\host`, `\\host`,
 * `\/\/host`): Storybook's docs pages render them as Markdown, raw HTML
 * allowed. The stories spec holds every string in a stories file to the same
 * rule.
 *
 * Not caught: a URL built at run time from an expression, a spread
 * attribute, a `<svelte:element this="style">`, and the doc comments of
 * types a component takes from a package.
 */
import type {
	DocComment,
	DocComments,
	Head,
	HttpEquivAttribute,
	ParsedSource,
	Place,
	ScriptElement,
	UrlAttribute,
	UrlAttributeName
} from './parse.ts';
import { cssValueParts } from './parse.ts';

/** The folders of the gallery host, the one place a `<svelte:head>` may live. */
export const hostFolders = ['src/routes/', 'src/gallery/'] as const;

export type NetworkRule =
	| 'remote-url'
	| 'scheme-url'
	| 'remote-string'
	| 'scheme-string'
	| 'remote-import'
	| 'non-relative-import'
	| 'remote-attribute'
	| 'shell-scheme'
	| 'remote-script-string'
	| 'http-equiv'
	| 'head-outside-host'
	| 'head-content'
	| 'doc-url'
	| 'doc-tag'
	| 'doc-destination'
	| 'unparsed';

export interface NetworkFinding {
	readonly rule: NetworkRule;
	readonly found: string;
	readonly place: Place;
}

/**
 * True when a browser would fetch `url` from another origin: an `http:` or
 * `https:` URL, or a protocol-relative one. Tabs and newlines are dropped and
 * leading controls and spaces trimmed first, as the URL parser does.
 */
export function isRemote(url: string): boolean {
	const read = urlReading(url);
	return read.startsWith('http:') || read.startsWith('https:') || /^[/\\]{2}/.test(read);
}

/** `url` as the URL parser reads its start: tabs and newlines dropped, leading controls and spaces trimmed, in lower case. */
function urlReading(url: string): string {
	const unbroken = url.replace(/[\t\n\r]/g, '');
	let start = 0;
	while (start < unbroken.length && unbroken.charCodeAt(start) <= 0x20) start += 1;
	return unbroken.slice(start).toLowerCase();
}

/** The scheme `url` starts with, in lower case, read as the URL parser reads it; undefined when it has none. */
export function schemeOf(url: string): string | undefined {
	return /^([a-z][a-z0-9+.-]*):/.exec(urlReading(url))?.[1];
}

/** True when `url` is remote or names any scheme, `data:` included. */
export function namesAnotherPlace(url: string): boolean {
	return isRemote(url) || schemeOf(url) !== undefined;
}

/** True when `url` names a scheme other than `data:` without being remote: `ftp:`, `blob:`, `mailto:`, … */
function namesAnotherScheme(url: string): boolean {
	const scheme = schemeOf(url);
	return !isRemote(url) && scheme !== undefined && scheme !== 'data';
}

/** True when an `@import` target is a relative path, exactly as written: `./…` or `../…`. */
function isRelativeImport(target: string): boolean {
	return target.startsWith('./') || target.startsWith('../');
}

/** True for an `.html` file, the page shell. */
function isShell(path: string): boolean {
	return path.toLowerCase().endsWith('.html');
}

function isHost(path: string): boolean {
	return hostFolders.some((folder) => path.startsWith(folder));
}

/** The URLs one attribute value holds. */
function attributeUrls(attribute: UrlAttributeName, value: string): readonly string[] {
	switch (attribute) {
		case 'srcset':
		case 'imagesrcset':
			return value.split(',');
		case 'ping':
			return value.split(/\s+/);
		case 'src':
		case 'href':
		case 'action':
		case 'formaction':
		case 'poster':
		case 'data':
		case 'xlink:href':
		case 'background':
			return [value];
	}
}

/** The URLs one attribute holds, each as the browser reads it. */
export function urlsOf(attribute: UrlAttribute): readonly string[] {
	return attribute.values.flatMap((value) => attributeUrls(attribute.attribute, value));
}

function attributeFindings(attribute: UrlAttribute, inShell: boolean): readonly NetworkFinding[] {
	return urlsOf(attribute).flatMap((url): readonly NetworkFinding[] => {
		const found = `<${attribute.element} ${attribute.attribute}="${url.trim()}">`;
		if (isRemote(url)) return [{ rule: 'remote-attribute', found, place: attribute.place }];
		if (inShell && schemeOf(url) !== undefined) {
			return [{ rule: 'shell-scheme', found, place: attribute.place }];
		}
		return [];
	});
}

/**
 * The page shell's scripts, which lint never reads: each is JavaScript, and
 * no string in it is remote.
 */
function scriptFindings(script: ScriptElement): readonly NetworkFinding[] {
	switch (script.kind) {
		case 'unparsed':
			return [{ rule: 'unparsed', found: `<script>: ${script.reason}`, place: script.place }];
		case 'javascript':
			return script.strings.filter(isRemote).map((text) => ({
				rule: 'remote-script-string',
				found: `<script> "${text.trim().slice(0, 60)}"`,
				place: script.place
			}));
	}
}

/** An `http-equiv` attribute as the scans name it: its element and its value as written. */
export function describeHttpEquiv(attribute: HttpEquivAttribute): string {
	const { element, value } = attribute;
	switch (value.kind) {
		case 'absent':
			return `<${element} http-equiv>`;
		case 'text':
			return `<${element} http-equiv="${value.text.trim()}">`;
		case 'expression':
			return `<${element} http-equiv={…}>`;
	}
}

function httpEquivFinding(attribute: HttpEquivAttribute): NetworkFinding {
	return { rule: 'http-equiv', found: describeHttpEquiv(attribute), place: attribute.place };
}

function isAllowedInHostHead(child: Head['children'][number]): boolean {
	if (child.type === 'Comment' || child.type === 'TitleElement') return true;
	if (child.type === 'Text') return child.blank;
	return (
		child.type === 'RegularElement' &&
		child.name === 'meta' &&
		child.attributes.includes('name') &&
		child.attributes.every((attribute) => attribute !== 'http-equiv' && attribute !== '{…}')
	);
}

function headFindings(path: string, head: Head): readonly NetworkFinding[] {
	if (!isHost(path)) {
		return [{ rule: 'head-outside-host', found: '<svelte:head>', place: head.place }];
	}
	return head.children
		.filter((child) => !isAllowedInHostHead(child))
		.map((child) => ({
			rule: 'head-content',
			found: child.name === '' ? child.type : `<${child.name}>`,
			place: child.place
		}));
}

/** The network findings in one parsed file. */
export function networkFindings(source: ParsedSource): readonly NetworkFinding[] {
	const unparsed = source.unparsed.map((failure): NetworkFinding => ({
		rule: 'unparsed',
		found: failure.reason,
		place: failure.place
	}));
	const inDeclarations = source.declarations.flatMap((declaration) => {
		const { urls, strings } = cssValueParts(declaration.value);
		const urlFindings = urls.flatMap((url): readonly NetworkFinding[] => {
			const found = `${declaration.property}: url(${url})`;
			if (isRemote(url)) return [{ rule: 'remote-url', found, place: declaration.place }];
			if (namesAnotherScheme(url)) return [{ rule: 'scheme-url', found, place: declaration.place }];
			return [];
		});
		const stringFindings = strings.flatMap((text): readonly NetworkFinding[] => {
			const found = `${declaration.property}: "${text}"`;
			if (isRemote(text)) return [{ rule: 'remote-string', found, place: declaration.place }];
			if (namesAnotherScheme(text)) {
				return [{ rule: 'scheme-string', found, place: declaration.place }];
			}
			return [];
		});
		return [...urlFindings, ...stringFindings];
	});
	const inImports = source.imports.flatMap((cssImport): readonly NetworkFinding[] => {
		const { urls, strings } = cssValueParts(cssImport.prelude);
		const targets = [...urls, ...strings];
		const place = cssImport.place;
		if (targets.length === 0) {
			return [{ rule: 'non-relative-import', found: `@import ${cssImport.prelude}`, place }];
		}
		return targets.flatMap((target): readonly NetworkFinding[] => {
			const found = `@import ${target}`;
			if (isRemote(target)) return [{ rule: 'remote-import', found, place }];
			if (!isRelativeImport(target)) return [{ rule: 'non-relative-import', found, place }];
			return [];
		});
	});
	const inShell = isShell(source.path);
	const inAttributes = source.urlAttributes.flatMap((attribute) =>
		attributeFindings(attribute, inShell)
	);
	const inScripts = inShell ? source.scripts.flatMap(scriptFindings) : [];
	const inHttpEquivs = source.httpEquivs.map(httpEquivFinding);
	const inHeads = source.heads.flatMap((head) => headFindings(source.path, head));
	return [
		...unparsed,
		...inDeclarations,
		...inImports,
		...inAttributes,
		...inScripts,
		...inHttpEquivs,
		...inHeads
	];
}

/** The folder whose doc comments Storybook's docs pages render. */
export const libFolder = 'src/lib/';

/** A URL in a doc comment: `http:`, `https:` or `//`, in any letter case. */
const docUrl = /https?:|\/\//i;

/** The start of an HTML tag, a comment or a declaration, as Markdown passes raw HTML through. */
const docTag = /<[A-Za-z/!?]/;

/**
 * The comment's text with each simple inline code span blanked: one backtick,
 * text holding no backtick, backslash or line break, one backtick. A comment
 * holding a backslash before a backtick or a run of two or more backticks is
 * left whole, since a renderer may pair its spans otherwise.
 */
function outsideCodeSpans(text: string): string {
	if (/\\`|``/.test(text)) return text;
	return text.replace(/`[^`\\\n]*`/g, (span) => ' '.repeat(span.length));
}

/** Where a Markdown destination starts: after `](` (a link or image), `]:` (a reference definition) or `<` (an autolink). */
const docDestinationStart = /\]\(|\]:|</g;

/** A Markdown backslash escape, as the docs pages' renderer undoes it in a destination. */
const markdownEscape = /\\([^0-9A-Za-z\s])/g;

/**
 * Each Markdown link, image, reference and autolink destination in `text`,
 * from its start to the end of the text, as written and with backslash
 * escapes undone. It over-reads on purpose: only a destination's start
 * decides whether it is remote.
 */
export function markdownDestinations(text: string): readonly string[] {
	return [...text.matchAll(docDestinationStart)].flatMap((match) => {
		const destination = text.slice(match.index + match[0].length).replace(/^\s*</, '');
		return [destination, destination.replace(markdownEscape, '$1')];
	});
}

/** A rule docs text breaks. */
export type DocTextRule = 'doc-url' | 'doc-tag' | 'doc-destination';

/**
 * The rules `text` breaks as docs text, which Storybook renders as Markdown
 * with raw HTML allowed: a URL anywhere, an HTML tag outside inline code
 * spans, or a Markdown destination a browser reads as remote.
 */
export function docTextRules(text: string): readonly DocTextRule[] {
	const rules: readonly (DocTextRule | false)[] = [
		docUrl.test(text) && 'doc-url',
		docTag.test(outsideCodeSpans(text)) && 'doc-tag',
		markdownDestinations(text).some(isRemote) && 'doc-destination'
	];
	return rules.filter((rule) => rule !== false);
}

function docFindings(comment: DocComment): readonly NetworkFinding[] {
	const excerpt = comment.text.replace(/\s+/g, ' ').slice(0, 80);
	return docTextRules(comment.text).map((rule) => ({ rule, found: excerpt, place: comment.place }));
}

/** The findings in one file's doc comments; only files under `src/lib/` are held to the rule. */
export function docCommentFindings(path: string, read: DocComments): readonly NetworkFinding[] {
	if (!path.startsWith(libFolder)) return [];
	const unparsed = read.unparsed.map((failure): NetworkFinding => ({
		rule: 'unparsed',
		found: failure.reason,
		place: failure.place
	}));
	return [...unparsed, ...read.comments.flatMap(docFindings)];
}
