/**
 * The path bar's input: the steps from the top down to the open page, which a
 * page's load hands the frame as `trail`. The last step is the page itself.
 */

import type { ResolvedPathname } from '$app/types';

/** One step of a {@link Trail}. */
export type TrailStep =
	/** My Den's root, shown as `~` with "My Den" for screen readers. */
	| { readonly step: 'root'; readonly href: ResolvedPathname | undefined }
	/** Any other step, shown by its name. */
	| { readonly step: 'named'; readonly label: string; readonly href: ResolvedPathname | undefined };

/** The steps from the top down to the open page. */
export type Trail = readonly TrailStep[];
