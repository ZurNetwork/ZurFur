<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import Toggle from '$lib/components/ui/Toggle.svelte';
	import type { DenHref } from '$lib/types/brand';

	/**
	 * The "View" group under the tree: "Show archived and deactivated". The
	 * flag rides only in the address; the server hands both links (on and
	 * off), so nothing here builds one. With the page's script it is a switch
	 * that loads the other link in place, keeping focus on it; before the
	 * script runs it is a plain link to the other address.
	 */
	let {
		includeDeleted,
		flagLinks
	}: {
		includeDeleted: boolean;
		flagLinks: { readonly on: DenHref; readonly off: DenHref };
	} = $props();

	const titleId = $props.id();
	let enhanced = $state(false);
	onMount(() => {
		enhanced = true;
	});

	const LABEL = 'Show archived and deactivated';
	const otherHref = $derived(includeDeleted ? flagLinks.off : flagLinks.on);

	function flip(next: boolean): void {
		const target = next ? flagLinks.on : flagLinks.off;
		// eslint-disable-next-line svelte/no-navigation-without-resolve -- a DenHref is built (and resolved) by the server's Den path builder
		void goto(target, { keepFocus: true, noScroll: true });
	}
</script>

<div class="view-options" role="group" aria-labelledby={titleId}>
	<p class="view-options__title" id={titleId}>View</p>
	{#if enhanced}
		<Toggle checked={includeDeleted} label={LABEL} onchange={flip} testid="show-archived" />
	{:else}
		<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -- a DenHref is built (and resolved) by the server's Den path builder -->
		<a class="view-options__link" href={otherHref} data-testid="show-archived-link"
			>{includeDeleted ? 'Hide archived and deactivated' : LABEL}</a
		>
	{/if}
</div>

<style>
	.view-options {
		margin: var(--space-4) var(--space-2) 0;
		padding: var(--space-3) var(--space-2) 0;
		border-top: var(--border-width) solid var(--color-border);
	}

	.view-options__title {
		margin-bottom: var(--space-1);
		color: var(--color-text-muted);
		font-size: var(--text-xs);
		font-weight: var(--text-weight-medium);
		text-transform: uppercase;
		letter-spacing: 0.04em;
	}

	.view-options__link {
		font-size: var(--text-sm);
	}
</style>
