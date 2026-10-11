<script lang="ts">
	import type { Snippet } from 'svelte';
	import Icon from './Icon.svelte';

	/**
	 * A calm message where content would be: an optional title, the message,
	 * and an optional action below it.
	 */
	let {
		title,
		message,
		testid,
		action
	}: {
		title?: string | undefined;
		message: string;
		testid?: string | undefined;
		action?: Snippet | undefined;
	} = $props();
</script>

<div class="empty-state" data-testid={testid}>
	{#if title !== undefined}
		<p class="empty-state__title">{title}</p>
	{/if}
	<p class="empty-state__message">
		<Icon name="info" />
		<span>{message}</span>
	</p>
	{#if action !== undefined}
		<div class="empty-state__action">{@render action()}</div>
	{/if}
</div>

<style>
	.empty-state {
		padding: var(--space-4) 0;
		color: var(--color-text-muted);
	}

	.empty-state__title {
		margin-bottom: var(--space-2);
		color: var(--color-text);
		font-size: var(--text-lg);
		font-weight: var(--text-weight-bold);
	}

	.empty-state__message {
		display: flex;
		align-items: flex-start;
		gap: var(--space-2);
	}

	.empty-state__action {
		margin-top: var(--space-4);
	}
</style>
