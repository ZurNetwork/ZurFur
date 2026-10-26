import { page } from 'vitest/browser';
import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import GalleryNav from './GalleryNav.svelte';
import { isSlug, type PageMeta } from './page-meta';

function entry(slug: string, title: string, group: PageMeta['group']): readonly PageMeta[] {
	return isSlug(slug) ? [{ slug, title, group }] : [];
}

describe('GalleryNav', () => {
	it('says there are no components when it has no entries', async () => {
		render(GalleryNav, { entries: [] });
		await expect.element(page.getByText('No components yet.')).toBeVisible();
		expect(page.getByRole('link').elements()).toHaveLength(0);
	});

	it('links each entry under its group, groups in order', async () => {
		const entries = [
			...entry('toast', 'Toast', 'Surfaces and feedback'),
			...entry('button', 'Button', 'Controls'),
			...entry('card', 'Card', 'Controls')
		];
		render(GalleryNav, { entries });
		await expect
			.element(page.getByRole('link', { name: 'Button' }))
			.toHaveAttribute('href', '/components/button');
		await expect
			.element(page.getByRole('link', { name: 'Toast' }))
			.toHaveAttribute('href', '/components/toast');
		const headings = page.getByRole('heading').elements();
		expect(headings.map((heading) => heading.textContent)).toEqual([
			'Controls',
			'Surfaces and feedback'
		]);
		const links = page.getByRole('link').elements();
		expect(links.map((link) => link.textContent)).toEqual(['Button', 'Card', 'Toast']);
		expect(page.getByText('No components yet.').elements()).toHaveLength(0);
	});
});
