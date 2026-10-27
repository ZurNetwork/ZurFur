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
			active: { control: 'boolean' },
			labelShown: { control: 'boolean' },
			testid: { table: { disable: true } }
		},
		parameters: {
			layout: 'centered',
			docs: {
				description: {
					component: `A simple component: a ring that turns while something loads. As a status it stays on the page, and holds the ring and its words only while Active is on: mount it once and turn Active on when loading starts, so screen readers hear the words. It can show the words beside the ring. As a glyph it is the ring alone, hidden from screen readers, for inside a button or a row that already says what is going on. It stops under Still and reduced motion.`
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
	args={{ kind: 'status', label: spinnerWords.loading, active: true }}
	play={async ({ canvas }) => {
		await expect(canvas.getByRole('status')).toHaveTextContent(spinnerWords.loading);
	}}
/>

<Story
	name="With Its Words"
	args={{ kind: 'status', label: spinnerWords.loading, active: true, labelShown: true }}
	play={async ({ canvas }) => {
		const status = canvas.getByRole('status');
		await expect(status).toHaveTextContent(spinnerWords.loading);
		await expect(canvas.getByText(spinnerWords.loading)).toBeVisible();
	}}
/>

<Story
	name="Not Loading"
	args={{ kind: 'status', label: spinnerWords.loading, active: false }}
	play={async ({ canvas }) => {
		await expect(canvas.getByRole('status').textContent.trim()).toBe('');
	}}
/>

<Story name="As a Glyph" args={{ kind: 'glyph' }} />

<Story name="Moves and Sounds" template={motion} />
