import { page } from 'vitest/browser';
import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import type { CardEntry, OpenEntry } from '$lib/api/den';
import { denHref, denName, denType, nameOrdinal } from '$lib/types/brand';
import NodeHeader from './NodeHeader.svelte';

/** An open folder named "Test 1", numbered `ordinal`. */
function twin(ordinal: number | undefined): OpenEntry {
	return {
		view: 'open',
		href: denHref('/den/c/b'),
		name: denName('Test 1'),
		ordinal: nameOrdinal(ordinal),
		type: denType('commission'),
		kind: 'directory',
		mounted: false,
		ownLevel: 'private',
		contentNotShown: false,
		removed: undefined
	};
}

describe('NodeHeader: equal names', () => {
	it('titles a numbered node "name (n)", the number outside the isolate', async () => {
		render(NodeHeader, { node: twin(2) });

		const heading = page.getByRole('heading', { level: 1 });
		await expect.element(heading).toHaveTextContent('Test 1 (2)');
		expect(heading.element().querySelector('bdi')?.textContent).toBe('Test 1');
	});

	it('titles a numbered card "name (n)" too', async () => {
		const card: CardEntry = {
			view: 'card',
			name: denName('Test 1'),
			ordinal: nameOrdinal(2),
			type: denType('commission'),
			mounted: false
		};
		render(NodeHeader, { node: card });

		await expect.element(page.getByRole('heading', { level: 1 })).toHaveTextContent('Test 1 (2)');
	});

	it('titles a node with no number by its name alone, and the root "My Den"', () => {
		const plain = render(NodeHeader, { node: twin(undefined) });
		expect(plain.container.querySelector('h1')?.textContent.trim()).toBe('Test 1');
		plain.unmount();

		const root = render(NodeHeader, { node: twin(2), root: true });
		expect(root.container.querySelector('h1')?.textContent.trim()).toBe('My Den');
	});
});
