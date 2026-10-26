import { page } from 'vitest/browser';
import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import Icon from './Icon.svelte';
import { ICON_NAMES, ICONS } from './icons.ts';
import '../styles/index.ts';

/** The one svg an Icon drew inside its container. */
function drawn(container: HTMLElement): SVGSVGElement {
	const found = container.querySelector('svg');
	if (found === null) throw new Error('the icon drew no svg');
	return found;
}

describe('Icon', () => {
	it('is an svg hidden from screen readers and off the tab order', () => {
		const { container } = render(Icon, { name: 'clock' });
		const svg = drawn(container);
		expect(svg.getAttribute('aria-hidden')).toBe('true');
		expect(svg.getAttribute('focusable')).toBe('false');
	});

	it('has no role and no accessible name', () => {
		const { container } = render(Icon, { name: 'clock' });
		const svg = drawn(container);
		expect(svg.getAttribute('role')).toBeNull();
		expect(svg.getAttribute('aria-label')).toBeNull();
		expect(svg.getAttribute('aria-labelledby')).toBeNull();
		expect(svg.querySelector('title')).toBeNull();
		expect(page.getByRole('img').elements()).toEqual([]);
		expect(page.getByRole('graphics-document').elements()).toEqual([]);
	});

	it.each([
		[undefined, '16px'],
		['sm', '14px'],
		['md', '16px'],
		['lg', '20px']
	] as const)('follows the size token for %s', (size, pixels) => {
		const { container } = render(
			Icon,
			size === undefined ? { name: 'clock' } : { name: 'clock', size }
		);
		const style = getComputedStyle(drawn(container));
		expect([style.width, style.height]).toEqual([pixels, pixels]);
	});

	it.each([
		[undefined, '1.75px'],
		['regular', '1.75px'],
		['heavy', '2.5px']
	] as const)('strokes at the %s weight', (weight, width) => {
		const { container } = render(
			Icon,
			weight === undefined ? { name: 'check' } : { name: 'check', weight }
		);
		expect(getComputedStyle(drawn(container)).strokeWidth).toBe(width);
	});

	it('rounds its caps and joins and draws in the text colour', () => {
		const { container } = render(Icon, { name: 'check' });
		const style = getComputedStyle(drawn(container));
		expect([style.strokeLinecap, style.strokeLinejoin, style.fill]).toEqual([
			'round',
			'round',
			'none'
		]);
	});

	it('takes its colour from the parent through currentColor', () => {
		const { container } = render(Icon, { name: 'clock' });
		const svg = drawn(container);
		container.style.color = 'rgb(10, 20, 30)';
		expect(getComputedStyle(svg).stroke).toBe('rgb(10, 20, 30)');
		container.style.color = 'rgb(200, 100, 50)';
		expect(getComputedStyle(svg).stroke).toBe('rgb(200, 100, 50)');
	});

	it('carries the bundle class', () => {
		const { container } = render(Icon, { name: 'clock' });
		expect(drawn(container).classList.contains('zf-icon')).toBe(true);
	});

	it('draws every shape of every icon, as the element its kind names', () => {
		for (const name of ICON_NAMES) {
			const { container } = render(Icon, { name });
			const tags = [...drawn(container).children].map((child) => child.tagName);
			const wanted = ICONS[name].map((shape) => shape.shape);
			expect(wanted.length, name).toBeGreaterThan(0);
			expect(tags, name).toEqual(wanted);
		}
	});
});
