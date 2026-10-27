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

	/*
	 * A docs page around its stories wears the skin too, so the toggle switches the whole page.
	 * Only Storybook's own docs chrome is painted here, never a story's content.
	 */
	:global(.sbdocs.sbdocs-wrapper) {
		background: var(--color-bg);
	}

	/* The docs page's own words (its title and prose), never the stories inside it. */
	:global(
		.sbdocs.sbdocs-wrapper > .sbdocs.sbdocs-content > :is(h1, h2, h3, h4, h5, h6, p, ul, ol)
	) {
		color: var(--color-text);
	}

	/* The component's description, its lists, and each story's name. */
	:global(
		.sbdocs.sbdocs-wrapper
			> .sbdocs.sbdocs-content
			> div:not(.sb-story, .sb-anchor)
			:is(p, ul, ol, li, code)
	),
	:global(.sbdocs.sbdocs-wrapper .sb-anchor :is(h1, h2, h3, h4, h5, h6, h3 > span)) {
		color: var(--color-text);
	}

	/* A story's canvas box and its toolbar. */
	:global(.sbdocs.sbdocs-wrapper .sbdocs-preview) {
		background: var(--color-bg);
		color: var(--color-text);
		border-color: var(--color-border);
	}

	:global(.sbdocs.sbdocs-wrapper .sbdocs-preview > div:not(.docs-story)) {
		background: var(--color-surface);
		color: var(--color-text-muted);
		box-shadow: 0 -1px 0 0 var(--color-border);
	}

	:global(.sbdocs.sbdocs-wrapper .sbdocs-preview > div:not(.docs-story) :is(button, a, svg)) {
		color: var(--color-text-muted);
	}

	:global(.sbdocs.sbdocs-wrapper .sbdocs-preview-actions button) {
		color: var(--color-text-muted);
	}

	:global(.sbdocs.sbdocs-wrapper .sbdocs-preview-actions button.docblock-code-toggle--expanded) {
		background: var(--color-hover);
		color: var(--color-text);
	}

	/* The source shown by "Show code". */
	:global(.sbdocs.sbdocs-wrapper .sbdocs-preview pre) {
		background: var(--color-surface);
		color: var(--color-text);
	}

	:global(.sbdocs.sbdocs-wrapper .sbdocs-preview pre .token) {
		color: var(--color-text);
	}

	:global(.sbdocs.sbdocs-wrapper .sbdocs-preview pre .token.comment) {
		color: var(--color-text-muted);
	}

	:global(.sbdocs.sbdocs-wrapper .sbdocs-preview pre .token:is(.string, .attr-value)) {
		color: var(--color-accent-text);
	}

	/* The props table: header, rows, descriptions and chips. */
	:global(.sbdocs.sbdocs-wrapper .docblock-argstable) {
		color: var(--color-text);
	}

	:global(.sbdocs.sbdocs-wrapper .docblock-argstable th) {
		background: var(--color-bg);
		color: var(--color-text-muted);
	}

	:global(.sbdocs.sbdocs-wrapper .docblock-argstable th :is(span, button)) {
		color: var(--color-text-muted);
	}

	:global(.sbdocs.sbdocs-wrapper .docblock-argstable td) {
		background: var(--color-surface);
		color: var(--color-text);
		border-color: var(--color-border);
	}

	/* Storybook forces its section row's background with !important, so this must too. */
	:global(.sbdocs.sbdocs-wrapper .docblock-argstable tr[title] td) {
		background: var(--color-surface-raised) !important;
		color: var(--color-text-muted);
	}

	:global(.sbdocs.sbdocs-wrapper .docblock-argstable td :is(span, div, label)) {
		color: inherit;
	}

	:global(.sbdocs.sbdocs-wrapper .docblock-argstable tr[title] td span) {
		background: transparent;
		color: var(--color-text-muted);
	}

	:global(.sbdocs.sbdocs-wrapper .docblock-argstable tr[title] td svg) {
		color: var(--color-text-muted);
	}

	:global(.sbdocs.sbdocs-wrapper .docblock-argstable td code) {
		color: var(--color-text);
	}

	:global(.sbdocs.sbdocs-wrapper .docblock-argstable td :is(a, summary)) {
		color: var(--color-accent-text);
	}

	/* The controls in the table, as far as CSS reaches. */
	:global(.sbdocs.sbdocs-wrapper .docblock-argstable :is(input, textarea, select)) {
		background: var(--color-bg);
		color: var(--color-text);
		box-shadow: 0 0 0 1px var(--color-border);
	}

	:global(.sbdocs.sbdocs-wrapper .docblock-argstable td button:not([tabindex])) {
		background: var(--color-surface-raised);
		color: var(--color-text);
		box-shadow: inset 0 0 0 1px var(--color-border);
	}
</style>
