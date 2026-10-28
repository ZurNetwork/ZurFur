import type { Attachment } from 'svelte/attachments';

export type NoteOrigin = 'in-place' | 'edge-start' | 'edge-end' | 'below';
export type LoopKind = 'spin' | 'breathe';

export interface Moves {
	/** A press: the element's `scale` squishes and springs back. Never `transform`. */
	readonly squish: () => Attachment<HTMLElement>;
	/**
	 * A note put down on mount and picked up when its data-state turns closed.
	 * The exit is a CSS or Web Animations animation on the attached element,
	 * the one that takes bits-ui's props.
	 */
	readonly note: (origin: NoteOrigin) => Attachment<HTMLElement>;
	/** The desk dims under a layer on mount and clears when its data-state turns closed; the exit as for `note`. */
	readonly dim: () => Attachment<HTMLElement>;
	/**
	 * A toast's whole life: put down from below, held, picked up, then `onEnd` once.
	 * The wait pauses under the pointer or focus. Without an adapter, `onEnd` is never called.
	 * If the element is removed first, `onEnd` is never called. The host removes it on `onEnd`.
	 */
	readonly life: (onEnd: () => void) => Attachment<HTMLElement>;
	/** A loop while mounted: `spin` turns the element, `breathe` fades it. */
	readonly loop: (kind: LoopKind) => Attachment<HTMLElement>;
	/** Confetti thrown once from the element on mount; the element itself doesn't move. */
	readonly burst: () => Attachment<HTMLElement>;
}

export type MoveName = keyof Moves;

const NONE: Attachment<HTMLElement> = () => undefined;

export const NO_MOVES: Moves = Object.freeze({
	squish: () => NONE,
	note: () => NONE,
	dim: () => NONE,
	life: () => NONE,
	loop: () => NONE,
	burst: () => NONE
});
