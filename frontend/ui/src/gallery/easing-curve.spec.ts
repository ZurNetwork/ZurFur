import { describe, expect, it } from 'vitest';
import { parseEasing, plotOf } from './easing-curve';

describe('parseEasing', () => {
	it('reads a cubic bezier', () => {
		expect(parseEasing('cubic-bezier(0.34, 1.56, 0.64, 1)')).toEqual({
			kind: 'bezier',
			x1: 0.34,
			y1: 1.56,
			x2: 0.64,
			y2: 1
		});
	});

	it('reads whole numbers and a negative control point', () => {
		expect(parseEasing('cubic-bezier(0,-1,1,2)')).toEqual({
			kind: 'bezier',
			x1: 0,
			y1: -1,
			x2: 1,
			y2: 2
		});
	});

	it('reads linear', () => {
		expect(parseEasing(' linear ')).toEqual({ kind: 'linear' });
	});

	it.each([
		[''],
		['ease-in'],
		['cubic-bezier(0.1, 0.2, 0.3)'],
		['cubic-bezier(a, b, c, d)'],
		['steps(4)']
	])('cannot plot %j', (text) => {
		expect(parseEasing(text)).toEqual({ kind: 'unplottable' });
	});
});

describe('plotOf', () => {
	it('draws linear as one straight line', () => {
		const plot = plotOf({ kind: 'linear' });
		expect(plot?.curve).toBe('M0 100 L100 0');
		expect(plot?.diagonal).toBe(plot?.curve);
	});

	it('draws a bezier from the bottom left to the top right', () => {
		const plot = plotOf({ kind: 'bezier', x1: 0.2, y1: 0, x2: 0, y2: 1 });
		expect(plot?.curve).toBe('M0 100 C20 100 0 0 100 0');
	});

	it('makes room above the box for an overshoot', () => {
		const plot = plotOf({ kind: 'bezier', x1: 0.34, y1: 1.56, x2: 0.64, y2: 1 });
		expect(plot?.viewBox).toBe('-6 -62 112 168');
	});

	it('draws nothing for an easing it cannot read', () => {
		expect(plotOf({ kind: 'unplottable' })).toBeUndefined();
	});
});
