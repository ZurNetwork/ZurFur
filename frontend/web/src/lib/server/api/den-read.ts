/**
 * What the `ZurfurApi` port's Den reads answer: the Den contract's shape as
 * plain data — segments as the backend sends them and vocabularies as raw
 * strings. The mock and the live port's contract decode (`den-wire.ts`)
 * both produce it. The den program (`lib/server/den.ts`) maps it to the
 * web's own view, building every link through the path builder.
 */

import type { CommissionId, Did, NameOrdinal, PageToken } from '$lib/types/brand';

/** What a Den read asks for besides the path. */
export interface DenQuery {
	/** Also list soft-deleted nodes the viewer may open. */
	readonly includeDeleted: boolean;
	/** Where a later page starts; absent for the first page. */
	readonly pageToken: PageToken | undefined;
}

/** The thing an open node stands for: a commission by its id or an Account by its DID. */
export type DenReadThing =
	| { readonly thing: 'commission'; readonly id: CommissionId }
	| { readonly thing: 'account'; readonly did: Did };

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
			/** What the node stands for; absent on kinds that name no thing, or when the answer names none this build knows. */
			readonly thing: DenReadThing | undefined;
	  }
	| { readonly access: 'card' };

/** One node as the Den shows it to this viewer. */
export interface DenReadNode {
	/** Its Den path from the viewer's root; empty on a card. */
	readonly segments: readonly string[];
	/** Its own name, never carrying its number. */
	readonly name: string;
	/** Its number among the equal names its folder shows this viewer; absent when it needs none. */
	readonly ordinal: NameOrdinal | undefined;
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
	/** Its number in its parent's listing; the root never has one. */
	readonly ordinal: NameOrdinal | undefined;
	/** The type of the node the crumb stands for; empty when the answer gives none. */
	readonly type: string;
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
