/**
 * The live Den reads' decode: a generated Den answer mapped to the port's
 * plain {@link DenRead}, failing closed wherever the answer says less than
 * this build knows.
 */

import { commissionId, did, nameOrdinal } from '$lib/types/brand';
import type {
	DenCrumb,
	DenListing,
	DenNode,
	DenOpen,
	GetDenNodeResponse,
	GetDenRootResponse
} from './generated/zurfur/api/v1/den_pb';
import type { DenRead, DenReadContent, DenReadCrumb, DenReadNode, DenReadThing } from './den-read';

/** The route templates a Den read's failures name: never the built path, which carries private names. */
export const DEN_ROUTES = { root: '/den', node: '/den/{path}' } as const;

/** One of {@link DEN_ROUTES}: a built path is not assignable to it. */
export type DenRoute = (typeof DEN_ROUTES)[keyof typeof DEN_ROUTES];

/**
 * The thing an open node names. A case this build doesn't know, no case, an
 * empty commission id, or an empty or whitespace-only account did is no thing.
 * A kept value is passed on untrimmed.
 */
function toThing(thing: DenOpen['thing']): DenReadThing | undefined {
	switch (thing.case) {
		case 'commissionId': {
			const id = commissionId(thing.value);
			return id === undefined ? undefined : { thing: 'commission', id };
		}
		case 'accountDid':
			return thing.value.trim() === '' ? undefined : { thing: 'account', did: did(thing.value) };
		case undefined:
			return undefined;
	}
}

/**
 * One node; anything but an `open` access, unset included, is a card, and a
 * number that isn't a whole number of 2 or more is none.
 */
function toReadNode(node: DenNode): DenReadNode {
	const base = {
		segments: node.segments,
		name: node.name,
		ordinal: nameOrdinal(node.nameOrdinal),
		type: node.type,
		kind: node.kind,
		mount: node.mount
	};
	if (node.access.case !== 'open') return { ...base, access: { access: 'card' } };
	const open = node.access.value;
	return {
		...base,
		access: {
			access: 'open',
			ownLevel: open.ownLevel,
			contentNotShown: open.contentNotShown,
			removed: open.softDelete === '' ? undefined : open.softDelete,
			thing: toThing(open.thing)
		}
	};
}

/** One page of entries; an empty token is the last page. */
function toListing(listing: DenListing): DenReadContent {
	return {
		content: 'listing',
		entries: listing.entries.map(toReadNode),
		nextPageToken: listing.nextPageToken === '' ? undefined : listing.nextPageToken
	};
}

/** One crumb; a number that isn't a whole number of 2 or more is none. */
function toCrumb(crumb: DenCrumb): DenReadCrumb {
	return {
		segments: crumb.segments,
		name: crumb.name,
		ordinal: nameOrdinal(crumb.nameOrdinal),
		type: crumb.type
	};
}

/** The root's answer as a read with no crumbs, or `undefined` when it has no node. */
export function denReadFromRoot(message: GetDenRootResponse): DenRead | undefined {
	if (message.node === undefined) return undefined;
	const content: DenReadContent =
		message.content.case === 'listing' ? toListing(message.content.value) : { content: 'none' };
	return { node: toReadNode(message.node), crumbs: [], content };
}

/** What a node's answer holds; a case this build doesn't know, or none, is `none`. */
function nodeContent(content: GetDenNodeResponse['content']): DenReadContent {
	switch (content.case) {
		case 'listing':
			return toListing(content.value);
		case 'file':
			return { content: 'file' };
		case 'link':
			return { content: 'link' };
		case undefined:
			return { content: 'none' };
	}
}

/** A node's answer as a read, or `undefined` when it has no node. */
export function denReadFromNode(message: GetDenNodeResponse): DenRead | undefined {
	if (message.node === undefined) return undefined;
	return {
		node: toReadNode(message.node),
		crumbs: message.crumbs.map(toCrumb),
		content: nodeContent(message.content)
	};
}
