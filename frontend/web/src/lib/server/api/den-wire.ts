/**
 * The live Den reads' decode: a generated Den answer mapped to the port's
 * plain {@link DenRead}, failing closed wherever the answer says less than
 * this build knows.
 */

import type {
	DenCrumb,
	DenListing,
	DenNode,
	GetDenNodeResponse,
	GetDenRootResponse
} from './generated/zurfur/api/v1/den_pb';
import type { DenRead, DenReadContent, DenReadCrumb, DenReadNode } from './den-read';

/** The route templates a Den read's failures name: never the built path, which carries private names. */
export const DEN_ROUTES = { root: '/den', node: '/den/{path}' } as const;

/** One of {@link DEN_ROUTES}: a built path is not assignable to it. */
export type DenRoute = (typeof DEN_ROUTES)[keyof typeof DEN_ROUTES];

/** One node; anything but an `open` access, unset included, is a card. */
function toReadNode(node: DenNode): DenReadNode {
	const base = {
		segments: node.segments,
		name: node.name,
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
			removed: open.softDelete === '' ? undefined : open.softDelete
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

function toCrumb(crumb: DenCrumb): DenReadCrumb {
	return { segments: crumb.segments, name: crumb.name };
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
