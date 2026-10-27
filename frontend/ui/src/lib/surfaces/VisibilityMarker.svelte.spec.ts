import { page } from 'vitest/browser';
import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import VisibilityMarker from './VisibilityMarker.svelte';
import type { VisibilityLevel, VisibilityWords } from './types.ts';
import '../styles/index.ts';

const words: VisibilityWords = { private: 'Only me', listed: 'By link', public: 'Everyone' };

/** The one element carrying `testid`. */
function byTestId(testid: string): HTMLElement {
	const [found] = page.getByTestId(testid).elements();
	if (!(found instanceof HTMLElement)) throw new Error(`nothing carries ${testid}`);
	return found;
}

describe('VisibilityMarker', () => {
	it.each([
		['private', 'Only me'],
		['listed', 'By link'],
		['public', 'Everyone']
	] as const)('shows the word for the %s level', (level, word) => {
		render(VisibilityMarker, { level, words, testid: 'marker' });
		const marker = byTestId('marker');
		expect(marker.classList.contains('zf-vis')).toBe(true);
		expect(marker.classList.contains(`zf-vis--${level}`)).toBe(true);
		expect(marker.textContent.trim()).toBe(word);
	});

	it('draws a different icon for each level', () => {
		const drawn = (['private', 'listed', 'public'] as const).map((level) => {
			const { container, unmount } = render(VisibilityMarker, { level, words });
			const shape = container.querySelector('svg')?.innerHTML ?? '';
			void unmount();
			return shape;
		});
		expect(new Set(drawn).size).toBe(3);
		for (const shape of drawn) expect(shape).not.toBe('');
	});

	it('hides its icon from screen readers', () => {
		render(VisibilityMarker, { level: 'public', words, testid: 'marker' });
		expect(byTestId('marker').querySelector('svg')?.getAttribute('aria-hidden')).toBe('true');
	});

	it('shows the word as plain text at full size', () => {
		render(VisibilityMarker, { level: 'public', words, testid: 'marker' });
		const word = byTestId('marker').querySelector('span');
		expect(word?.classList.contains('zf-sr')).toBe(false);
	});

	it('compact keeps the icon on its own and speaks its word once', async () => {
		render(VisibilityMarker, { level: 'private', words, compact: true, testid: 'marker' });
		const marker = byTestId('marker');
		expect(marker.classList.contains('zf-vis--compact')).toBe(true);
		const word = marker.querySelector('span');
		expect(word?.classList.contains('zf-sr')).toBe(true);
		expect(word?.textContent).toBe('Only me');
		expect(marker.querySelector('svg')?.getAttribute('aria-hidden')).toBe('true');
		expect(marker.textContent.trim()).toBe('Only me');
		await expect.element(page.getByText('Only me')).toBeInTheDocument();
		expect(page.getByText('Only me').elements()).toHaveLength(1);
	});

	it.each(['hidden', 'Private', '', 'toString', undefined])(
		'draws nothing for the level %j',
		(stranger) => {
			const { container } = render(VisibilityMarker, {
				level: stranger as VisibilityLevel,
				words
			});
			expect(container.querySelector('.zf-vis')).toBeNull();
			expect(container.textContent.trim()).toBe('');
		}
	);

	it('shows no word that was not handed to it', () => {
		const unique: VisibilityWords = { private: 'P-word', listed: 'L-word', public: 'U-word' };
		const { container } = render(VisibilityMarker, {
			level: 'listed',
			words: unique,
			testid: 'marker'
		});
		expect(container.textContent.trim()).toBe('L-word');
	});
});
