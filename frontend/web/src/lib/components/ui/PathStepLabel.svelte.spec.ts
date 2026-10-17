import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { nameOrdinal } from '$lib/types/brand';
import PathStepLabel from './PathStepLabel.svelte';

describe('PathStepLabel', () => {
	it('shows a numbered step’s name inside the isolate and its number outside', () => {
		const step = {
			step: 'named',
			label: 'Test 1',
			ordinal: nameOrdinal(2),
			href: undefined
		} as const;
		const { container } = render(PathStepLabel, { step });

		expect(container.querySelector('bdi')?.textContent).toBe('Test 1');
		expect(container.textContent).toBe('Test 1 (2)');
	});

	it('shows a step with no number by its name alone', () => {
		const step = { step: 'named', label: 'Test 1', ordinal: undefined, href: undefined } as const;
		const { container } = render(PathStepLabel, { step });

		expect(container.querySelector('bdi')?.textContent).toBe('Test 1');
		expect(container.textContent).not.toContain(' (');
	});
});
