<script lang="ts">
	import type { Session } from '$lib/api/session';

	/**
	 * Who is signed in: their picture (or a plain circle when the profile has
	 * none) and their handle in the monospace face, falling back to the DID
	 * when the profile didn't resolve.
	 */
	let { session }: { session: Session } = $props();

	const shown = $derived(session.handle ?? session.did);
</script>

<span class="avatar">
	{#if session.avatarUrl !== undefined}
		<img
			class="avatar__picture"
			data-testid="session-avatar"
			src={session.avatarUrl}
			alt=""
			width="28"
			height="28"
		/>
	{:else}
		<span class="avatar__picture avatar__picture--blank" aria-hidden="true"></span>
	{/if}
	<span class="avatar__handle" data-testid="session-handle">{shown}</span>
</span>

<style>
	.avatar {
		display: inline-flex;
		align-items: center;
		gap: var(--space-2);
		min-width: 0;
	}

	.avatar__picture {
		flex: none;
		width: var(--space-7);
		height: var(--space-7);
		border-radius: var(--radius-round);
		object-fit: cover;
	}

	.avatar__picture--blank {
		background: var(--color-surface-raised);
		border: var(--border-width) solid var(--color-border);
	}

	.avatar__handle {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		font-family: var(--font-mono);
		font-size: var(--text-sm);
	}
</style>
