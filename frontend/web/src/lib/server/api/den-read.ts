/**
 * What the `ZurfurApi` port's Den reads answer: the Den contract's shape as
 * plain data — segments as the backend sends them and vocabularies as raw
 * strings. The mock produces it now; the live port's contract decode
 * produces it later. The den program (`lib/server/den.ts`) maps it to the
 * web's own view, building every link through the path builder.
 */

import type { PageToken } from '$lib/types/brand';

/** What a Den read asks for besides the path. */
export interface DenQuery {
	/** Also list soft-deleted nodes the viewer may open. */
	readonly includeDeleted: boolean;
	/** Where a later page starts; absent for the first page. */
	readonly pageToken: PageToken | undefined;
}

/**
 * The viewer's access to a node. Only exactly `open` opens: any other value,
 * a card or one this build doesn't know, renders as a card.
 */
export type DenReadAccess =
	| {
			readonly access: 'open';
			/** `private` | `listed` | `public`; anything else shows no level. */
			readonly ownLevel: string;
			readonly contentNotShown: boolean;
			/** `archived` | `deactivated` when soft-deleted; absent otherwise. */
			readonly removed: string | undefined;
	  }
	| { readonly access: 'card' };

/** One node as the Den shows it to this viewer. */
export interface DenReadNode {
	/** Its Den path from the viewer's root; empty on a card. */
	readonly segments: readonly string[];
	readonly name: string;
	readonly type: string;
	/** `directory` | `file` | `symlink`; empty on a card. */
	readonly kind: string;
	readonly mount: boolean;
	readonly access: DenReadAccess;
}

/** One folder above the node. */
export interface DenReadCrumb {
	readonly segments: readonly string[];
	readonly name: string;
}

/** What an open node holds. */
export type DenReadContent =
	| {
			readonly content: 'listing';
			readonly entries: readonly DenReadNode[];
			readonly nextPageToken: string | undefined;
	  }
	| { readonly content: 'file' }
	| { readonly content: 'link' }
	/** No content case this build knows, or a card's absent content. */
	| { readonly content: 'none' };

/** One Den read: the node, its breadcrumbs (root first), and its content. */
export interface DenRead {
	readonly node: DenReadNode;
	readonly crumbs: readonly DenReadCrumb[];
	readonly content: DenReadContent;
}
