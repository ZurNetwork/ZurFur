/** One path grammar for links and same-origin sources, and the two brands it mints. */

declare const linkHrefBrand: unique symbol;
declare const sameOriginPathBrand: unique symbol;

/** A link a Button may follow: a same-origin path, or one of the bare roots SvelteKit's relative paths emit. */
export type LinkHref = string & { readonly [linkHrefBrand]: true };

/** A same-origin path with at least one segment; every one is also a {@link LinkHref}. */
export type SameOriginPath = LinkHref & { readonly [sameOriginPathBrand]: true };

/** A backslash, any whitespace, or a control character, anywhere. */
const UNSAFE_CHARACTER = /[\\\s\p{Cc}]/u;

/** An encoded slash or backslash, in any letter case, anywhere. */
const ENCODED_SEPARATOR = /%(?:2f|5c)/i;

/** `/`, `./` or one or more `../`, then the rest. */
const PREFIXED = /^(?:\/|\.\/|(?:\.\.\/)+)(?<rest>.*)$/s;

/** The path ends at the first `?` or `#`. */
const PATH_PART = /^[^?#]*/;

/** Two slashes in a row. */
const DOUBLE_SLASH = /\/{2}/;

/** A single-dot or double-dot segment, its dots written plain or as `%2e`, in any letter case. */
const DOT_SEGMENT = /^(?:\.|%2e){1,2}$/i;

/** The bare roots: `/`, `.`, `./`, `..`, `../`, `../..` and `../../`. */
const BARE_ROOT = /^(?:\/|\.\/?|\.\.(?:\/\.\.)?\/?)$/;

/**
 * Whether `text` is a same-origin path: `/`, `./` or `../` (repeated) and at least one segment,
 * with no backslash, whitespace, control character, encoded slash, `//` or dot segment in the path.
 */
export function isSameOriginPath(text: string): text is SameOriginPath {
	if (UNSAFE_CHARACTER.test(text) || ENCODED_SEPARATOR.test(text)) return false;
	const rest = PREFIXED.exec(text)?.groups?.rest;
	if (rest === undefined) return false;
	const path = PATH_PART.exec(text)?.[0] ?? '';
	const segments = PATH_PART.exec(rest)?.[0].split('/') ?? [];
	const [first] = segments;
	if (first === undefined || first === '') return false;
	return !DOUBLE_SLASH.test(path) && !segments.some((segment) => DOT_SEGMENT.test(segment));
}

/** Whether `text` is a link a Button may follow: a same-origin path, or exactly one of the bare roots. */
export function isLinkHref(text: string): text is LinkHref {
	return isSameOriginPath(text) || BARE_ROOT.test(text);
}
