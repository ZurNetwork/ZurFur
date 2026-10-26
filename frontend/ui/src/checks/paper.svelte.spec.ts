import { afterEach, describe, expect, it } from 'vitest';
import { userEvent } from 'vitest/browser';
import '../lib/styles/index.ts';

const mounted: HTMLElement[] = [];

/** Puts markup on the page and hands back the element inside it that carries `id`. */
function mount(markup: string, id: string): HTMLElement {
	const container = document.createElement('div');
	container.innerHTML = markup;
	document.body.append(container);
	mounted.push(container);
	const found = container.querySelector<HTMLElement>(`#${id}`);
	if (found === null) throw new Error(`the markup has no #${id}`);
	return found;
}

afterEach(() => {
	for (const container of mounted.splice(0)) container.remove();
});

const peek = '<div class="pile"><div id="peek" class="peek pk2"></div></div>';

describe('a peek of the second sheet', () => {
	it.each([
		['dark', `<div data-skin="dark">${peek}</div>`, '12px'],
		['light', `<div data-skin="light">${peek}</div>`, '8px'],
		[
			'light inside dark',
			`<div data-skin="dark"><div data-skin="light">${peek}</div></div>`,
			'8px'
		],
		[
			'dark inside light',
			`<div data-skin="light"><div data-skin="dark">${peek}</div></div>`,
			'12px'
		]
	])('sits down by its skin: %s', (_label, markup, top) => {
		const element = mount(markup, 'peek');
		expect(getComputedStyle(element).top).toBe(top);
	});
});

describe('a page moving in', () => {
	it('lasts 0.48s', () => {
		const element = mount(
			'<div data-skin="dark"><div id="page" class="pagein"></div></div>',
			'page'
		);
		const style = getComputedStyle(element);
		expect({ name: style.animationName, duration: style.animationDuration }).toEqual({
			name: 'pagein',
			duration: '0.48s'
		});
	});

	it('does not animate under Still', () => {
		const markup =
			'<div data-skin="dark" data-motion="still"><div id="page" class="pagein"></div></div>';
		const element = mount(markup, 'page');
		expect(getComputedStyle(element).animationName).toBe('none');
	});
});

describe('a hovered placed button', () => {
	const button = '<button id="button" class="placed" type="button">Send</button>';

	it('rises 1px', async () => {
		const element = mount(`<div data-skin="dark">${button}</div>`, 'button');
		await userEvent.hover(element);
		await expect.poll(() => getComputedStyle(element).translate).toBe('0px -1px');
	});

	it('stays put under Still', async () => {
		const element = mount(`<div data-skin="dark" data-motion="still">${button}</div>`, 'button');
		await userEvent.hover(element);
		expect(element.matches(':hover')).toBe(true);
		expect(getComputedStyle(element).translate).not.toContain('-1px');
	});
});
