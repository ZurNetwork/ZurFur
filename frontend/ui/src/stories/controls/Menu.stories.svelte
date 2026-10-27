<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import { expect, fn, userEvent, waitFor, within } from 'storybook/test';
	import { Menu } from '@zurfur/ui';
	import {
		COMMISSION_ACTIONS,
		ICON_TRIGGER,
		ICON_WORDS_TRIGGER,
		WORDS_TRIGGER
	} from '../fixtures/fields.ts';
	import type { MenuArgs } from './args.ts';
	import MenuHolder from './MenuHolder.svelte';
	import { FLOATING_ROOM } from './room.ts';

	/** Waits until the page takes presses again: a menu left open by the story before has handed it back. */
	async function untilPressable(trigger: HTMLElement): Promise<void> {
		await waitFor(() => expect(trigger).not.toHaveStyle('pointer-events: none'));
	}

	const { Story } = defineMeta({
		title: 'Controls/Menu',
		component: Menu,
		render: template,
		tags: ['autodocs', 'simple'],
		args: {
			trigger: WORDS_TRIGGER,
			items: COMMISSION_ACTIONS,
			open: false,
			onOpenChange: fn(),
			onSelect: fn()
		},
		parameters: {
			docs: {
				story: { height: FLOATING_ROOM },
				description: {
					component: `A simple component: a button that opens a short list of actions. The button shows words, or an icon alone that shows its name in a tip. A row can carry an icon, be switched off, or be marked as one that can’t be undone. The menu is open only when its parent says so.

Keyboard:

- Tab: reaches the button
- Enter, Space or ↓: opens the menu on its first row
- ↑ and ↓: move between rows, skipping rows that are off
- Type a letter: jumps to the row that starts with it
- Enter: chooses the row and closes the menu
- Esc: closes the menu and returns to the button`
				}
			}
		}
	});
</script>

