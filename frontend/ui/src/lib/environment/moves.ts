import type { Attachment } from 'svelte/attachments';

export type NoteOrigin = 'in-place' | 'edge-start' | 'edge-end' | 'below';

export interface Moves {
	/** A press: the element's `scale` squishes and springs back. Never `transform`. */
	readonly squish: () => Attachment<HTMLElement>;
	/**
	 * A note put down on mount and picked up when its data-state turns closed.
	 * The exit is a CSS or Web Animations animation on the attached element,
	 * the one that takes bits-ui's props.
	 */
	readonly note: (origin: NoteOrigin) => Attachment<HTMLElement>;
}

export type MoveName = keyof Moves;

const NONE: Attachment<HTMLElement> = () => undefined;

export const NO_MOVES: Moves = Object.freeze({
	squish: () => NONE,
	note: () => NONE
});
