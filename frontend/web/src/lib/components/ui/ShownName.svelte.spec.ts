import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { nameOrdinal } from '$lib/types/brand';
import ShownName from './ShownName.svelte';

describe('ShownName', () => {
	it('shows the name inside the isolate and " (n)" outside it', () => {
		const { container } = render(ShownName, { name: 'Test 1', ordinal: nameOrdinal(2) });

		const isolate = container.querySelector('bdi');
		expect(isolate?.textContent).toBe('Test 1');
		expect(container.textContent).toBe('Test 1 (2)');
		expect(isolate?.contains(container.querySelector('[data-testid="name-ordinal"]'))).toBe(false);
	});

	it('shows the name alone, with no " (", when it has no number', () => {
		const { container } = render(ShownName, { name: 'Test 1', ordinal: undefined });

		expect(container.querySelector('bdi')?.textContent).toBe('Test 1');
		expect(container.textContent).toBe('Test 1');
		expect(container.textContent).not.toContain(' (');
	});

	it('keeps a right-to-left name inside the isolate, so it can’t swallow the number', () => {
		const name = 'طلب رسم شخصية';
		const { container } = render(ShownName, { name, ordinal: nameOrdinal(3) });

		expect(container.querySelector('bdi')?.textContent).toBe(name);
		expect(container.textContent).toBe(`${name} (3)`);
	});
});
