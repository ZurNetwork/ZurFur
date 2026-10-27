import { page } from 'vitest/browser';
import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import type { ComponentProps } from 'svelte';
import RecordingEnvironment from '../../checks/testing/RecordingEnvironment.svelte';
import Skeleton from './Skeleton.svelte';
import { SKELETON_LINES_CAP } from './choices.ts';
import '../styles/index.ts';

type SeamCall = Parameters<ComponentProps<typeof RecordingEnvironment>['onRecord']>[0];

/** The one element carrying `testid`. */
function byTestId(testid: string): HTMLElement {
	const [found] = page.getByTestId(testid).elements();
	if (!(found instanceof HTMLElement)) throw new Error(`nothing carries ${testid}`);
	return found;
}

/** The bars inside a container. */
function barsIn(container: HTMLElement): readonly Element[] {
	return [...container.querySelectorAll('.zf-skeleton')];
}

describe('Skeleton', () => {
	it('draws that many text bars as lines', () => {
		const { container } = render(Skeleton, { shape: 'lines', lines: 3 });
		const bars = barsIn(container);
		expect(bars).toHaveLength(3);
		for (const bar of bars) expect(bar.classList.contains('zf-skeleton--text')).toBe(true);
		expect(container.querySelector('.zf-skeleton-lines')).not.toBeNull();
	});

	it('makes the last of several bars shorter than the rest', () => {
		const { container } = render(Skeleton, { shape: 'lines', lines: 3 });
		const [first, , last] = barsIn(container);
		if (first === undefined || last === undefined) throw new Error('the bars are missing');
		const widthOf = (bar: Element) => bar.getBoundingClientRect().width;
		expect(widthOf(last)).toBeGreaterThan(0);
		expect(widthOf(last)).toBeLessThan(widthOf(first));
	});

	it('keeps a single line full width', () => {
		const { container } = render(Skeleton, { shape: 'lines', lines: 1 });
		const [only] = barsIn(container);
		const wrap = container.querySelector('.zf-skeleton-lines');
		if (only === undefined || wrap === null) throw new Error('the bar is missing');
		expect(only.getBoundingClientRect().width).toBeCloseTo(wrap.getBoundingClientRect().width, 0);
	});

	it.each([1, 2, SKELETON_LINES_CAP])('draws %d lines', (lines) => {
		const { container } = render(Skeleton, { shape: 'lines', lines });
		expect(barsIn(container)).toHaveLength(lines);
	});

	it.each([
		0,
		-2,
		1.5,
		SKELETON_LINES_CAP + 1,
		5000,
		Number.NaN,
		Number.POSITIVE_INFINITY,
		'3',
		undefined
	])('draws one bar for the count %o', (stranger) => {
		const { container } = render(Skeleton, { shape: 'lines', lines: stranger as number });
		expect(barsIn(container)).toHaveLength(1);
	});

	it('draws a rectangle', () => {
		render(Skeleton, { shape: 'rect', testid: 'shape' });
		const shape = byTestId('shape');
		expect(shape.classList.contains('zf-skeleton--rect')).toBe(true);
	});

	it('draws a circle', () => {
		render(Skeleton, { shape: 'circle', testid: 'shape' });
		const shape = byTestId('shape');
		expect(shape.classList.contains('zf-skeleton--circle')).toBe(true);
	});

	it.each([
		['lines', { shape: 'lines', lines: 2, testid: 'shape' }],
		['rect', { shape: 'rect', testid: 'shape' }],
		['circle', { shape: 'circle', testid: 'shape' }]
	] as const)('is hidden from the accessibility tree as %s', (_name, props) => {
		const { container } = render(Skeleton, props);
		expect(byTestId('shape').getAttribute('aria-hidden')).toBe('true');
		expect(page.getByRole('status').elements()).toEqual([]);
		expect(container.textContent.trim()).toBe('');
	});

	it('records the breathe loop on the single shape', () => {
		const calls: SeamCall[] = [];
		render(
			Skeleton,
			{ shape: 'rect', testid: 'shape' },
			{ wrapper: RecordingEnvironment, wrapperProps: { onRecord: (call) => calls.push(call) } }
		);
		expect(calls).toEqual([
			{ kind: 'move', move: 'loop', args: ['breathe'], element: byTestId('shape') }
		]);
	});

	it('records the breathe loop on each bar of the lines', () => {
		const calls: SeamCall[] = [];
		const { container } = render(
			Skeleton,
			{ shape: 'lines', lines: 3 },
			{ wrapper: RecordingEnvironment, wrapperProps: { onRecord: (call) => calls.push(call) } }
		);
		const bars = barsIn(container);
		expect(calls).toEqual(
			bars.map((element) => ({ kind: 'move', move: 'loop', args: ['breathe'], element }))
		);
		expect(calls).toHaveLength(3);
	});

	it('draws without an environment, and stays still', () => {
		const { container } = render(Skeleton, { shape: 'circle' });
		expect(barsIn(container)).toHaveLength(1);
	});
});
