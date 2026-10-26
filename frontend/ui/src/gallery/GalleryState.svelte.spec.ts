import { page } from 'vitest/browser';
import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { createRawSnippet } from 'svelte';
import GalleryState from './GalleryState.svelte';

describe('GalleryState', () => {
	it('wraps its content in an element carrying the state name', () => {
		const children = createRawSnippet(() => ({ render: () => '<span>inside</span>' }));
		render(GalleryState, { name: 'busy', children });
		const wrapper = page.getByText('inside').element().closest('[data-gallery-state]');
		expect(wrapper?.getAttribute('data-gallery-state')).toBe('busy');
	});

	it('gives each state its own name', () => {
		const children = createRawSnippet(() => ({ render: () => '<span>x</span>' }));
		render(GalleryState, { name: 'disabled', children });
		expect(document.querySelectorAll('[data-gallery-state="disabled"]')).toHaveLength(1);
		expect(document.querySelectorAll('[data-gallery-state="busy"]')).toHaveLength(0);
	});
});
