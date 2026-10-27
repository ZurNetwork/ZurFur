import { afterEach, describe, expect, it, vi } from 'vitest';
import { measureRow, stepRow, watchRow, type RowEdges } from './step-row.ts';

/** The pages the rows are built in, removed after each spec. */
const holders: HTMLElement[] = [];

interface Built {
	readonly holder: HTMLElement;
	readonly row: HTMLElement;
	readonly items: readonly HTMLButtonElement[];
}

/**
 * A row 250 wide holding five buttons 100 wide with 4 between them, inside a
 * holder that can scroll too: the items start at 0, 104, 208, 312 and 416, and the row goes 266 along.
 */
function buildRow(): Built {
	const holder = document.createElement('div');
	holder.style.cssText = 'height: 40px; overflow: auto;';
	const row = document.createElement('div');
	row.style.cssText =
		'display: flex; gap: 4px; width: 250px; overflow-x: auto; scrollbar-width: none;';
	const items = [0, 1, 2, 3, 4].map(() => item());
	row.append(...items);
	const drop = document.createElement('div');
	drop.style.height = '300px';
	holder.append(row, drop);
	document.body.append(holder);
	holders.push(holder);
	return { holder, row, items };
}

/** One 100-wide button for a row. */
function item(): HTMLButtonElement {
	const button = document.createElement('button');
	button.style.cssText = 'flex: none; width: 100px; height: 20px; padding: 0; border: 0;';
	return button;
}

/** The edges a watch has told, in order. */
function recorder(): { readonly told: RowEdges[]; readonly onEdges: (edges: RowEdges) => void } {
	const told: RowEdges[] = [];
	return { told, onEdges: (edges) => told.push(edges) };
}

/** The last edges told. */
function lastOf(told: readonly RowEdges[]): RowEdges | undefined {
	return told[told.length - 1];
}

/** Waits until the row has scrolled to `position`. */
async function scrolledTo(row: HTMLElement, position: number): Promise<void> {
	await vi.waitFor(() => {
		expect(row.scrollLeft).toBe(position);
	});
}

/** Whether `item` lies whole inside `row`, to a pixel. */
function isWhollyIn(row: HTMLElement, item: HTMLElement): boolean {
	const around = row.getBoundingClientRect();
	const inside = item.getBoundingClientRect();
	return inside.left >= around.left - 1 && inside.right <= around.right + 1;
}

/** Waits `ms` milliseconds. */
function pause(ms: number): Promise<void> {
	return new Promise((resolve) => setTimeout(resolve, ms));
}

/** Resolves at the next scroll the row makes, once its other listeners have heard it. */
function nextScroll(row: HTMLElement): Promise<void> {
	return new Promise((resolve) => {
		row.addEventListener(
			'scroll',
			() => {
				resolve();
			},
			{ once: true }
		);
	});
}

afterEach(() => {
	for (const holder of holders.splice(0)) holder.remove();
	vi.restoreAllMocks();
});

describe('measureRow', () => {
	it('reads where the row is, how far it goes and where each item fits', () => {
		const { row } = buildRow();
		const measured = measureRow(row);
		expect(measured.position).toBe(0);
		expect(measured.furthest).toBe(266);
		expect(measured.fits).toEqual(
			[0, 104, 208, 312, 416].map((start) => ({
				atStart: start,
				atEnd: start + 100 - 250
			}))
		);
	});

	it('allows for the scroll padding at both ends', () => {
		const { row } = buildRow();
		row.style.scrollPaddingInline = '10px';
		const [first] = measureRow(row).fits;
		expect(first).toEqual({ atStart: -10, atEnd: -140 });
	});

	it('counts an item from the row whether or not the row is positioned', () => {
		const { row } = buildRow();
		row.style.margin = '0 0 0 30px';
		const loose = measureRow(row).fits.map((fit) => fit.atStart);
		row.style.position = 'relative';
		const positioned = measureRow(row).fits.map((fit) => fit.atStart);
		expect(loose).toEqual([0, 104, 208, 312, 416]);
		expect(positioned).toEqual(loose);
	});

	it('follows the row as it scrolls', () => {
		const { row } = buildRow();
		row.scrollLeft = 120;
		const measured = measureRow(row);
		expect(measured.position).toBe(120);
		expect(measured.fits.map((fit) => fit.atStart)).toEqual([0, 104, 208, 312, 416]);
	});
});

