import { isGroup, type Group } from './groups';

declare const slugBrand: unique symbol;

/** A page's URL segment under `/components/`: lowercase words joined by single hyphens. */
export type Slug = string & { readonly [slugBrand]: true };

const slugPattern = /^[a-z0-9]+(?:-[a-z0-9]+)*$/;

/** Whether the text is a well-formed page slug. */
export function isSlug(text: string): text is Slug {
	return slugPattern.test(text);
}

/** What a page tells the gallery about itself: where it sits and what to call its link. */
export interface PageMeta {
	readonly slug: Slug;
	readonly title: string;
	readonly group: Group;
}

/** Why a page's `meta` export was refused. */
export type MetaRefusal = 'missing-meta' | 'malformed-meta' | 'unknown-group';

export type MetaParse =
	| { readonly kind: 'page'; readonly meta: PageMeta }
	| { readonly kind: 'refused'; readonly reason: MetaRefusal };

type Fields = Readonly<Record<string, unknown>>;

function isFields(value: unknown): value is Fields {
	return Object(value) === value;
}

function refused(reason: MetaRefusal): MetaParse {
	return { kind: 'refused', reason };
}

/**
 * Reads the `meta` export of a page module. The module is untrusted: it must
 * export an object holding a non-empty string `title` and a string `group`
 * that names one of the gallery's groups.
 */
export function parseMeta(slug: Slug, module: unknown): MetaParse {
	if (!isFields(module)) return refused('missing-meta');
	const exported = module.meta;
	if (exported === undefined) return refused('missing-meta');
	if (!isFields(exported)) return refused('malformed-meta');
	const title = exported.title;
	const group = exported.group;
	if (typeof title !== 'string' || title.trim() === '') return refused('malformed-meta');
	if (typeof group !== 'string') return refused('malformed-meta');
	if (!isGroup(group)) return refused('unknown-group');
	const meta: PageMeta = { slug, title, group };
	return { kind: 'page', meta };
}
