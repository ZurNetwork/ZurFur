/** Steps a scrolling row one item at a time, left to right: the numbers are pure, the DOM glue is small. */
import { on } from 'svelte/events';

/** Which way a step goes: `back` toward the first item, `forward` toward the last. */
export type StepSide = 'back' | 'forward';

/** Whether a row sits at its first or its last item. */
export interface RowEdges {
	readonly atStart: boolean;
	readonly atEnd: boolean;
}

/** The scroll positions at which one item sits flush with the row's start edge and with its end edge. */
export interface ItemFit {
	readonly atStart: number;
	readonly atEnd: number;
}

/** What a row measures: where it is scrolled, how far it can go, and the fit of each item in order. */
export interface RowMeasure {
	readonly position: number;
	readonly furthest: number;
	readonly fits: readonly ItemFit[];
}

/** A gap this small, in pixels, is rounding, not a distance. */
const NEAR = 1;

/** A position held between the row's two ends. */
function within(position: number, furthest: number): number {
	return Math.min(Math.max(position, 0), furthest);
}

/** Whether the row is scrolled all the way back, and all the way forward. */
export function rowEdges({ position, furthest }: RowMeasure): RowEdges {
	return { atStart: position <= NEAR, atEnd: position >= furthest - NEAR };
}

/** Where one step in `side` goes: the start of the next item ahead, or of the one before, held to the row. */
export function stepPosition(side: StepSide, { position, furthest, fits }: RowMeasure): number {
	switch (side) {
		case 'forward': {
			const next = fits.find((fit) => fit.atStart > position + NEAR);
			return within(next === undefined ? furthest : next.atStart, furthest);
		}
		case 'back': {
			const before = fits.findLast((fit) => fit.atStart < position - NEAR);
			return within(before === undefined ? 0 : before.atStart, furthest);
		}
	}
}

/**
 * Where to scroll so the item at `index` lies whole inside the row, landing on
 * an item's start; an item wider than the row lands on its own start, never past
 * it. Undefined when no scroll is needed, or there is no such item.
 */
export function revealPosition(
	index: number,
	{ position, furthest, fits }: RowMeasure
): number | undefined {
	const fit = fits[index];
	if (fit === undefined) return undefined;
	const ownStart = within(fit.atStart, furthest);
	if (position > fit.atStart + NEAR) return ownStart;
	if (position >= fit.atEnd - NEAR) return undefined;
	if (fit.atEnd > fit.atStart) return Math.abs(position - ownStart) <= NEAR ? undefined : ownStart;
	const landing = fits.find((other) => other.atStart >= fit.atEnd - NEAR);
	return within(landing === undefined ? furthest : landing.atStart, furthest);
}

/** A length from a computed style: `auto` and anything else unreadable count as none. */
function lengthOf(text: string): number {
	const length = Number.parseFloat(text);
	return Number.isFinite(length) ? length : 0;
}

/** The row's own items: its direct children that are elements. */
function itemsOf(row: HTMLElement): readonly HTMLElement[] {
	return [...row.children].filter((child) => child instanceof HTMLElement);
}

/** Measures a row of items laid out left to right, allowing for the row's scroll padding. */
export function measureRow(row: HTMLElement): RowMeasure {
	const style = getComputedStyle(row);
	const padStart = lengthOf(style.scrollPaddingLeft);
	const padEnd = lengthOf(style.scrollPaddingRight);
	const fits = itemsOf(row).map((item): ItemFit => {
		// An item's offset counts from its offset parent: the row when it is positioned, else what holds the row.
		const origin = item.offsetParent === row ? 0 : row.offsetLeft + row.clientLeft;
		const start = item.offsetLeft - origin;
		const end = start + item.offsetWidth;
		return { atStart: start - padStart, atEnd: end - row.clientWidth + padEnd };
	});
	return {
		position: row.scrollLeft,
		furthest: Math.max(row.scrollWidth - row.clientWidth, 0),
		fits
	};
}

/** Moves the row one item in `side`, at once: it sets the row's own scroll and nothing else's. */
export function stepRow(row: HTMLElement, side: StepSide): void {
	row.scrollLeft = stepPosition(side, measureRow(row));
}

/**
 * Keeps `onEdges` told where the row stands, and brings a focused item whole
 * into view at once; returns what undoes it all. It listens for scrolls, focus
 * and the person's own scrolling, and watches the row and its items for resizes
 * and for items coming and going.
 */
export function watchRow(row: HTMLElement, onEdges: (edges: RowEdges) => void): () => void {
	const tell = (): void => {
		onEdges(rowEdges(measureRow(row)));
	};
	const resizes = new ResizeObserver(tell);
	const watchItems = (): void => {
		resizes.disconnect();
		resizes.observe(row);
		for (const item of itemsOf(row)) resizes.observe(item);
	};
	const changes = new MutationObserver(() => {
		watchItems();
		tell();
	});
	changes.observe(row, { childList: true });
	/** Scrolls the row so the item holding focus now lies whole, when one does and it does not already. */
	const reveal = (): void => {
		const focused = row.ownerDocument.activeElement;
		const index = itemsOf(row).findIndex(
			(item) => focused instanceof Node && item.contains(focused)
		);
		const target = revealPosition(index, measureRow(row));
		if (target !== undefined) row.scrollLeft = target;
	};
	// Chromium scrolls inside `focus()`, before `focusin`: it centres an item that was hidden and leaves
	// one partly shown, so the reveal on `focusin` has the last word. A browser that scrolls after
	// `focusin` is put back by the first scroll that follows; any later scroll, and any the person
	// starts, is left alone.
	let settling = false;
	const settle = (): void => {
		settling = false;
	};
	const stopScroll = on(
		row,
		'scroll',
		() => {
			if (settling) {
				settling = false;
				reveal();
			}
			tell();
		},
		{ passive: true }
	);
	const stopFocusIn = on(row, 'focusin', () => {
		reveal();
		settling = true;
	});
	const stopFocusOut = on(row, 'focusout', settle);
	const stopWheel = on(row, 'wheel', settle, { passive: true });
	const stopPointer = on(row, 'pointerdown', settle, { passive: true });
	const stopTouch = on(row, 'touchstart', settle, { passive: true });
	watchItems();
	tell();
	return () => {
		stopScroll();
		stopFocusIn();
		stopFocusOut();
		stopWheel();
		stopPointer();
		stopTouch();
		resizes.disconnect();
		changes.disconnect();
	};
}
