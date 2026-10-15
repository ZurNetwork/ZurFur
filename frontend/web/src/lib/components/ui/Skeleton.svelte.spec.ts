import '$lib/styles/tokens.css';
import { page } from 'vitest/browser';
import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import Skeleton from './Skeleton.svelte';

/** The height `--row-height` resolves to on this page, in pixels. */
function rowHeight(): number {
	const probe = document.createElement('div');
	probe.style.height = 'var(--row-height)';
	document.body.append(probe);
	const height = probe.getBoundingClientRect().height;
	probe.remove();
	return height;
}

describe('Skeleton', () => {
	it('stands each placeholder row in for a real row: as tall as --row-height', async () => {
		render(Skeleton, { rows: 3 });
		await expect.element(page.getByTestId('skeleton')).toBeInTheDocument();

		const skeleton = page.getByTestId('skeleton').element();
		const expected = rowHeight();

		expect(expected).toBeGreaterThan(0);
		expect(skeleton.getBoundingClientRect().height).toBeCloseTo(expected * 3, 1);
	});
});
