<script lang="ts">
	import { resolve } from '$app/paths';
	import { denRootPath } from '$lib/api/den-route';
	import type { Session } from '$lib/api/session';

	/**
	 * The public pages' signed-in/out corner: the My Den link (only where this
	 * run serves a Den) and Accounts, handle + avatar + sign-out
	 * when a session exists (falling back to the DID when the profile could
	 * not be resolved — the `/me` contract's absent-handle case), a sign-in
	 * link otherwise.
	 */
	let { session, denServed }: { session: Session | undefined; denServed: boolean } = $props();
</script>

<header>
	{#if session !== undefined}
		<nav>
			{#if denServed}
				<a href={denRootPath()} data-testid="den-link">My Den</a>
			{/if}
			<a href={resolve('/accounts')} data-testid="accounts-link">Accounts</a>
		</nav>
		{#if session.avatarUrl !== undefined}
			<img data-testid="session-avatar" src={session.avatarUrl} alt="" width="32" height="32" />
		{/if}
		<span data-testid="session-handle">{session.handle ?? session.did}</span>
		<form method="post" action={resolve('/logout')}>
			<button>Sign out</button>
		</form>
	{:else}
		<a href={resolve('/login')} data-testid="signin-link">Sign in</a>
	{/if}
</header>
