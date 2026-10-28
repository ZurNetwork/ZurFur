import { describe, expect, it } from 'vitest';
import { pinPointFrom, pinPointOf } from './pin-point.ts';

/** Numbers no point may hold. */
const outOfRange = [
	-0.0001,
	1.0001,
	Number.NaN,
	Number.POSITIVE_INFINITY,
	Number.NEGATIVE_INFINITY
];

/** Values that are not numbers, as a page could still hand them over at run time. */
const notNumbers: readonly unknown[] = ['0.5', '', true, undefined, {}, [], 0n, Symbol('x')];

describe('pinPointOf', () => {
	it.each([
		[0, 0],
		[1, 1],
		[0.5, 0.25]
	])('accepts (%d, %d)', (x, y) => {
		expect(pinPointOf(x, y)).toEqual({ x, y });
	});

	it('turns -0 into 0 on each axis', () => {
		const point = pinPointOf(-0, -0);
		expect(point).toBeDefined();
		expect(Object.is(point?.x, 0)).toBe(true);
		expect(Object.is(point?.y, 0)).toBe(true);
	});

	it('gives a point that cannot be changed', () => {
		const point = pinPointOf(0.5, 0.5);
		expect(Object.isFrozen(point)).toBe(true);
	});

	it.each(outOfRange)('refuses %d across', (x) => {
		expect(pinPointOf(x, 0.5)).toBeUndefined();
	});

	it.each(outOfRange)('refuses %d down', (y) => {
		expect(pinPointOf(0.5, y)).toBeUndefined();
	});

	it.each(notNumbers)('refuses %o across', (x) => {
		expect(pinPointOf(x as number, 0.5)).toBeUndefined();
	});

	it.each(notNumbers)('refuses %o down', (y) => {
		expect(pinPointOf(0.5, y as number)).toBeUndefined();
	});
});

describe('pinPointFrom', () => {
	it('gives a fresh point from a good one', () => {
		const given = { x: 0.25, y: 0.75 };
		const point = pinPointFrom(given);
		expect(point).toEqual(given);
		expect(point).not.toBe(given);
	});

	it.each([
		undefined,
		0,
		'x',
		[],
		{},
		{ x: 0.5 },
		{ y: 0.5 },
		{ x: 2, y: 0.5 },
		{ x: 0.5, y: '1' }
	])('refuses %o', (value) => {
		expect(pinPointFrom(value)).toBeUndefined();
	});

	it('reads each number once, so a getter cannot change it after the check', () => {
		let reads = 0;
		const shifty = {
			get x() {
				reads += 1;
				return reads === 1 ? 0.5 : 5;
			},
			y: 0.5
		};
		expect(pinPointFrom(shifty)).toEqual({ x: 0.5, y: 0.5 });
		expect(reads).toBe(1);
	});
});
