<script module lang="ts">
	import { defineMeta, type StoryContext } from '@storybook/addon-svelte-csf';
	import FrameSkin from '../frame/FrameSkin.svelte';
	import { parseSkinChoice } from '../frame/globals.ts';
	import PaperPage from './PaperPage.svelte';

	type Args = Record<string, never>;

	const { Story } = defineMeta({
		title: 'Tokens and Paper/Paper',
		parameters: { layout: 'fullscreen' }
	});
</script>

<Story name="Page">
	{#snippet template(args: Args, context: StoryContext<Args>)}
		<FrameSkin choice={parseSkinChoice(context.globals.skin)}>
			{#snippet children(skin)}
				<PaperPage {...args} {skin} />
			{/snippet}
		</FrameSkin>
	{/snippet}
</Story>
