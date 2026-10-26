<script lang="ts">
	import type { Snippet } from 'svelte';
	import { MOTION_ATTRIBUTE, SKIN_ATTRIBUTE, type MotionMode } from '@zurfur/ui';
	import FrameSkin from './FrameSkin.svelte';
	import type { SkinChoice } from './globals.ts';

	interface Props {
		/** The Skin menu's choice; Follow the OS leaves `<html>` without a skin. */
		readonly skin: SkinChoice;
		/** The Motion menu's choice; Lively leaves `<html>` without a motion mode. */
		readonly motion: MotionMode;
		/** The story. */
		readonly children: Snippet;
	}

	let { skin, motion, children: story }: Props = $props();

	$effect(() => {
		const root = document.documentElement;
		if (skin === 'os') root.removeAttribute(SKIN_ATTRIBUTE);
		else root.setAttribute(SKIN_ATTRIBUTE, skin);
		return () => {
			root.removeAttribute(SKIN_ATTRIBUTE);
		};
	});

	$effect(() => {
		const root = document.documentElement;
		if (motion === 'still') root.setAttribute(MOTION_ATTRIBUTE, 'still');
		else root.removeAttribute(MOTION_ATTRIBUTE);
		return () => {
			root.removeAttribute(MOTION_ATTRIBUTE);
		};
	});

	const stillMotion: MotionMode | undefined = $derived(motion === 'still' ? 'still' : undefined);
</script>

<FrameSkin choice={skin}>
	{#snippet children(shown)}
		<div class="story-frame" data-skin={shown} data-motion={stillMotion}>
			{@render story()}
		</div>
	{/snippet}
</FrameSkin>

<style>
	.story-frame {
		background: var(--color-bg);
		color: var(--color-text);
	}

	/* A docs page around its stories wears the skin too, so the toggle switches the whole page. */
	:global(.sbdocs.sbdocs-wrapper) {
		background: var(--color-bg);
	}

	/* The docs page's own words (its title and prose), never the stories inside it. */
	:global(
		.sbdocs.sbdocs-wrapper > .sbdocs.sbdocs-content > :is(h1, h2, h3, h4, h5, h6, p, ul, ol)
	) {
		color: var(--color-text);
	}
</style>
