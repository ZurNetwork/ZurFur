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
 * an item's start; undefined when it already does, or there is no such item.
 */
export function revealPosition(
	index: number,
	{ position, furthest, fits }: RowMeasure
): number | undefined {
	const fit = fits[index];
	if (fit === undefined) return undefined;
	if (position > fit.atStart + NEAR) return within(fit.atStart, furthest);
	if (position >= fit.atEnd - NEAR) return undefined;
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
 * into view; returns what undoes it all. It listens for scrolls and focus,
 * and watches the row and its items for resizes and for items coming and going.
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
	const stopScroll = on(row, 'scroll', tell, { passive: true });
	// A browser scrolls a focused item into view after the focus event, so the reveal waits a frame for that.
	let revealing: number | undefined;
	const stopFocus = on(row, 'focusin', (event) => {
		const focused = event.target;
		if (revealing !== undefined) cancelAnimationFrame(revealing);
		revealing = requestAnimationFrame(() => {
			revealing = undefined;
			const index = itemsOf(row).findIndex(
				(item) => focused instanceof Node && item.contains(focused)
			);
			const target = revealPosition(index, measureRow(row));
			if (target !== undefined) row.scrollLeft = target;
		});
	});
	watchItems();
	tell();
	return () => {
		stopScroll();
		stopFocus();
		if (revealing !== undefined) cancelAnimationFrame(revealing);
		resizes.disconnect();
		changes.disconnect();
	};
}