{#snippet template(args: MenuArgs)}
	<MenuHolder
		trigger={args.trigger}
		items={args.items}
		open={args.open}
		onOpenChange={args.onOpenChange}
		onSelect={args.onSelect}
	/>
{/snippet}

<Story name="Playground" />

<Story name="With an Icon" args={{ trigger: ICON_WORDS_TRIGGER }} />

<Story name="Icon Only" args={{ trigger: ICON_TRIGGER }} />

<Story
	name="Opened"
	play={async ({ canvas, canvasElement, args }) => {
		await untilPressable(canvas.getByRole('button', { name: 'Actions' }));
		await userEvent.click(canvas.getByRole('button', { name: 'Actions' }));
		const menu = await within(canvasElement.ownerDocument.body).findByRole('menu');
		await expect(menu).toBeVisible();
		await expect(args.onOpenChange).toHaveBeenCalledWith(true);
		const rows = within(menu).getAllByRole('menuitem');
		await expect(rows.map((row) => row.textContent.trim())).toEqual([
			'Rename',
			'Duplicate',
			'Move to archive',
			'Delete commission'
		]);
		await expect(within(menu).getByRole('menuitem', { name: 'Move to archive' })).toHaveAttribute(
			'aria-disabled',
			'true'
		);
	}}
/>

<Story
	name="Icon Only Opened"
	args={{ trigger: ICON_TRIGGER }}
	play={async ({ canvas, canvasElement }) => {
		const trigger = canvas.getByRole('button', { name: 'More actions' });
		await expect(trigger).toHaveAttribute('aria-haspopup', 'menu');
		await untilPressable(trigger);
		await userEvent.click(trigger);
		const menu = await within(canvasElement.ownerDocument.body).findByRole('menu');
		await expect(menu).toBeVisible();
	}}
/>

<Story
	name="Pointer"
	play={async ({ canvas, canvasElement, args }) => {
		await untilPressable(canvas.getByRole('button', { name: 'Actions' }));
		await userEvent.click(canvas.getByRole('button', { name: 'Actions' }));
		const menu = await within(canvasElement.ownerDocument.body).findByRole('menu');
		await userEvent.click(within(menu).getByRole('menuitem', { name: 'Duplicate' }));
		await expect(args.onSelect).toHaveBeenCalledTimes(1);
		await expect(args.onSelect).toHaveBeenCalledWith('duplicate');
		await expect(args.onOpenChange).toHaveBeenLastCalledWith(false);
		await waitFor(() =>
			expect(within(canvasElement.ownerDocument.body).queryByRole('menu')).toBeNull()
		);
	}}
/>

<Story
	name="Keyboard"
	play={async ({ canvas, canvasElement, args }) => {
		const page = within(canvasElement.ownerDocument.body);
		const trigger = canvas.getByRole('button', { name: 'Actions' });
		await untilPressable(trigger);
		await userEvent.tab();
		await expect(trigger).toHaveFocus();
		await userEvent.keyboard('{Enter}');
		await page.findByRole('menu');
		await waitFor(() => expect(page.getByRole('menuitem', { name: 'Rename' })).toHaveFocus());
		await userEvent.keyboard('{ArrowDown}');
		await expect(page.getByRole('menuitem', { name: 'Duplicate' })).toHaveFocus();
		await userEvent.keyboard('{ArrowDown}');
		await expect(page.getByRole('menuitem', { name: 'Delete commission' })).toHaveFocus();
		await userEvent.keyboard('{Escape}');
		await waitFor(() => expect(page.queryByRole('menu')).toBeNull());
		await expect(trigger).toHaveFocus();
		await expect(args.onSelect).not.toHaveBeenCalled();
		await userEvent.keyboard('{ArrowDown}');
		await page.findByRole('menu');
		await waitFor(() => expect(page.getByRole('menuitem', { name: 'Rename' })).toHaveFocus());
		await userEvent.keyboard('{ArrowDown}');
		await expect(page.getByRole('menuitem', { name: 'Duplicate' })).toHaveFocus();
		await userEvent.keyboard('{Enter}');
		await expect(args.onSelect).toHaveBeenCalledTimes(1);
		await expect(args.onSelect).toHaveBeenCalledWith('duplicate');
		await waitFor(() => expect(page.queryByRole('menu')).toBeNull());
		await expect(trigger).toHaveFocus();
	}}
/>

<Story
	name="Focused"
	play={async ({ canvas }) => {
		await userEvent.tab();
		await expect(canvas.getByRole('button', { name: 'Actions' })).toHaveFocus();
	}}
/>

<Story
	name="Switched Off Row Does Nothing"
	play={async ({ canvas, canvasElement, args }) => {
		await untilPressable(canvas.getByRole('button', { name: 'Actions' }));
		await userEvent.click(canvas.getByRole('button', { name: 'Actions' }));
		const menu = await within(canvasElement.ownerDocument.body).findByRole('menu');
		const archive = within(menu).getByRole('menuitem', { name: 'Move to archive' });
		await userEvent.click(archive, { pointerEventsCheck: 0 });
		await expect(args.onSelect).not.toHaveBeenCalled();
		await expect(menu).toBeVisible();
	}}
/>

<Story
	name="Stays Open After a Click"
	play={async ({ canvas, canvasElement, args }) => {
		const trigger = canvas.getByRole('button', { name: 'Actions' });
		await untilPressable(trigger);
		await userEvent.pointer({ keys: '[MouseLeft>]', target: trigger });
		await userEvent.pointer({ keys: '[/MouseLeft]', target: trigger });
		await new Promise((resolve) => setTimeout(resolve, 300));
		const menu = within(canvasElement.ownerDocument.body).getByRole('menu');
		await expect(menu).toBeVisible();
		await expect(args.onOpenChange).toHaveBeenCalledTimes(1);
		await expect(args.onSelect).not.toHaveBeenCalled();
	}}
/>

<Story
	name="Release on a Row Chooses Nothing"
	play={async ({ canvas, canvasElement, args, userEvent: user }) => {
		const trigger = canvas.getByRole('button', { name: 'Actions' });
		await untilPressable(trigger);
		await user.pointer({ keys: '[MouseLeft>]', target: trigger });
		const menu = await within(canvasElement.ownerDocument.body).findByRole('menu');
		const row = within(menu).getByRole('menuitem', { name: 'Duplicate' });
		await user.pointer({ keys: '[/MouseLeft]', target: row });
		await expect(args.onSelect).not.toHaveBeenCalled();
		await expect(menu).toBeVisible();
	}}
/>
