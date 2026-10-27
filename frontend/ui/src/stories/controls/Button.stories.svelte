<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import { expect, fn } from 'storybook/test';
	import { Button, ICON_NAMES } from '@zurfur/ui';
	import { actionWords, queueLink } from '../fixtures/actions.ts';

	/** What a template here reads from the story's args. */
	interface Pressable {
		readonly onPress?: (() => void) | undefined;
	}

	const { Story } = defineMeta({
		title: 'Controls/Button',
		component: Button,
		tags: ['autodocs', 'simple'],
		args: { onPress: fn() },
		argTypes: {
			variant: { control: 'select', options: ['primary', 'secondary', 'quiet', 'destructive'] },
			size: { control: 'inline-radio', options: ['md', 'sm'] },
			icon: { control: 'select', options: [...ICON_NAMES] },
			iconAfter: { control: 'select', options: [...ICON_NAMES] },
			busy: { control: 'boolean' },
			disabled: { control: 'boolean' },
			href: { table: { disable: true } },
			testid: { table: { disable: true } },
			type: { table: { disable: true } }
		},
		parameters: {
			layout: 'centered',
			docs: {
				description: {
					component: `A simple component: the one button, for an action. Its words are a verb and its object. Four looks: Primary is the view's one main action, Secondary is the everyday one, Quiet is for Cancel and other light actions, Destructive is for what deletes or can't be undone. Given a safe address instead of an action, it becomes a link that looks the same. While Busy it keeps its words and its focus, and ignores presses.

Keyboard:

- Tab: reaches it
- Enter: presses it
- Space: presses it`
				}
			}
		}
	});
</script>

{#snippet sizes(args: Pressable)}
	<Button
		onPress={() => {
			args.onPress?.();
		}}>{actionWords.send}</Button
	>
	<Button
		size="sm"
		onPress={() => {
			args.onPress?.();
		}}>{actionWords.send}</Button
	>
{/snippet}

{#snippet withIcons(args: Pressable)}
	<Button
		icon="send"
		onPress={() => {
			args.onPress?.();
		}}>{actionWords.send}</Button
	>
	<Button
		iconAfter="chevron-right"
		onPress={() => {
			args.onPress?.();
		}}>{actionWords.next}</Button
	>
	<Button
		variant="quiet"
		icon="plus"
		onPress={() => {
			args.onPress?.();
		}}>{actionWords.addReference}</Button
	>
{/snippet}

{#snippet asLink(_args: Pressable)}
	{#if queueLink !== undefined}
		<Button href={queueLink}>{actionWords.openQueue}</Button>
	{/if}
{/snippet}

<Story
	name="Primary"
	args={{ variant: 'primary' }}
	play={async ({ args, canvas, userEvent }) => {
		await userEvent.click(canvas.getByRole('button', { name: actionWords.send }));
		await expect(args.onPress).toHaveBeenCalledTimes(1);
		await expect(args.onPress).toHaveBeenCalledWith();
	}}>{actionWords.send}</Story
>

<Story name="Secondary" args={{ variant: 'secondary' }}>{actionWords.addReference}</Story>

<Story name="Quiet" args={{ variant: 'quiet' }}>{actionWords.cancel}</Story>

<Story name="Destructive" args={{ variant: 'destructive' }}>{actionWords.deleteSketch}</Story>

<Story name="Sizes" template={sizes} />

<Story name="With an Icon" template={withIcons} />

<Story
	name="Disabled"
	args={{ disabled: true }}
	play={async ({ args, canvas, userEvent }) => {
		const button = canvas.getByRole('button', { name: actionWords.send });
		await expect(button).toBeDisabled();
		await userEvent.click(button);
		await expect(args.onPress).not.toHaveBeenCalled();
	}}>{actionWords.send}</Story
>

<Story
	name="As a Link"
	template={asLink}
	play={async ({ canvas }) => {
		const link = canvas.getByRole('link', { name: actionWords.openQueue });
		await expect(link).toHaveAttribute('href', '/den/commissions');
	}}
/>

<Story
	name="Busy"
	args={{ busy: true, variant: 'primary' }}
	play={async ({ args, canvas, userEvent }) => {
		const button = canvas.getByRole('button', { name: actionWords.send });
		await expect(button).toHaveAttribute('aria-busy', 'true');
		await userEvent.click(button);
		await expect(args.onPress).not.toHaveBeenCalled();
	}}>{actionWords.send}</Story
>

<Story
	name="Keyboard"
	play={async ({ args, canvas, userEvent }) => {
		await userEvent.tab();
		const button = canvas.getByRole('button', { name: actionWords.send });
		await expect(button).toHaveFocus();
		await userEvent.keyboard('{Enter}');
		await userEvent.keyboard(' ');
		await expect(args.onPress).toHaveBeenCalledTimes(2);
	}}>{actionWords.send}</Story
>

<Story
	name="Focused"
	play={async ({ canvas, userEvent }) => {
		await userEvent.tab();
		await expect(canvas.getByRole('button', { name: actionWords.send })).toHaveFocus();
	}}>{actionWords.send}</Story
>

<Story name="Playground" args={{ variant: 'primary' }}>{actionWords.send}</Story>
