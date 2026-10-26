/**
 * The network scan: markup and CSS never fetch from another origin. It
 * refuses a remote `url()` and a remote string in any declaration (which
 * covers `image-set()`), a remote `@import`, a remote value in a URL-bearing
 * attribute (`src`, `href`, `srcset`, `action`, …; static text and the string
 * literals in its expressions), any `<svelte:head>` outside the gallery host
 * (`src/routes/`, `src/gallery/`), and anything in the host's head but
 * `<title>` and `<meta name>`. Remote means `http:`, `https:` or
 * protocol-relative (`//`, `\\`), read the way a browser reads a URL.
 *
 * Not caught: a URL built at run time from an expression, a spread
 * attribute, and a `<svelte:element this="style">`.
 */
import type { Head, ParsedSource, Place, UrlAttribute, UrlAttributeName } from './parse';
import { cssValueParts } from './parse';

/** The folders of the gallery host, the one place a `<svelte:head>` may live. */
export const hostFolders = ['src/routes/', 'src/gallery/'] as const;

export type NetworkRule =
	| 'remote-url'
	| 'remote-string'
	| 'remote-import'
	| 'remote-attribute'
	| 'head-outside-host'
	| 'head-content'
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
	const unbroken = url.replace(/[\t\n\r]/g, '');
	let start = 0;
	while (start < unbroken.length && unbroken.charCodeAt(start) <= 0x20) start += 1;
	const read = unbroken.slice(start).toLowerCase();
	return read.startsWith('http:') || read.startsWith('https:') || /^[/\\]{2}/.test(read);
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

function attributeFindings(attribute: UrlAttribute): readonly NetworkFinding[] {
	return attribute.values
		.flatMap((value) => attributeUrls(attribute.attribute, value))
		.filter(isRemote)
		.map((url) => ({
			rule: 'remote-attribute',
			found: `<${attribute.element} ${attribute.attribute}="${url.trim()}">`,
			place: attribute.place
		}));
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
		const remoteUrls = urls.filter(isRemote).map((url): NetworkFinding => ({
			rule: 'remote-url',
			found: `${declaration.property}: url(${url})`,
			place: declaration.place
		}));
		const remoteStrings = strings.filter(isRemote).map((text): NetworkFinding => ({
			rule: 'remote-string',
			found: `${declaration.property}: "${text}"`,
			place: declaration.place
		}));
		return [...remoteUrls, ...remoteStrings];
	});
	const inImports = source.imports.flatMap((cssImport) => {
		const { urls, strings } = cssValueParts(cssImport.prelude);
		return [...urls, ...strings].filter(isRemote).map((url): NetworkFinding => ({
			rule: 'remote-import',
			found: `@import ${url}`,
			place: cssImport.place
		}));
	});
	const inAttributes = source.urlAttributes.flatMap(attributeFindings);
	const inHeads = source.heads.flatMap((head) => headFindings(source.path, head));
	return [...unparsed, ...inDeclarations, ...inImports, ...inAttributes, ...inHeads];
}
