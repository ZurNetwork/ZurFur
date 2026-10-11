<script lang="ts">
	import type { Snippet } from 'svelte';
	import { resolve } from '$app/paths';
	import type { Session } from '$lib/api/session';
	import Avatar from '$lib/components/ui/Avatar.svelte';
	import type { FrameSection } from './frame-section';
	import SignOut from './SignOut.svelte';

	/**
	 * The frame's left column: its navigation content, plus — inside the
	 * phone drawer only — the section links, who is signed in and Sign out,
	 * which the top bar shows on a desktop.
	 */
	let {
		session,
		section,
		inDrawer,
		children
	}: {
		session: Session;
		section: FrameSection;
		inDrawer: boolean;
		children: Snippet;
	} = $props();
</script>

<div class="sidebar" data-testid={inDrawer ? 'sidebar-drawer' : 'sidebar'}>
	<div class="sidebar__content">{@render children()}</div>
	{#if inDrawer}
		<div class="sidebar__footer">
			<nav aria-label="Sections">
				<a
					href={resolve('/accounts')}
					aria-current={section === 'accounts' ? 'page' : undefined}
					data-testid="drawer-accounts-link">Accounts</a
				>
			</nav>
			<div class="sidebar__session">
				<Avatar {session} />
				<SignOut />
			</div>
		</div>
	{/if}
</div>

<style>
	.sidebar {
		display: flex;
		flex-direction: column;
		height: 100%;
		min-height: 0;
	}

	.sidebar__content {
		flex: 1;
		min-height: 0;
		overflow: auto;
		padding: var(--space-2) 0;
	}

	.sidebar__footer {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
		padding: var(--space-3) var(--space-4);
		border-top: var(--border-width) solid var(--color-border);
	}

	.sidebar__footer a {
		color: var(--color-text);
	}

	.sidebar__session {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-2);
		min-width: 0;
	}
</style>
