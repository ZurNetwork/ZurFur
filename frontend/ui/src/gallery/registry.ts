import { groupNames } from './groups';
import { isSlug, parseMeta, type MetaRefusal, type PageMeta } from './page-meta';

/** Why a page file was left out of the registry. */
export type RefusalReason = 'path-shape' | MetaRefusal | 'duplicate-slug';

export interface Refusal {
	readonly path: string;
	readonly reason: RefusalReason;
}

/** The pages the gallery lists, in nav order, and the files it turned away. */
export interface Registry {
	readonly entries: readonly PageMeta[];
	readonly refusals: readonly Refusal[];
}

const pathPattern = /^\/src\/routes\/components\/([^/]+)\/meta\.ts$/;

function compareText(left: string, right: string): number {
	if (left < right) return -1;
	return left > right ? 1 : 0;
}

function compareEntries(left: PageMeta, right: PageMeta): number {
	const byGroup = groupNames.indexOf(left.group) - groupNames.indexOf(right.group);
	if (byGroup !== 0) return byGroup;
	const byTitle = compareText(left.title, right.title);
	return byTitle !== 0 ? byTitle : compareText(left.slug, right.slug);
}

/**
 * Builds the registry from `[path, module]` pairs. A file is refused, never
 * thrown on, when its path is not `/src/routes/components/<slug>/meta.ts`, its
 * `meta` export is missing or malformed, its group is unknown, or an earlier
 * file already claimed its slug.
 */
export function buildRegistry(files: readonly (readonly [string, unknown])[]): Registry {
	const entries: PageMeta[] = [];
	const refusals: Refusal[] = [];
	const taken = new Set<string>();
	const ordered = files.toSorted(([left], [right]) => compareText(left, right));
	for (const [path, module] of ordered) {
		const segment = pathPattern.exec(path)?.[1];
		if (segment === undefined || !isSlug(segment)) {
			refusals.push({ path, reason: 'path-shape' });
			continue;
		}
		if (taken.has(segment)) {
			refusals.push({ path, reason: 'duplicate-slug' });
			continue;
		}
		const parsed = parseMeta(segment, module);
		switch (parsed.kind) {
			case 'page':
				taken.add(segment);
				entries.push(parsed.meta);
				break;
			case 'refused':
				refusals.push({ path, reason: parsed.reason });
				break;
		}
	}
	return { entries: entries.toSorted(compareEntries), refusals };
}

/** Every page under `src/routes/components/`, found by their `meta.ts` files. */
export const registry: Registry = buildRegistry(
	Object.entries(import.meta.glob('/src/routes/**/meta.ts', { eager: true }))
);
