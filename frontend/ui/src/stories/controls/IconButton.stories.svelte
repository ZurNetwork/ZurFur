<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import { expect, fn, waitFor, within } from 'storybook/test';
	import { IconButton, ICON_NAMES } from '@zurfur/ui';
	import { copyTool, filesTool, hiddenFilesTool } from '../fixtures/actions.ts';

	/** What a template here reads from the story's args. */
	interface Pressable {
		readonly onPress: () => void;
	}

	const { Story } = defineMeta({
		title: 'Controls/Icon Button',
		component: IconButton,
		tags: ['autodocs'],
		args: { icon: copyTool.icon, label: copyTool.label, onPress: fn() },
		argTypes: {
			icon: { control: 'select', options: [...ICON_NAMES] },
			variant: { control: 'inline-radio', options: ['quiet', 'secondary'] },
			size: { control: 'inline-radio', options: ['md', 'sm'] },
			pressed: { control: 'boolean' },
			expanded: { control: 'boolean' },
			disabled: { control: 'boolean' },
			controls: { table: { disable: true } },
			testid: { table: { disable: true } }
		},
		parameters: {
			layout: 'centered',
			docs: {
				description: {
					component: `A simple component: a square button that shows only an icon, for toolbars, rows and close buttons. Its label is its accessible name, and it shows as a tip on hover and on keyboard focus, so say what it does. Quiet sits in toolbars and rows, Secondary has an edge. Pressed is for on and off tools. Expanded is for a button that shows or hides something.

Keyboard:

- Tab: reaches it and shows its name
- Enter: presses it
- Space: presses it
- Esc: hides the name and keeps focus`
				}
			}
		}
	});
</script>

{#snippet sizes(args: Pressable)}
	<IconButton
		icon={copyTool.icon}
		label={copyTool.label}
		onPress={() => {
			args.onPress();
		}}
	/>
	<IconButton
		size="sm"
		icon={filesTool.icon}
		label={filesTool.label}
		onPress={() => {
			args.onPress();
		}}
	/>
{/snippet}

<Story
	name="Quiet"
	args={{ variant: 'quiet' }}
	play={async ({ args, canvas, userEvent }) => {
		await userEvent.click(canvas.getByRole('button', { name: copyTool.label }));
		await expect(args.onPress).toHaveBeenCalledTimes(1);
		await expect(args.onPress).toHaveBeenCalledWith();
	}}
/>

<Story name="Secondary" args={{ variant: 'secondary' }} />

<Story name="Sizes" template={sizes} />

<Story
	name="Pressed"
	args={{ icon: hiddenFilesTool.icon, label: hiddenFilesTool.label, pressed: true }}
	play={async ({ canvas }) => {
		await expect(
			canvas.getByRole('button', { name: hiddenFilesTool.label, pressed: true })
		).toBeVisible();
	}}
/>

<Story
	name="Expanded"
	args={{ icon: filesTool.icon, label: filesTool.label, expanded: false }}
	play={async ({ canvas }) => {
		await expect(canvas.getByRole('button', { name: filesTool.label })).toHaveAttribute(
			'aria-expanded',
			'false'
		);
	}}
/>

<Story
	name="Disabled"
	args={{ disabled: true }}
	play={async ({ args, canvas, userEvent }) => {
		const button = canvas.getByRole('button', { name: copyTool.label });
		await expect(button).toBeDisabled();
		await userEvent.click(button);
		await expect(args.onPress).not.toHaveBeenCalled();
	}}
/>

<Story
	name="Keyboard"
	play={async ({ args, canvas, canvasElement, userEvent }) => {
		const screen = within(canvasElement.ownerDocument.body);
		await userEvent.tab();
		const button = canvas.getByRole('button', { name: copyTool.label });
		await expect(button).toHaveFocus();
		await waitFor(() => expect(screen.getByText(copyTool.label)).toBeVisible());
		await userEvent.keyboard('{Escape}');
		await waitFor(() => expect(screen.queryByText(copyTool.label)).not.toBeInTheDocument());
		await expect(button).toHaveFocus();
		await userEvent.keyboard('{Enter}');
		await userEvent.keyboard(' ');
		await expect(args.onPress).toHaveBeenCalledTimes(2);
	}}
/>

<Story
	name="Focused"
	play={async ({ canvas, canvasElement, userEvent }) => {
		const screen = within(canvasElement.ownerDocument.body);
		await userEvent.tab();
		await expect(canvas.getByRole('button', { name: copyTool.label })).toHaveFocus();
		await waitFor(() => expect(screen.getByText(copyTool.label)).toBeVisible());
	}}
/>

<Story name="Playground" />
