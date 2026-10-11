<script lang="ts">
	import type { Trail } from '$lib/api/trail';
	import Icon from './Icon.svelte';
	import PathStepLabel from './PathStepLabel.svelte';

	/**
	 * Breadcrumbs by name. On a desktop every step shows, the last one marked
	 * as the current page; on a phone only the parent shows, as a back link.
	 */
	let { trail }: { trail: Trail } = $props();

	const parent = $derived(trail.length >= 2 ? trail[trail.length - 2] : undefined);
	const parentHref = $derived(parent?.href);
</script>

<nav class="path-bar desktop-only" aria-label="Path" data-testid="path-bar">
	<ol>
		{#each trail as step, index (index)}
			{@const current = index === trail.length - 1}
			<li>
				{#if current || step.href === undefined}
					<span class="path-bar__here" aria-current={current ? 'page' : undefined}
						><PathStepLabel {step} /></span
					>
				{:else}
					<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -- trail links are resolved by their producers (resolve() or the Den path builder) -->
					<a href={step.href}><PathStepLabel {step} /></a>
				{/if}
			</li>
		{/each}
	</ol>
</nav>

{#if parent !== undefined && parentHref !== undefined}
	<nav class="path-bar path-bar--back phone-only" aria-label="Back" data-testid="path-bar-back">
		<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -- trail links are resolved by their producers (resolve() or the Den path builder) -->
		<a href={parentHref}><Icon name="chevron-left" /><PathStepLabel step={parent} /></a>
	</nav>
{/if}

<style>
	.path-bar {
		min-width: 0;
		font-family: var(--font-mono);
		font-size: var(--text-sm);
	}

	ol {
		display: flex;
		flex-wrap: nowrap;
		align-items: center;
		gap: var(--space-1);
		margin: 0;
		padding: 0;
		list-style: none;
		overflow: hidden;
	}

	li {
		display: inline-flex;
		align-items: center;
		min-width: 0;
		white-space: nowrap;
	}

	li + li::before {
		content: '/';
		margin-right: var(--space-1);
		color: var(--color-text-muted);
	}

	li > * {
		overflow: hidden;
		text-overflow: ellipsis;
	}

	a {
		color: var(--color-text-muted);
		text-decoration: none;
	}

	a:hover {
		color: var(--color-text);
		text-decoration: underline;
	}

	.path-bar__here {
		color: var(--color-text);
	}

	.path-bar--back a {
		display: inline-flex;
		align-items: center;
		gap: var(--space-1);
		max-width: 100%;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
</style>
