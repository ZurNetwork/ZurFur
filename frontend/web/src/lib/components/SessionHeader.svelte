<script lang="ts">
	import { resolve } from '$app/paths';
	import type { Session } from '$lib/api/session';

	/**
	 * The signed-in/out corner every page carries: handle + avatar + sign-out
	 * when a session exists (falling back to the DID when the profile could
	 * not be resolved — the `/me` contract's null case), a sign-in link
	 * otherwise.
	 */
	let { session }: { session: Session | null } = $props();
</script>

<header>
	{#if session !== null}
		<nav>
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
