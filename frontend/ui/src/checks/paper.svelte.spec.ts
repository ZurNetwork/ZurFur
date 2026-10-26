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

describe('a pick row', () => {
	const skins = ['dark', 'light'] as const;
	const transparent = 'rgba(0, 0, 0, 0)';

	/** A pick of `classes` and `attributes`, and beside it a reference painted with `token`, in one skin. */
	function mountPick(
		skin: string,
		classes: string,
		attributes: string,
		token: string
	): [HTMLElement, HTMLElement] {
		const markup = `<div data-skin="${skin}"><button id="pick" class="pick ${classes}" type="button" ${attributes}>Row</button><div id="ref" style="background: var(${token})">ref</div></div>`;
		const pick = mount(markup, 'pick');
		const reference = pick.parentElement?.querySelector<HTMLElement>('#ref');
		if (reference === null || reference === undefined) throw new Error('the markup has no #ref');
		return [pick, reference];
	}

	/** Waits out the transitions the element has started, so its colour is the one it settled on. */
	async function settle(element: HTMLElement): Promise<void> {
		await Promise.all(element.getAnimations().map((animation) => animation.finished));
	}

	const selectedForms = [
		['aria-selected', '', 'aria-selected="true"'],
		['aria-checked', '', 'aria-checked="true"'],
		['aria-current', '', 'aria-current="page"'],
		['aria-pressed', '', 'aria-pressed="true"'],
		['is-on', 'is-on', '']
	] as const;

	describe.each(skins)('in the %s skin', (skin) => {
		it('has a selected tint that differs from the object tint', () => {
			const [, selected] = mountPick(skin, '', '', '--color-selected');
			const [, object] = mountPick(skin, '', '', '--color-object');
			expect(getComputedStyle(selected).backgroundColor).not.toBe(
				getComputedStyle(object).backgroundColor
			);
		});

		it.each(selectedForms)(
			'keeps its selected tint while hovered: %s',
			async (_form, classes, attributes) => {
				const [pick, reference] = mountPick(skin, classes, attributes, '--color-selected');
				await userEvent.hover(pick);
				expect(pick.matches(':hover')).toBe(true);
				await settle(pick);
				expect(getComputedStyle(pick).backgroundColor).toBe(
					getComputedStyle(reference).backgroundColor
				);
			}
		);

		it.each(selectedForms)(
			'keeps its selected tint while focus-visible: %s',
			async (_form, classes, attributes) => {
				const [pick, reference] = mountPick(skin, classes, attributes, '--color-selected');
				pick.focus();
				await userEvent.keyboard('{ArrowDown}');
				expect(pick.matches(':focus-visible')).toBe(true);
				await settle(pick);
				expect(getComputedStyle(pick).backgroundColor).toBe(
					getComputedStyle(reference).backgroundColor
				);
			}
		);

		it.each([
			['unselected', ''],
			['aria-current is "false"', 'aria-current="false"']
		])('goes to the object tint when hovered and %s', async (_label, attributes) => {
			const [pick, reference] = mountPick(skin, '', attributes, '--color-object');
			await userEvent.hover(pick);
			await settle(pick);
			expect(getComputedStyle(pick).backgroundColor).toBe(
				getComputedStyle(reference).backgroundColor
			);
		});

		it.each([
			['locked', 'is-locked', ''],
			['disabled', '', 'disabled'],
			['aria-disabled', '', 'aria-disabled="true"']
		])('does not react to hover when %s', async (_label, classes, attributes) => {
			const [pick] = mountPick(skin, classes, attributes, '--color-object');
			await userEvent.hover(pick);
			await settle(pick);
			expect(getComputedStyle(pick).backgroundColor).toBe(transparent);
		});
	});
});
