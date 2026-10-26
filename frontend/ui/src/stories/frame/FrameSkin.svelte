<script lang="ts">
	import type { Snippet } from 'svelte';
	import { MediaQuery } from 'svelte/reactivity';
	import type { Skin } from '@zurfur/ui';
	import type { SkinChoice } from './globals.ts';

	interface Props {
		/** The Skin menu's choice; Follow the OS is resolved to the skin the page shows. */
		readonly choice: SkinChoice;
		/** Rendered once, and handed the skin that is showing. */
		readonly children: Snippet<[Skin]>;
	}

	let { choice, children }: Props = $props();

	/** The OS asks for light; anything else, no preference included, shows dark, as the tokens read it. */
	const osPrefersLight = new MediaQuery('(prefers-color-scheme: light)', false);

	const osSkin: Skin = $derived(osPrefersLight.current ? 'light' : 'dark');
	const skin: Skin = $derived(choice === 'os' ? osSkin : choice);
</script>

{@render children(skin)}
