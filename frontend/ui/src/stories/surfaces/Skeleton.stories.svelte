<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import { expect } from 'storybook/test';
	import { Skeleton, Spinner } from '@zurfur/ui';
	import GalleryMotion from '../../gallery/GalleryMotion.svelte';
	import { spinnerWords } from '../fixtures/surfaces.ts';

	const { Story } = defineMeta({
		title: 'Surfaces and Feedback/Skeleton',
		component: Skeleton,
		tags: ['autodocs', 'simple'],
		argTypes: {
			shape: { control: 'inline-radio', options: ['lines', 'rect', 'circle'] },
			lines: { control: 'number' },
			testid: { table: { disable: true } }
		},
		parameters: {
			docs: {
				description: {
					component: `A simple component: a blank in the shape of what is on its way, so the page does not jump when it lands. Lines draws text bars, the last one shorter, and a count that is not a whole number from 1 to 12 draws one bar. Rectangle fills the width of its parent, for a picture. Circle is for an avatar. It breathes while it waits, stops under Still and reduced motion, and is hidden from screen readers. Pair it with a status that says what is loading, such as a Spinner status the page mounts once and turns on while it loads.`
				}
			}
		}
	});
</script>

{#snippet motion(_args: unknown)}
	<Skeleton shape="rect" />
	<GalleryMotion moves={['loop']} sounds={[]} />
{/snippet}

<Story
	name="Lines"
	args={{ shape: 'lines', lines: 3, testid: 'blank' }}
	play={async ({ canvas }) => {
		await expect(canvas.getByTestId('blank')).toHaveAttribute('aria-hidden', 'true');
		await expect(canvas.queryByRole('status')).toBeNull();
	}}
/>

<Story name="Rectangle" args={{ shape: 'rect' }} />

<Story name="Circle" args={{ shape: 'circle' }} />

{#snippet withStatus(_args: unknown)}
	<Spinner kind="status" label={spinnerWords.loading} active />
	<Skeleton shape="lines" lines={3} testid="blank" />
{/snippet}

<Story
	name="With a Status"
	template={withStatus}
	play={async ({ canvas }) => {
		await expect(canvas.getByRole('status')).toHaveTextContent(spinnerWords.loading);
		await expect(canvas.getByTestId('blank')).toHaveAttribute('aria-hidden', 'true');
	}}
/>

<Story name="Moves and Sounds" template={motion} />
