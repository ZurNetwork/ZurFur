<script lang="ts">
	import type { Snippet } from 'svelte';
	import '@zurfur/ui/styles';
	import GalleryNav from '$gallery/GalleryNav.svelte';
	import SkinSwitch from '$gallery/SkinSwitch.svelte';
	import { registry } from '$gallery/registry';

	interface Props {
		readonly children: Snippet;
	}

	let { children }: Props = $props();
</script>

<svelte:head>
	<title>Zurfur · Component gallery</title>
</svelte:head>

<header>
	<h1 id="gallery-heading">Component gallery</h1>
	<SkinSwitch />
</header>
<nav aria-labelledby="gallery-heading">
	<GalleryNav entries={registry.entries} />
</nav>
<main>
	{@render children()}
</main>
