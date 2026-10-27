import { describe, expect, it } from 'vitest';
import {
	revealPosition,
	rowEdges,
	stepPosition,
	type ItemFit,
	type RowMeasure
} from './step-row.ts';

/**
 * A row 250 wide holding five items 100 wide with 4 between them: they start at
 * 0, 104, 208, 312 and 416, and the row can go 266 along.
 */
const fits: readonly ItemFit[] = [0, 104, 208, 312, 416].map((start) => ({
	atStart: start,
	atEnd: start + 100 - 250
}));

/** The five-item row, scrolled to `position`. */
function rowAt(position: number): RowMeasure {
	return { position, furthest: 266, fits };
}

describe('rowEdges', () => {
	it.each([
		['at the start', 0, { atStart: true, atEnd: false }],
		['within a pixel of the start', 1, { atStart: true, atEnd: false }],
		['between the ends', 100, { atStart: false, atEnd: false }],
		['within a pixel of the far end', 265, { atStart: false, atEnd: true }],
		['at the far end', 266, { atStart: false, atEnd: true }]
	])('says %s', (_label, position, expected) => {
		expect(rowEdges(rowAt(position))).toEqual(expected);
	});

	it('says both when every item fits', () => {
		expect(rowEdges({ position: 0, furthest: 0, fits })).toEqual({ atStart: true, atEnd: true });
	});
});

describe('stepPosition', () => {
	it.each([
		['from the start', 0, 104],
		['from an item’s start', 104, 208],
		['from a little short of an item’s start', 103.5, 208],
		['from a little past an item’s start', 105, 208],
		['from between two items', 150, 208],
		['onto the far end when the next item starts past it', 208, 266],
		['and stays at the far end', 266, 266]
	])('goes forward one item %s', (_label, position, expected) => {
		expect(stepPosition('forward', rowAt(position))).toBe(expected);
	});

	it.each([
		['from the far end', 266, 208],
		['from an item’s start', 208, 104],
		['from within a pixel past an item’s start', 209, 104],
		['from a little more past an item’s start', 212, 208],
		['from between two items', 150, 104],
		['to the start', 104, 0],
		['and stays at the start', 0, 0]
	])('goes back one item %s', (_label, position, expected) => {
		expect(stepPosition('back', rowAt(position))).toBe(expected);
	});

	it('steps through every item, one each, and no further than the ends', () => {
		const forward: number[] = [];
		let position = 0;
		for (let count = 0; count < 6; count += 1) {
			position = stepPosition('forward', rowAt(position));
			forward.push(position);
		}
		expect(forward).toEqual([104, 208, 266, 266, 266, 266]);
		const back: number[] = [];
		for (let count = 0; count < 5; count += 1) {
			position = stepPosition('back', rowAt(position));
			back.push(position);
		}
		expect(back).toEqual([208, 104, 0, 0, 0]);
	});

	it('stays put when there is nothing to scroll', () => {
		const still: RowMeasure = { position: 0, furthest: 0, fits };
		expect(stepPosition('forward', still)).toBe(0);
		expect(stepPosition('back', still)).toBe(0);
	});
});

describe('revealPosition', () => {
	it('leaves an item that already lies whole inside', () => {
		expect(revealPosition(0, rowAt(0))).toBeUndefined();
		expect(revealPosition(1, rowAt(0))).toBeUndefined();
		expect(revealPosition(2, rowAt(104))).toBeUndefined();
	});

	it('brings an item cut off at the start to the start', () => {
		expect(revealPosition(1, rowAt(150))).toBe(104);
		expect(revealPosition(0, rowAt(266))).toBe(0);
	});

	it('brings an item cut off at the end to the first item start that shows it whole', () => {
		expect(revealPosition(2, rowAt(0))).toBe(104);
		expect(revealPosition(3, rowAt(0))).toBe(208);
	});

	it('goes no further than the row does', () => {
		expect(revealPosition(4, rowAt(0))).toBe(266);
		expect(revealPosition(4, rowAt(104))).toBe(266);
	});

	it('does nothing for an item that is not there', () => {
		expect(revealPosition(5, rowAt(0))).toBeUndefined();
		expect(revealPosition(-1, rowAt(0))).toBeUndefined();
	});
});
