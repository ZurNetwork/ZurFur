import { page } from 'vitest/browser';
import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import SessionHeader from './SessionHeader.svelte';
import { did, handleFromTrusted } from '$lib/types/brand';

describe('SessionHeader', () => {
	it('shows handle, avatar and sign-out for a session', async () => {
		const alice = {
			did: did('did:plc:alice'),
			handle: handleFromTrusted('alice.zurfur.app'),
			displayName: 'Alice',
			avatarUrl: 'https://cdn.example/alice.jpg'
		};
		render(SessionHeader, { session: alice });

		await expect.element(page.getByTestId('session-handle')).toHaveTextContent('alice.zurfur.app');
		await expect
			.element(page.getByTestId('session-avatar'))
			.toHaveAttribute('src', 'https://cdn.example/alice.jpg');
		await expect.element(page.getByRole('button', { name: 'Sign out' })).toBeInTheDocument();
	});

	it('shows the accounts nav link for a session', async () => {
		const alice = {
			did: did('did:plc:alice'),
			handle: handleFromTrusted('alice.zurfur.app'),
			displayName: 'Alice',
			avatarUrl: undefined
		};
		render(SessionHeader, { session: alice });

		await expect.element(page.getByTestId('accounts-link')).toHaveAttribute('href', '/accounts');
	});

	it('falls back to the DID when the profile did not resolve', async () => {
		const unresolved = {
			did: did('did:plc:alice'),
			handle: undefined,
			displayName: undefined,
			avatarUrl: undefined
		};
		render(SessionHeader, { session: unresolved });

		await expect.element(page.getByTestId('session-handle')).toHaveTextContent('did:plc:alice');
	});

	it('shows the sign-in link when signed out', async () => {
		render(SessionHeader, { session: undefined });

		await expect.element(page.getByTestId('signin-link')).toHaveAttribute('href', '/login');
	});

	it('hides the accounts nav link when signed out', async () => {
		render(SessionHeader, { session: undefined });

		await expect.element(page.getByTestId('accounts-link')).not.toBeInTheDocument();
	});
});
