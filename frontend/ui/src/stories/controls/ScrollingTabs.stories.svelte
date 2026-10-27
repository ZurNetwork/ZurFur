<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import { expect, fn, userEvent, waitFor } from 'storybook/test';
	import { ScrollingTabs } from '@zurfur/ui';
	import { SCROLLING_TABS, TABS_LABEL, TAB_STEP_WORDS } from '../fixtures/fields.ts';
	import type { ScrollingTabsArgs } from './args.ts';
	import ScrollingTabsHolder from './ScrollingTabsHolder.svelte';

	const { Story } = defineMeta({
		title: 'Controls/Scrolling Tabs',
		component: ScrollingTabs,
		render: template,
		tags: ['autodocs', 'composite'],
		args: {
			label: TABS_LABEL,
			tabs: SCROLLING_TABS,
			value: 'gallery',
			stepWords: TAB_STEP_WORDS,
			onChange: fn()
		},
		parameters: {
			docs: {
				description: {
					component: `A composite component: Tabs and two icon buttons, one at each end of the row. When the tabs outgrow the room, each button moves the row by one tab, never a free scroll, and stops at its end. A step never changes the tab showing. It keeps no copy of the choice, so what shows is always what its parent hands it.

Keyboard:

- Tab: reaches the tab showing, then the forward step
- ← and →: move to the previous or next tab and show it; the tab comes fully into view
- Home and End: move to the first or last tab and show it
- Enter or Space on a step: moves the row by one tab`
				}
			}
		}
	});

	/** How far the list is scrolled when `item` sits at its start, read from where the tab is drawn. */
	function startOf(list: HTMLElement, item: HTMLElement): number {
		const padding = Number.parseFloat(getComputedStyle(list).scrollPaddingLeft);
		const drawn = item.getBoundingClientRect().left - list.getBoundingClientRect().left;
		return drawn + list.scrollLeft - padding;
	}

	/** Whether `item` lies whole inside the list, to a pixel. */
	function isWhollyIn(list: HTMLElement, item: HTMLElement): boolean {
		const around = list.getBoundingClientRect();
		const inside = item.getBoundingClientRect();
		return inside.left >= around.left - 1 && inside.right <= around.right + 1;
	}

	/** The tabs of the list, in order. */
	function tabsOf(list: HTMLElement): readonly HTMLElement[] {
		return [...list.children].filter((child) => child instanceof HTMLElement);
	}

	/** Presses `button` until it is disabled, at most `presses` times. */
	async function pressUntilDisabled(button: HTMLElement, presses: number): Promise<void> {
		if (presses === 0 || button.hasAttribute('disabled')) return;
		await userEvent.click(button);
		await pressUntilDisabled(button, presses - 1);
	}
</script>

{#snippet template(args: ScrollingTabsArgs)}
	<ScrollingTabsHolder
		label={args.label}
		tabs={args.tabs}
		value={args.value}
		stepWords={args.stepWords}
		onChange={args.onChange}
	/>
{/snippet}

<Story name="Playground" />

<Story
	name="Step Forward"
	play={async ({ canvas, args }) => {
		const list = canvas.getByRole('tablist');
		const [first, second, third] = tabsOf(list);
		await userEvent.click(canvas.getByRole('button', { name: TAB_STEP_WORDS.forward }));
		await waitFor(async () => {
			await expect(Math.abs(list.scrollLeft - startOf(list, second ?? list))).toBeLessThanOrEqual(
				1
			);
		});
		await userEvent.click(canvas.getByRole('button', { name: TAB_STEP_WORDS.forward }));
		await waitFor(async () => {
			await expect(Math.abs(list.scrollLeft - startOf(list, third ?? list))).toBeLessThanOrEqual(1);
		});
		await expect(first).toHaveAttribute('aria-selected', 'true');
		await expect(args.onChange).not.toHaveBeenCalled();
	}}
/>

<Story
	name="Step Back"
	play={async ({ canvas, args }) => {
		const list = canvas.getByRole('tablist');
		const [, second, third] = tabsOf(list);
		await userEvent.click(canvas.getByRole('button', { name: TAB_STEP_WORDS.forward }));
		await userEvent.click(canvas.getByRole('button', { name: TAB_STEP_WORDS.forward }));
		await waitFor(async () => {
			await expect(Math.abs(list.scrollLeft - startOf(list, third ?? list))).toBeLessThanOrEqual(1);
		});
		await userEvent.click(canvas.getByRole('button', { name: TAB_STEP_WORDS.back }));
		await waitFor(async () => {
			await expect(Math.abs(list.scrollLeft - startOf(list, second ?? list))).toBeLessThanOrEqual(
				1
			);
		});
		await expect(canvas.getByRole('tab', { name: 'Gallery 24' })).toHaveAttribute(
			'aria-selected',
			'true'
		);
		await expect(args.onChange).not.toHaveBeenCalled();
	}}
/>

<Story
	name="At the Ends"
	play={async ({ canvas }) => {
		const list = canvas.getByRole('tablist');
		const back = canvas.getByRole('button', { name: TAB_STEP_WORDS.back });
		const forward = canvas.getByRole('button', { name: TAB_STEP_WORDS.forward });
		await expect(back).toBeDisabled();
		await expect(forward).toBeEnabled();
		await pressUntilDisabled(forward, SCROLLING_TABS.length);
		await waitFor(async () => {
			await expect(forward).toBeDisabled();
		});
		await expect(back).toBeEnabled();
		await expect(
			Math.abs(list.scrollLeft - (list.scrollWidth - list.clientWidth))
		).toBeLessThanOrEqual(1);
		await pressUntilDisabled(back, SCROLLING_TABS.length);
		await waitFor(async () => {
			await expect(back).toBeDisabled();
		});
		await expect(forward).toBeEnabled();
		await expect(list.scrollLeft).toBeLessThanOrEqual(1);
	}}
/>

<Story
	name="Keyboard"
	play={async ({ canvas, args }) => {
		const list = canvas.getByRole('tablist');
		const items = tabsOf(list);
		await userEvent.tab();
		await expect(canvas.getByRole('tab', { name: 'Gallery 24' })).toHaveFocus();
		for (const item of items.slice(1)) {
			await userEvent.keyboard('{ArrowRight}');
			await expect(item).toHaveFocus();
			await expect(item).toHaveAttribute('aria-selected', 'true');
			await waitFor(async () => {
				await expect(isWhollyIn(list, item)).toBe(true);
			});
		}
		await expect(args.onChange).toHaveBeenLastCalledWith('wishlist');
		await userEvent.keyboard('{Home}');
		await expect(args.onChange).toHaveBeenLastCalledWith('gallery');
		await waitFor(async () => {
			await expect(list.scrollLeft).toBeLessThanOrEqual(1);
		});
		await userEvent.keyboard('{End}');
		await expect(args.onChange).toHaveBeenLastCalledWith('wishlist');
		await waitFor(async () => {
			await expect(isWhollyIn(list, items[items.length - 1] ?? list)).toBe(true);
		});
	}}
/>

<Story
	name="Steps Never Choose"
	play={async ({ canvas, args }) => {
		await userEvent.tab();
		await userEvent.tab();
		await expect(canvas.getByRole('button', { name: TAB_STEP_WORDS.forward })).toHaveFocus();
		await userEvent.keyboard('{Enter}');
		await expect(args.onChange).not.toHaveBeenCalled();
		await expect(canvas.getByRole('tab', { name: 'Gallery 24' })).toHaveAttribute(
			'aria-selected',
			'true'
		);
	}}
/>
