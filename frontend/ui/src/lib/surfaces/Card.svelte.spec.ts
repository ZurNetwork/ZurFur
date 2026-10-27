import { page } from 'vitest/browser';
import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { createRawSnippet } from 'svelte';
import Card from './Card.svelte';
import type { CardElement } from './types.ts';
import '../styles/index.ts';

/** Words for a Card, given as markup already escaped, so the browser reads them as text. */
function words(escaped: string) {
	return createRawSnippet(() => ({ render: () => `<span>${escaped}</span>` }));
}

/** The one element carrying `testid`. */
function byTestId(testid: string): HTMLElement {
	const [found] = page.getByTestId(testid).elements();
	if (!(found instanceof HTMLElement)) throw new Error(`nothing carries ${testid}`);
	return found;
}

describe('Card', () => {
	it('holds what it is given', async () => {
		render(Card, { children: words('Sketch for Mika') });
		await expect.element(page.getByText('Sketch for Mika')).toBeInTheDocument();
	});

	it('is a div sheet by default, with padding', () => {
		render(Card, { children: words('Sketch'), testid: 'card' });
		const card = byTestId('card');
		expect(card.tagName).toBe('DIV');
		expect(card.classList.contains('obj')).toBe(true);
		expect(card.classList.contains('zf-card')).toBe(true);
		expect(card.classList.contains('zf-card--flush')).toBe(false);
	});

	it('drops its padding when asked', () => {
		render(Card, { children: words('Sketch'), padding: 'none', testid: 'card' });
		expect(byTestId('card').classList.contains('zf-card--flush')).toBe(true);
	});

	it.each([
		['div', 'DIV'],
		['section', 'SECTION'],
		['article', 'ARTICLE'],
		['li', 'LI']
	] as const)('becomes a %s', (as, tag) => {
		render(Card, { children: words('Sketch'), as, testid: 'card' });
		expect(byTestId('card').tagName).toBe(tag);
	});

	it.each(['img', 'a', 'button', 'DIV', '', 'script', 'toString'])(
		'draws a div for the element %j',
		(stranger) => {
			render(Card, { children: words('Sketch'), as: stranger as CardElement, testid: 'card' });
			expect(byTestId('card').tagName).toBe('DIV');
		}
	);

	it('draws a div when no element is given at all', () => {
		render(Card, { children: words('Sketch'), as: undefined, testid: 'card' });
		expect(byTestId('card').tagName).toBe('DIV');
	});

	it('shows no word of its own', () => {
		const { container } = render(Card, { children: words('Sketch'), testid: 'card' });
		expect(container.textContent.trim()).toBe('Sketch');
	});
});
