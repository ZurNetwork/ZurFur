<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import { expect } from 'storybook/test';
	import { Spinner } from '@zurfur/ui';
	import GalleryMotion from '../../gallery/GalleryMotion.svelte';
	import { spinnerWords } from '../fixtures/surfaces.ts';

	const { Story } = defineMeta({
		title: 'Surfaces and Feedback/Spinner',
		component: Spinner,
		tags: ['autodocs', 'simple'],
		argTypes: {
			kind: { control: 'inline-radio', options: ['status', 'glyph'] },
			labelShown: { control: 'boolean' },
			testid: { table: { disable: true } }
		},
		parameters: {
			layout: 'centered',
			docs: {
				description: {
					component: `A simple component: a ring that turns while something loads. As a status it carries the words for screen readers, and can show them beside the ring. As a glyph it is the ring alone, hidden from screen readers, for inside a button or a row that already says what is going on. It stops under Still and reduced motion.`
				}
			}
		}
	});
</script>

{#snippet motion(_args: unknown)}
	<Spinner kind="glyph" />
	<GalleryMotion moves={['loop']} sounds={[]} />
{/snippet}

<Story
	name="Default"
	args={{ kind: 'status', label: spinnerWords.loading }}
	play={async ({ canvas }) => {
		await expect(canvas.getByRole('status')).toHaveTextContent(spinnerWords.loading);
	}}
/>

<Story
	name="With Its Words"
	args={{ kind: 'status', label: spinnerWords.loading, labelShown: true }}
	play={async ({ canvas }) => {
		const status = canvas.getByRole('status');
		await expect(status).toHaveTextContent(spinnerWords.loading);
		await expect(canvas.getByText(spinnerWords.loading)).toBeVisible();
	}}
/>

<Story name="As a Glyph" args={{ kind: 'glyph' }} />

<Story name="Moves and Sounds" template={motion} />
