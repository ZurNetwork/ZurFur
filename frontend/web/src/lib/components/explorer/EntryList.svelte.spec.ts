import { page } from 'vitest/browser';
import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import type { CardEntry, OpenEntry } from '$lib/api/den';
import { denHref, denName, denType, nameOrdinal } from '$lib/types/brand';
import EntryList from './EntryList.svelte';

/** An open file entry at `href`. */
function open(href: string, name: string, extra: Partial<OpenEntry> = {}): OpenEntry {
	return {
		view: 'open',
		href: denHref(href),
		name: denName(name),
		ordinal: undefined,
		type: denType('commission'),
		kind: 'file',
		mounted: false,
		ownLevel: 'private',
		contentNotShown: false,
		removed: undefined,
		...extra
	};
}

/** A card named `name`, numbered `ordinal`. */
function card(name: string, ordinal: number | undefined): CardEntry {
	return {
		view: 'card',
		name: denName(name),
		ordinal: nameOrdinal(ordinal),
		type: denType('commission'),
		mounted: false
	};
}

describe('EntryList: equal names', () => {
	it('shows each later twin as "name (n)", the first plain, a card too', async () => {
		const entries = [
			open('/den/c/a', 'Test 1'),
			open('/den/c/b', 'Test 1', { ordinal: nameOrdinal(2) }),
			card('Test 1', 3)
		];
		render(EntryList, { entries, more: undefined });

		await expect.element(page.getByRole('link', { name: 'Test 1 (2)' })).toBeInTheDocument();
		const rows = [...page.getByTestId('entry-list').element().querySelectorAll('.tree-item__name')];
		expect(rows.map((row) => row.textContent)).toEqual(['Test 1', 'Test 1 (2)', 'Test 1 (3)']);
		expect(rows.map((row) => row.querySelector('bdi')?.textContent)).toEqual([
			'Test 1',
			'Test 1',
			'Test 1'
		]);
	});
});
