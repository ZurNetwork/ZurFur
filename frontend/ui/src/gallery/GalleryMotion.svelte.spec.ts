import { page } from 'vitest/browser';
import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import GalleryMotion from './GalleryMotion.svelte';

/** Each term in the list under `heading`, beside its line. */
function listUnder(container: HTMLElement, heading: string): readonly (readonly string[])[] {
	const part = [...container.querySelectorAll('h3')].find((h3) => h3.textContent === heading);
	const terms = [...(part?.parentElement?.querySelectorAll('dt') ?? [])];
	return terms.map((term) => [term.textContent, term.nextElementSibling?.textContent ?? '']);
}

describe('GalleryMotion', () => {
	it('lists the moves and the cues the control calls, each with its line', () => {
		const { container } = render(GalleryMotion, { moves: ['squish'], sounds: ['toggle'] });
		expect(listUnder(container, 'Moves')).toEqual([
			['squish', 'Squishes a little when pressed, then springs back']
		]);
		expect(listUnder(container, 'Sounds')).toEqual([['toggle', 'A short high blip']]);
	});

	it('has a line for every move and every cue', () => {
		const { container } = render(GalleryMotion, {
			moves: ['squish', 'note'],
			sounds: ['open', 'close', 'toggle']
		});
		const lines = [...listUnder(container, 'Moves'), ...listUnder(container, 'Sounds')];
		expect(lines).toHaveLength(5);
		expect(lines.filter(([, line]) => line === '')).toEqual([]);
	});

	it('says None for an empty list', () => {
		const { container } = render(GalleryMotion, { moves: ['squish'], sounds: [] });
		const none = [...container.querySelectorAll('p')].filter((p) => p.textContent === 'None');
		expect(none).toHaveLength(1);
		expect(listUnder(container, 'Sounds')).toEqual([]);
	});

	it('says the gallery is still and silent for now', async () => {
		render(GalleryMotion, { moves: [], sounds: [] });
		await expect
			.element(page.getByText('The gallery stays still and silent until the motion layer arrives.'))
			.toBeVisible();
	});
});
