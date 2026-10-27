<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import { expect, fn, userEvent } from 'storybook/test';
	import { Tabs } from '@zurfur/ui';
	import { LONG_TABS, PLAIN_TABS, PROFILE_TABS, TABS_LABEL } from '../fixtures/fields.ts';
	import type { TabsArgs } from './args.ts';
	import TabsHolder from './TabsHolder.svelte';

	const { Story } = defineMeta({
		title: 'Controls/Tabs',
		component: Tabs,
		render: template,
		tags: ['autodocs'],
		args: {
			label: TABS_LABEL,
			tabs: PROFILE_TABS,
			value: 'gallery',
			onChange: fn()
		},
		parameters: {
			docs: {
				description: {
					component: `A simple component: one row of tabs that switches the views of a profile or an open item. Each tab can carry a small icon and a muted count. Only the tab showing has its panel on the page. It keeps no copy of the choice, so what shows is always what its parent hands it.

Keyboard:

- Tab: reaches the tab showing
- ← and →: move to the previous or next tab and show it
- Home and End: move to the first or last tab and show it`
				}
			}
		}
	});
</script>

{#snippet template(args: TabsArgs)}
	<TabsHolder label={args.label} tabs={args.tabs} value={args.value} onChange={args.onChange} />
{/snippet}

<Story name="Playground" />

<Story name="Plain" args={{ tabs: PLAIN_TABS }} />

<Story name="Second Tab Showing" args={{ value: 'comms' }} />

<Story name="Many Tabs" args={{ tabs: LONG_TABS }} />

<Story
	name="Pointer"
	play={async ({ canvas, args }) => {
		await userEvent.click(canvas.getByRole('tab', { name: 'Comms 2' }));
		await expect(args.onChange).toHaveBeenCalledWith('comms');
		await expect(canvas.getByRole('tab', { name: 'Comms 2' })).toHaveAttribute(
			'aria-selected',
			'true'
		);
		await expect(canvas.getByRole('tabpanel', { name: 'Comms 2' })).toHaveTextContent(
			'Comms would show here.'
		);
	}}
/>

<Story
	name="Keyboard"
	play={async ({ canvas, args }) => {
		await userEvent.tab();
		await expect(canvas.getByRole('tab', { name: 'Gallery 24' })).toHaveFocus();
		await userEvent.keyboard('{ArrowRight}');
		await expect(args.onChange).toHaveBeenLastCalledWith('comms');
		await expect(canvas.getByRole('tab', { name: 'Comms 2' })).toHaveFocus();
		await expect(canvas.getByRole('tabpanel', { name: 'Comms 2' })).toBeVisible();
		await userEvent.keyboard('{End}');
		await expect(args.onChange).toHaveBeenLastCalledWith('ocs');
		await expect(canvas.getByRole('tab', { name: 'OCs' })).toHaveFocus();
		await userEvent.keyboard('{ArrowRight}');
		await expect(args.onChange).toHaveBeenLastCalledWith('gallery');
		await userEvent.keyboard('{ArrowLeft}');
		await expect(args.onChange).toHaveBeenLastCalledWith('ocs');
		await userEvent.keyboard('{Home}');
		await expect(args.onChange).toHaveBeenLastCalledWith('gallery');
		await expect(canvas.getByRole('tab', { name: 'Gallery 24' })).toHaveAttribute(
			'aria-selected',
			'true'
		);
		await expect(canvas.getByRole('tabpanel', { name: 'Gallery 24' })).toBeVisible();
	}}
/>

<Story
	name="Focused"
	play={async ({ canvas }) => {
		await userEvent.tab();
		await expect(canvas.getByRole('tab', { name: 'Gallery 24' })).toHaveFocus();
	}}
/>
