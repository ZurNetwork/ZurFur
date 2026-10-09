<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { Session } from '$lib/api/session';
	import type { Trail } from '$lib/api/trail';
	import Dialog from '$lib/components/ui/Dialog.svelte';
	import type { FrameSection } from './frame-section';
	import Sidebar from './Sidebar.svelte';
	import TopBar from './TopBar.svelte';

	/**
	 * The signed-in frame: a "Skip to content" link, the top bar, the sidebar
	 * beside the main pane, and on a phone the sidebar as a drawer.
	 * `navigation` renders the sidebar's content, once inline and once in the
	 * drawer (only while the drawer is open).
	 */
	let {
		session,
		trail,
		section,
		drawerOpen = $bindable(false),
		navigation,
		children
	}: {
		session: Session;
		trail: Trail;
		section: FrameSection;
		drawerOpen?: boolean;
		navigation: Snippet<[{ inDrawer: boolean }]>;
		children: Snippet;
	} = $props();

	const drawerId = 'explorer-drawer';
</script>

<a class="skip-link" href="#main" data-testid="skip-link">Skip to content</a>
<div class="app-shell">
	<TopBar
		{session}
		{trail}
		{section}
		{drawerOpen}
		{drawerId}
		onOpenDrawer={() => (drawerOpen = true)}
	/>
	<div class="app-shell__body">
		<div class="app-shell__sidebar desktop-only">
			<Sidebar {session} {section} inDrawer={false}>
				{@render navigation({ inDrawer: false })}
			</Sidebar>
		</div>
		<main id="main" class="app-shell__pane" tabindex="-1">
			{@render children()}
		</main>
	</div>
</div>
<Dialog bind:open={drawerOpen} label="My Den" variant="drawer" id={drawerId}>
	<Sidebar {session} {section} inDrawer={true}>
		{@render navigation({ inDrawer: true })}
	</Sidebar>
</Dialog>

<style>
	.skip-link {
		position: absolute;
		top: var(--space-2);
		left: var(--space-2);
		z-index: 10;
		padding: var(--space-2) var(--space-3);
		border-radius: var(--radius-md);
		background: var(--color-accent);
		color: var(--color-on-accent);
		transform: translateY(-200%);
	}

	.skip-link:focus {
		transform: none;
	}

	.app-shell {
		display: flex;
		flex-direction: column;
		min-height: 100vh;
	}

	.app-shell__body {
		flex: 1;
		display: grid;
		grid-template-columns: var(--shell-columns);
		min-height: 0;
	}

	.app-shell__sidebar {
		position: sticky;
		top: 0;
		align-self: start;
		height: calc(100vh - var(--topbar-height));
		border-right: var(--border-width) solid var(--color-border);
		background: var(--color-surface);
	}

	.app-shell__pane {
		min-width: 0;
		padding: var(--space-6) clamp(var(--space-4), 3vw, var(--space-6)) var(--space-12);
	}

	.app-shell__pane:focus {
		outline: none;
	}

	@media (forced-colors: active) {
		.app-shell__sidebar {
			border-right: 1px solid CanvasText;
		}
	}
</style>
