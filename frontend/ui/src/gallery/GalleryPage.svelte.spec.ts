import { page } from 'vitest/browser';
import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { createRawSnippet } from 'svelte';
import GalleryPage from './GalleryPage.svelte';

function body(text: string) {
	return createRawSnippet(() => ({ render: () => `<p>${text}</p>` }));
}

function headingTexts(): readonly (string | null)[] {
	return page
		.getByRole('heading')
		.elements()
		.map((heading) => heading.textContent);
}

describe('GalleryPage', () => {
	it('renders nothing when no region is given', () => {
		render(GalleryPage, {});
		expect(headingTexts()).toEqual([]);
	});

	it('renders only the regions given', async () => {
		render(GalleryPage, { keyboard: body('keys'), tests: body('checks') });
		expect(headingTexts()).toEqual(['Keyboard', 'Tests']);
		await expect.element(page.getByText('keys')).toBeVisible();
		expect(page.getByText('States').elements()).toHaveLength(0);
	});

	it('renders all five regions in order whatever order they are passed in', () => {
		render(GalleryPage, {
			tests: body('t'),
			motion: body('m'),
			keyboard: body('k'),
			example: body('e'),
			states: body('s')
		});
		expect(headingTexts()).toEqual(['States', 'Live example', 'Keyboard', 'Motion', 'Tests']);
	});

	it('names each region by its heading', async () => {
		render(GalleryPage, { states: body('s'), example: body('e') });
		await expect.element(page.getByRole('region', { name: 'States' })).toBeVisible();
		await expect.element(page.getByRole('region', { name: 'Live example' })).toBeVisible();
	});
});
