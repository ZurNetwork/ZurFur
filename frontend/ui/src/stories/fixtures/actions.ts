import { isLinkHref, type IconName, type LinkHref, type PillOption } from '@zurfur/ui';

/** The words a button says: a verb and its object. */
export const actionWords = {
	send: 'Send request',
	addReference: 'Add a reference',
	cancel: 'Cancel',
	deleteSketch: 'Delete sketch',
	next: 'Next',
	openQueue: 'Open the queue'
} as const;

/** The queue's address, checked by the link guard; undefined if it ever stops passing. */
const queueAddress = '/den/commissions';
export const queueLink: LinkHref | undefined = isLinkHref(queueAddress) ? queueAddress : undefined;

/** One icon-only tool: what it looks like and what it does. */
export interface Tool {
	readonly icon: IconName;
	readonly label: string;
}

export const copyTool: Tool = { icon: 'copy', label: 'Copy path' };
export const hiddenFilesTool: Tool = { icon: 'eye', label: 'Show hidden files' };
export const filesTool: Tool = { icon: 'folder', label: 'Show files' };

/** A box and its words: a label, and a line that says more. */
export interface Choice {
	readonly label: string;
	readonly description: string;
}

export const shareSketches: Choice = {
	label: 'Share sketches',
	description: 'Shown on your profile'
};
export const selectAllSketches: Choice = {
	label: 'Select all sketches',
	description: 'Some are ticked'
};
export const showQueue: Choice = {
	label: 'Show my queue',
	description: 'Commissions you are working on'
};

export type FilterId = 'all' | 'sketches' | 'stickers';

/** A plain filter: one word for each option. */
export const filterOptions: readonly PillOption<FilterId>[] = [
	{ id: 'all', label: 'All' },
	{ id: 'sketches', label: 'Sketches' },
	{ id: 'stickers', label: 'Stickers' }
];

export type BatchId = 'all' | 'spooky_sketches' | 'stickers';

/** The batch filter: one plain option and two choices of the same type, which the switch shows once. */
export const batchOptions: readonly PillOption<BatchId>[] = [
	{ id: 'all', label: 'All' },
	{ id: 'spooky_sketches', label: 'spooky_sketches', prefix: 'batch:' },
	{ id: 'stickers', label: 'stickers', prefix: 'batch:' }
];

export type MixedId = 'all' | 'spooky_sketches' | 'sketch';

/** Two types in one switch, so each typed option keeps its own muted type. */
export const mixedOptions: readonly PillOption<MixedId>[] = [
	{ id: 'all', label: 'All' },
	{ id: 'spooky_sketches', label: 'spooky_sketches', prefix: 'batch:' },
	{ id: 'sketch', label: 'sketch', prefix: 'phase:' }
];

export type ViewId = 'list' | 'files' | 'calendar';

/** The same kind of switch, with an icon before each label. */
export const viewOptions: readonly PillOption<ViewId>[] = [
	{ id: 'list', label: 'List', icon: 'list' },
	{ id: 'files', label: 'Files', icon: 'folder' },
	{ id: 'calendar', label: 'Calendar', icon: 'calendar' }
];

export type StageId = 'sketch' | 'lineart' | 'colour';

/** Three stages, the last one not open yet. */
export const stageOptions: readonly PillOption<StageId>[] = [
	{ id: 'sketch', label: 'Sketch' },
	{ id: 'lineart', label: 'Lineart' },
	{ id: 'colour', label: 'Colour', disabled: true }
];
