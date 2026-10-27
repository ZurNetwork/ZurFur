<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import { expect } from 'storybook/test';
	import { Skeleton } from '@zurfur/ui';
	import GalleryMotion from '../../gallery/GalleryMotion.svelte';

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
					component: `A simple component: a blank in the shape of what is on its way, so the page does not jump when it lands. Lines draws text bars, the last one shorter, and a count that is not a whole number from 1 to 12 draws one bar. Rectangle fills the width of its parent, for a picture. Circle is for an avatar. It breathes while it waits, stops under Still and reduced motion, and is hidden from screen readers.`
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

<Story name="Moves and Sounds" template={motion} />
