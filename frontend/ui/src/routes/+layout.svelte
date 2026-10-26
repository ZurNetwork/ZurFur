<script lang="ts">
	import type { Snippet } from 'svelte';
	import GalleryNav from '$gallery/GalleryNav.svelte';
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
</header>
<nav aria-labelledby="gallery-heading">
	<GalleryNav entries={registry.entries} />
</nav>
<main>
	{@render children()}
</main>
