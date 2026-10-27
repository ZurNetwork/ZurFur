import { page } from 'vitest/browser';
import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { createRawSnippet } from 'svelte';
import ToastStack from './ToastStack.svelte';
import '../styles/index.ts';

/** Words for a ToastStack, given as markup already escaped, so the browser reads them as text. */
function words(escaped: string) {
	return createRawSnippet(() => ({ render: () => `<span>${escaped}</span>` }));
}

/** The one live region inside the rendered output. */
function liveRegion(container: HTMLElement): Element {
	const found = container.querySelector('[aria-live]');
	if (found === null) throw new Error('no live region');
	return found;
}

/** How an element stacks what it holds. */
function stackingOf(element: Element) {
	const style = getComputedStyle(element);
	return {
		display: style.display,
		direction: style.flexDirection,
		align: style.alignItems,
		gap: style.rowGap
	};
}

describe('ToastStack', () => {
	it('is a region named by its label', async () => {
		render(ToastStack, { label: 'Notifications' });
		await expect.element(page.getByRole('region', { name: 'Notifications' })).toBeInTheDocument();
	});

	it('draws its family class on the region', () => {
		const { container } = render(ToastStack, { label: 'Notifications' });
		expect(container.querySelector('section')?.classList.contains('zf-toast-stack')).toBe(true);
	});

	it('lays out its live region by its own class, stacking the toasts like the region', () => {
		const { container } = render(ToastStack, { label: 'Notifications' });
		const live = liveRegion(container);
		expect(live.classList.contains('zf-toast-stack__live')).toBe(true);
		const region = container.querySelector('section');
		expect(region).not.toBeNull();
		if (region === null) return;
		expect(stackingOf(live)).toEqual(stackingOf(region));
		expect(stackingOf(live)).toMatchObject({ display: 'flex', direction: 'column' });
	});

	it('has its polite live region while empty', () => {
		const { container } = render(ToastStack, { label: 'Notifications' });
		const live = liveRegion(container);
		expect(live.getAttribute('aria-live')).toBe('polite');
		expect(live.children).toHaveLength(0);
		expect(container.querySelectorAll('[aria-live]')).toHaveLength(1);
	});

	it('keeps the live region mounted with an empty snippet too', () => {
		const empty = createRawSnippet(() => ({ render: () => '<span></span>' }));
		const { container } = render(ToastStack, { label: 'Notifications', children: empty });
		expect(liveRegion(container).getAttribute('aria-live')).toBe('polite');
	});

	it('holds what it is given inside the live region', () => {
		const { container } = render(ToastStack, {
			label: 'Notifications',
			children: words('Sketch sent')
		});
		expect(liveRegion(container).textContent).toBe('Sketch sent');
		expect(page.getByRole('region', { name: 'Notifications' }).elements()).toHaveLength(1);
	});

	it('names its region by a label written as an image tag, as text, with no image', () => {
		const markup = '<img src=x onerror=alert(1)>';
		const { container } = render(ToastStack, { label: markup });
		expect(container.querySelector('img')).toBeNull();
		expect(container.querySelector('section')?.getAttribute('aria-label')).toBe(markup);
		expect(container.textContent.trim()).toBe('');
	});

	it('shows no word of its own', () => {
		const { container } = render(ToastStack, { label: 'Notifications' });
		expect(container.textContent.trim()).toBe('');
	});

	it('shows only the words it is given', () => {
		const { container } = render(ToastStack, {
			label: 'Notifications',
			children: words('Sketch sent')
		});
		expect(container.textContent.trim()).toBe('Sketch sent');
	});
});
