<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import { expect, fn } from 'storybook/test';
	import { Pin } from '@zurfur/ui';
	import PlaceholderArt from '../fixtures/PlaceholderArt.svelte';
	import { artPins, onePin, pinLabels, placeholderArtWords } from '../fixtures/surfaces.ts';

	/** What a template here reads from the story's args. */
	interface Shown {
		readonly open?: boolean | undefined;
		readonly onPress?: (() => void) | undefined;
	}

	const { Story } = defineMeta({
		title: 'Surfaces and Feedback/Pin',
		component: Pin,
		render: onArt,
		tags: ['autodocs', 'simple'],
		args: { open: false, onPress: fn() },
		argTypes: {
			open: { control: 'boolean' }
		},
		parameters: {
			layout: 'centered',
			docs: {
				description: {
					component: `A simple component: a numbered mark on a piece of art, for a note about one spot. It sits on its point, given as fractions of the art's width and height, and a point off the art draws no pin. Its name is read out instead of the mark, so the name says the number too, and the spot in words, such as Pin 1: Left ear, since where it sits on the art is not read out. Open means its thread is showing.

Keyboard:

- Tab: reaches it
- Enter, Space: presses it`
				}
			}
		}
	});
</script>

{#snippet onArt(args: Shown)}
	<PlaceholderArt label={placeholderArtWords.label}>
		{#each artPins as pin, index (pin.mark)}
			<Pin
				point={pin.point}
				mark={pin.mark}
				label={pin.label}
				open={index === 1 && args.open === true}
				onPress={() => {
					args.onPress?.();
				}}
			/>
		{/each}
	</PlaceholderArt>
{/snippet}

{#snippet single(args: Shown)}
	<PlaceholderArt label={placeholderArtWords.label}>
		{#each onePin as pin (pin.mark)}
			<Pin
				point={pin.point}
				mark={pin.mark}
				label={pin.label}
				open={args.open === true}
				onPress={() => {
					args.onPress?.();
				}}
			/>
		{/each}
	</PlaceholderArt>
{/snippet}

<Story name="On Art" />

<Story
	name="Open"
	args={{ open: true }}
	template={single}
	play={async ({ canvas }) => {
		await expect(canvas.getByRole('button', { name: pinLabels.ear })).toHaveAttribute(
			'aria-expanded',
			'true'
		);
	}}
/>

<Story
	name="Keyboard"
	template={single}
	play={async ({ args, canvas, userEvent }) => {
		await userEvent.tab();
		await expect(canvas.getByRole('button', { name: pinLabels.ear })).toHaveFocus();
		await userEvent.keyboard('{Enter}');
		await userEvent.keyboard(' ');
		await expect(args.onPress).toHaveBeenCalledTimes(2);
	}}
/>

<Story
	name="Focused"
	template={single}
	play={async ({ canvas, userEvent }) => {
		await userEvent.tab();
		await expect(canvas.getByRole('button', { name: pinLabels.ear })).toHaveFocus();
	}}
/>