describe('stepRow', () => {
	it('moves the row one item at a time, forward and back', () => {
		const { row } = buildRow();
		const seen: number[] = [];
		for (const side of [
			'forward',
			'forward',
			'forward',
			'forward',
			'back',
			'back',
			'back',
			'back'
		] as const) {
			stepRow(row, side);
			seen.push(row.scrollLeft);
		}
		expect(seen).toEqual([104, 208, 266, 266, 208, 104, 0, 0]);
	});

	it('scrolls the row alone, not the box around it or the page', () => {
		const { holder, row } = buildRow();
		stepRow(row, 'forward');
		stepRow(row, 'forward');
		expect(holder.scrollTop).toBe(0);
		expect(holder.scrollLeft).toBe(0);
		expect(document.scrollingElement?.scrollTop).toBe(0);
	});
});

describe('watchRow', () => {
	it('tells the edges at once, and again as the row scrolls', async () => {
		const { row } = buildRow();
		const { told, onEdges } = recorder();
		const stop = watchRow(row, onEdges);
		expect(lastOf(told)).toEqual({ atStart: true, atEnd: false });
		row.scrollLeft = 100;
		await vi.waitFor(() => {
			expect(lastOf(told)).toEqual({ atStart: false, atEnd: false });
		});
		row.scrollLeft = 266;
		await vi.waitFor(() => {
			expect(lastOf(told)).toEqual({ atStart: false, atEnd: true });
		});
		stop();
	});

	it('tells the edges again as the row is resized', async () => {
		const { row } = buildRow();
		const { told, onEdges } = recorder();
		const stop = watchRow(row, onEdges);
		row.style.width = '600px';
		await vi.waitFor(() => {
			expect(lastOf(told)).toEqual({ atStart: true, atEnd: true });
		});
		row.style.width = '250px';
		await vi.waitFor(() => {
			expect(lastOf(told)).toEqual({ atStart: true, atEnd: false });
		});
		stop();
	});

	it('tells the edges again as an item changes size, or comes or goes', async () => {
		const { row, items } = buildRow();
		const { told, onEdges } = recorder();
		const stop = watchRow(row, onEdges);
		for (const each of items) each.style.width = '40px';
		await vi.waitFor(() => {
			expect(lastOf(told)).toEqual({ atStart: true, atEnd: true });
		});
		items[0]?.style.setProperty('width', '100px');
		const added = item();
		row.append(added);
		await vi.waitFor(() => {
			expect(lastOf(told)).toEqual({ atStart: true, atEnd: false });
		});
		const before = told.length;
		added.style.width = '10px';
		added.remove();
		await vi.waitFor(() => {
			expect(told.length).toBeGreaterThan(before);
		});
		stop();
	});

	it('brings a focused item whole into view, and leaves one already whole', async () => {
		const { holder, row, items } = buildRow();
		const stop = watchRow(row, () => undefined);
		// The browser's own scroll to a focused item is held off, so only the watch moves the row.
		const focus = async (index: number): Promise<void> => {
			items[index]?.focus({ preventScroll: true });
			await vi.waitFor(() => {
				expect(document.activeElement).toBe(items[index]);
			});
		};
		await focus(1);
		await new Promise((resolve) => setTimeout(resolve, 100));
		expect(row.scrollLeft).toBe(0);
		await focus(3);
		await scrolledTo(row, 208);
		await focus(4);
		await scrolledTo(row, 266);
		await focus(0);
		await scrolledTo(row, 0);
		expect(holder.scrollTop).toBe(0);
		stop();
	});

	it('lets the browser’s own focus scroll stand once it shows the item whole', async () => {
		const { holder, row, items } = buildRow();
		const stop = watchRow(row, () => undefined);
		// Chromium centres a hidden item before `focusin` and leaves a partly shown one; the reveal
		// finishes at once, and an item the browser left whole is not moved again.
		for (const index of [3, 4, 0, 2]) {
			const focused = items[index] ?? row;
			focused.focus();
			expect(isWhollyIn(row, focused)).toBe(true);
			const left = row.scrollLeft;
			await pause(100);
			expect(row.scrollLeft).toBe(left);
			expect(isWhollyIn(row, focused)).toBe(true);
		}
		expect(holder.scrollTop).toBe(0);
		stop();
	});

	it('ends a focus on a snapping row with the item whole at an item’s start', async () => {
		const { row, items } = buildRow();
		row.style.scrollSnapType = 'x mandatory';
		for (const each of items) each.style.scrollSnapAlign = 'start';
		const stop = watchRow(row, () => undefined);
		for (const index of [3, 4, 0, 2]) {
			const focused = items[index] ?? row;
			focused.focus();
			await pause(100);
			expect(isWhollyIn(row, focused)).toBe(true);
			expect([0, 104, 208, 266]).toContain(row.scrollLeft);
		}
		stop();
	});

	it('puts the reveal back when a browser scrolls after the focus event', async () => {
		const { row, items } = buildRow();
		const stop = watchRow(row, () => undefined);
		// Heard after the row's own listener, as a browser that scrolls after `focusin` would scroll.
		const lateScroll = (): void => {
			row.scrollLeft = 50;
		};
		document.addEventListener('focusin', lateScroll);
		try {
			items[3]?.focus({ preventScroll: true });
			expect(row.scrollLeft).toBe(50);
			await scrolledTo(row, 208);
		} finally {
			document.removeEventListener('focusin', lateScroll);
		}
		stop();
	});

	it('brings an item wider than the row to its own start, and leaves it there', async () => {
		const { row, items } = buildRow();
		items[1]?.style.setProperty('width', '300px');
		const stop = watchRow(row, () => undefined);
		items[1]?.focus();
		expect(row.scrollLeft).toBe(104);
		await pause(100);
		expect(row.scrollLeft).toBe(104);
		items[1]?.blur();
		items[1]?.focus({ preventScroll: true });
		expect(row.scrollLeft).toBe(104);
		await pause(100);
		expect(row.scrollLeft).toBe(104);
		stop();
	});

	it('leaves a scroll the person makes once a focus has settled', async () => {
		const { row, items } = buildRow();
		const stop = watchRow(row, () => undefined);
		const settled = nextScroll(row);
		items[3]?.focus();
		await settled;
		expect(isWhollyIn(row, items[3] ?? row)).toBe(true);
		row.dispatchEvent(new Event('wheel', { bubbles: true }));
		row.scrollLeft = 0;
		await pause(150);
		expect(row.scrollLeft).toBe(0);
		stop();
	});

	it.each(['wheel', 'pointerdown', 'touchstart'])(
		'leaves a scroll the person starts with %s after a focus that needed no reveal',
		async (input) => {
			const { row, items } = buildRow();
			const stop = watchRow(row, () => undefined);
			items[1]?.focus();
			await pause(100);
			expect(row.scrollLeft).toBe(0);
			row.dispatchEvent(new Event(input, { bubbles: true }));
			row.scrollLeft = 150;
			await pause(150);
			expect(row.scrollLeft).toBe(150);
			stop();
		}
	);

	it('leaves the row alone once stopped: no scroll, resize, change or focus is heard', async () => {
		const { row, items } = buildRow();
		const { told, onEdges } = recorder();
		const remove = vi.spyOn(row, 'removeEventListener');
		const disconnect = vi.spyOn(ResizeObserver.prototype, 'disconnect');
		const disconnectChanges = vi.spyOn(MutationObserver.prototype, 'disconnect');
		const stop = watchRow(row, onEdges);
		disconnect.mockClear();
		stop();
		expect(remove.mock.calls.map(([type]) => type).sort()).toEqual([
			'focusin',
			'focusout',
			'pointerdown',
			'scroll',
			'touchstart',
			'wheel'
		]);
		expect(disconnect).toHaveBeenCalled();
		expect(disconnectChanges).toHaveBeenCalled();
		const heard = told.length;
		row.scrollLeft = 150;
		row.style.width = '600px';
		row.append(item());
		items[4]?.focus();
		await new Promise((resolve) => setTimeout(resolve, 150));
		expect(told.length).toBe(heard);
	});
});
