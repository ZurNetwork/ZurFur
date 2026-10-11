/**
 * The path bar's input: the steps from the top down to the open page, which a
 * page's load hands the frame as `trail`. The last step is the page itself.
 */

import type { DenHref } from '$lib/types/brand';

/** The fixed section pages a path step may link to: a closed list, so no other address type-checks. */
export type SectionHref = '/accounts';

/** Where a step links: a fixed section page, or a Den link the Den path builder built. */
export type TrailHref = SectionHref | DenHref;

/** One step of a {@link Trail}. */
export type TrailStep =
	/** My Den's root, shown as `~` with "My Den" for screen readers. */
	| { readonly step: 'root'; readonly href: TrailHref | undefined }
	/** Any other step, shown by its name. */
	| { readonly step: 'named'; readonly label: string; readonly href: TrailHref | undefined };

/** The steps from the top down to the open page. */
export type Trail = readonly TrailStep[];
