<script module lang="ts">
	import { defineMeta } from '@storybook/addon-svelte-csf';
	import { expect } from 'storybook/test';
	import { VisibilityMarker } from '@zurfur/ui';
	import { visibilityWords } from '../fixtures/surfaces.ts';

	const { Story } = defineMeta({
		title: 'Surfaces and Feedback/Visibility Marker',
		component: VisibilityMarker,
		tags: ['autodocs', 'simple'],
		args: { level: 'private', words: visibilityWords },
		argTypes: {
			level: { control: 'inline-radio', options: ['private', 'listed', 'public'] },
			compact: { control: 'boolean' },
			words: { table: { disable: true } },
			testid: { table: { disable: true } }
		},
		parameters: {
			layout: 'centered',
			docs: {
				description: {
					component: `A simple component: who can see a thing. Private is only you, Listed is anyone with the link, Public is everyone. Each level has its own icon and its own word, so the two always agree. Compact keeps just the icon on the page, and the word is still read out, once. A level it does not know draws nothing.`
				}
			}
		}
	});
</script>

<Story name="Private" args={{ level: 'private' }} />

<Story name="Listed" args={{ level: 'listed' }} />

<Story name="Public" args={{ level: 'public' }} />

<Story
	name="Compact"
	args={{ level: 'private', compact: true }}
	play={async ({ canvas }) => {
		await expect(canvas.getAllByText(visibilityWords.private)).toHaveLength(1);
	}}
/>
