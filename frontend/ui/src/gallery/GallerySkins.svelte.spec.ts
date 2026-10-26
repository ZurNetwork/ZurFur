import { page } from 'vitest/browser';
import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { createRawSnippet } from 'svelte';
import GallerySkins from './GallerySkins.svelte';
import type { Skin } from '../lib/styles/environment.ts';
import '../lib/styles/index.ts';

/** Content that prints the skin it was handed. */
const children = createRawSnippet((skin: () => Skin) => ({
	render: () => `<span class="inside">inside ${skin()}</span>`
}));

function copies(): readonly HTMLElement[] {
	return [...document.querySelectorAll<HTMLElement>('[data-gallery-skin]')];
}

describe('GallerySkins', () => {
	it('renders its content twice, once in each skin', () => {
		render(GallerySkins, { children });
		const found = copies().map((copy) => ({
			skin: copy.getAttribute('data-skin'),
			gallerySkin: copy.getAttribute('data-gallery-skin'),
			text: copy.textContent.trim()
		}));
		expect(found).toEqual([
			{ skin: 'dark', gallerySkin: 'dark', text: 'inside dark' },
			{ skin: 'light', gallerySkin: 'light', text: 'inside light' }
		]);
	});

	it('captions each copy outside it', async () => {
		render(GallerySkins, { children });
		for (const caption of ['Paper dark', 'Paper light']) {
			const element = page.getByText(caption, { exact: true });
			await expect.element(element).toBeVisible();
			expect(element.element().closest('[data-gallery-skin]')).toBeNull();
		}
	});

	it('paints each copy in its own skin', () => {
		render(GallerySkins, { children });
		const painted = copies().map((copy) => {
			const style = getComputedStyle(copy);
			return [style.backgroundColor, style.color];
		});
		expect(painted).toEqual([
			['rgb(21, 20, 23)', 'rgb(219, 219, 219)'],
			['rgb(236, 229, 211)', 'rgb(45, 43, 43)']
		]);
	});
});
