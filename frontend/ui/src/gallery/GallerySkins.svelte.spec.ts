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

	it.each(['dark', 'light'] as const)('renders one copy, in %s, when only is set', (skin) => {
		render(GallerySkins, { children, only: skin });
		const found = copies().map((copy) => ({
			skin: copy.getAttribute('data-skin'),
			gallerySkin: copy.getAttribute('data-gallery-skin'),
			text: copy.textContent.trim()
		}));
		expect(found).toEqual([{ skin, gallerySkin: skin, text: `inside ${skin}` }]);
	});

	it('has no caption and no columns when only is set', () => {
		render(GallerySkins, { children, only: 'light' });
		expect(page.getByText('Paper dark').elements()).toHaveLength(0);
		expect(page.getByText('Paper light').elements()).toHaveLength(0);
		expect(document.querySelectorAll('figure, figcaption, .gallery-skins')).toHaveLength(0);
	});

	it('paints the one copy in its skin', () => {
		render(GallerySkins, { children, only: 'light' });
		const style = getComputedStyle(copies()[0] ?? document.body);
		expect([style.backgroundColor, style.color]).toEqual(['rgb(236, 229, 211)', 'rgb(45, 43, 43)']);
	});

	it('renders both columns when only is undefined', () => {
		render(GallerySkins, { children, only: undefined });
		expect(copies().map((copy) => copy.getAttribute('data-gallery-skin'))).toEqual([
			'dark',
			'light'
		]);
	});
});
