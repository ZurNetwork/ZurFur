<script lang="ts">
	import { resolve } from '$app/paths';
	import type { Session } from '$lib/api/session';
	import type { Trail } from '$lib/api/trail';
	import Avatar from '$lib/components/ui/Avatar.svelte';
	import IconButton from '$lib/components/ui/IconButton.svelte';
	import PathBar from '$lib/components/ui/PathBar.svelte';
	import type { FrameSection } from './frame-section';
	import SignOut from './SignOut.svelte';

	/**
	 * The frame's top row. On a desktop: the brand, the path, the section
	 * links, who is signed in and Sign out. On a phone with a `drawer`: the
	 * drawer's ☰ and the back link to the parent, the rest moving into the
	 * drawer; without one, everything stays in the row.
	 */
	let {
		session,
		trail,
		section,
		drawer,
		drawerOpen,
		drawerId,
		onOpenDrawer
	}: {
		session: Session;
		trail: Trail;
		section: FrameSection;
		drawer: boolean;
		drawerOpen: boolean;
		drawerId: string;
		onOpenDrawer: () => void;
	} = $props();
</script>

<header class="top-bar">
	{#if drawer}
		<span class="phone-only">
			<IconButton
				icon="menu"
				label="Open navigation"
				expanded={drawerOpen}
				controls={drawerId}
				onclick={onOpenDrawer}
				testid="open-drawer"
			/>
		</span>
	{/if}
	<a class="top-bar__brand" class:desktop-only={drawer} href={resolve('/')}>zurfur</a>
	<div class="top-bar__path">
		<PathBar {trail} />
	</div>
	<nav class="top-bar__links" class:desktop-only={drawer} aria-label="Sections">
		<a
			href={resolve('/accounts')}
			aria-current={section === 'accounts' ? 'page' : undefined}
			data-testid="accounts-link">Accounts</a
		>
	</nav>
	<div class="top-bar__session" class:desktop-only={drawer}>
		<Avatar {session} />
		<SignOut />
	</div>
</header>

<style>
	.top-bar {
		display: flex;
		align-items: center;
		gap: var(--space-4);
		height: var(--topbar-height);
		padding: 0 var(--space-4);
		border-bottom: var(--border-width) solid var(--color-border);
		background: var(--color-surface);
	}

	.top-bar__brand {
		color: var(--color-text);
		font-weight: var(--text-weight-bold);
		text-decoration: none;
	}

	.top-bar__path {
		flex: 1;
		min-width: 0;
	}

	.top-bar__links {
		display: flex;
		gap: var(--space-4);
	}

	.top-bar__links a {
		color: var(--color-text-muted);
		font-size: var(--text-sm);
		text-decoration: none;
	}

	.top-bar__links a:hover,
	.top-bar__links a[aria-current='page'] {
		color: var(--color-text);
	}

	.top-bar__session {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		min-width: 0;
		max-width: var(--sidebar-width);
	}

	@media (forced-colors: active) {
		.top-bar {
			border-bottom: 1px solid CanvasText;
		}
	}
</style>
