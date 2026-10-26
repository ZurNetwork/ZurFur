<script module lang="ts">
	import { defineMeta, type StoryContext } from '@storybook/addon-svelte-csf';
	import FrameSkin from '../frame/FrameSkin.svelte';
	import { parseSkinChoice } from '../frame/globals.ts';
	import TypePage from './TypePage.svelte';

	type Args = Record<string, never>;

	const { Story } = defineMeta({
		title: 'Tokens and Paper/Type',
		parameters: { layout: 'fullscreen' }
	});
</script>

<Story name="Page">
	{#snippet template(_args: Args, context: StoryContext<Args>)}
		<FrameSkin choice={parseSkinChoice(context.globals.skin)}>
			{#snippet children(skin)}
				<TypePage {skin} />
			{/snippet}
		</FrameSkin>
	{/snippet}
</Story>
