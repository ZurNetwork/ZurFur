<script lang="ts">
	import { resolve } from '$app/paths';
	import type { Session } from '$lib/api/session';
	import type { Trail } from '$lib/api/trail';
	import Avatar from '$lib/components/ui/Avatar.svelte';
	import IconButton from '$lib/components/ui/IconButton.svelte';
	import PathBar from '$lib/components/ui/PathBar.svelte';
	import type { FrameSection } from './frame-section';
	import ReportBugButton from './ReportBugButton.svelte';
	import SignOut from './SignOut.svelte';
	import { denRootPath } from '$lib/api/den-route';

	/**
	 * The frame's top row. On a desktop: the brand, the path, the section
	 * links, "Report a bug", who is signed in and Sign out. On a phone: the drawer's ☰ and
	 * the back link to the parent, the rest moving into the drawer.
	 * `menuButton` binds ☰, so the frame can give it focus back.
	 */
	let {
		session,
		trail,
		section,
		drawerOpen,
		drawerId,
		onOpenDrawer,
		menuButton = $bindable()
	}: {
		session: Session;
		trail: Trail;
		section: FrameSection;
		drawerOpen: boolean;
		drawerId: string;
		onOpenDrawer: () => void;
		menuButton?: HTMLButtonElement | undefined;
	} = $props();
</script>

<header class="top-bar">
	<span class="phone-only">
		<IconButton
			icon="menu"
			label="Open navigation"
			expanded={drawerOpen}
			controls={drawerId}
			onclick={onOpenDrawer}
			testid="open-drawer"
			bind:element={menuButton}
		/>
	</span>
	<a class="top-bar__brand desktop-only" href={resolve('/')}>zurfur</a>
	<div class="top-bar__path">
		<PathBar {trail} />
	</div>
	<nav class="top-bar__links desktop-only" aria-label="Sections">
		<a
			href={denRootPath()}
			aria-current={section === 'den' ? 'page' : undefined}
			data-testid="den-link">My Den</a
		>
		<a
			href={resolve('/accounts')}
			aria-current={section === 'accounts' ? 'page' : undefined}
			data-testid="accounts-link">Accounts</a
		>
	</nav>
	<span class="top-bar__report desktop-only">
		<ReportBugButton current={section === 'report'} />
	</span>
	<div class="top-bar__session desktop-only">
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

	.top-bar__report {
		display: inline-flex;
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
