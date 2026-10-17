import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { nameOrdinal } from '$lib/types/brand';
import TreeItem from './TreeItem.svelte';

describe('TreeItem', () => {
	it('shows a numbered name as "name (n)", the number outside the isolate', () => {
		const { container } = render(TreeItem, {
			look: 'directory',
			name: 'Test 1',
			ordinal: nameOrdinal(2)
		});

		const shown = container.querySelector('.tree-item__name');
		expect(shown?.textContent).toBe('Test 1 (2)');
		expect(shown?.querySelector('bdi')?.textContent).toBe('Test 1');
	});

	it('titles a numbered name with the isolated name and its number', () => {
		const { container } = render(TreeItem, {
			look: 'cantOpen',
			name: 'Test 1',
			ordinal: nameOrdinal(2)
		});

		const shown = container.querySelector('.tree-item__name');
		expect(shown?.getAttribute('title')).toBe('\u2068Test 1\u2069 (2)');
	});

	it('titles a name with no number by the isolated name alone', () => {
		const { container } = render(TreeItem, { look: 'file', name: 'Test 1', ordinal: undefined });

		const shown = container.querySelector('.tree-item__name');
		expect(shown?.textContent).toBe('Test 1');
		expect(shown?.getAttribute('title')).toBe('\u2068Test 1\u2069');
	});
});
