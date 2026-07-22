import { page } from 'vitest/browser';
import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import SessionHeader from './SessionHeader.svelte';

describe('SessionHeader', () => {
	it('shows handle, avatar and sign-out for a session', async () => {
		const alice = {
			did: 'did:plc:alice',
			handle: 'alice.zurfur.app',
			display_name: 'Alice',
			avatar_url: 'https://cdn.example/alice.jpg'
		};
		render(SessionHeader, { session: alice });

		await expect.element(page.getByTestId('session-handle')).toHaveTextContent('alice.zurfur.app');
		await expect
			.element(page.getByTestId('session-avatar'))
			.toHaveAttribute('src', 'https://cdn.example/alice.jpg');
		await expect.element(page.getByRole('button', { name: 'Sign out' })).toBeInTheDocument();
	});

	it('falls back to the DID when the profile did not resolve', async () => {
		const unresolved = { did: 'did:plc:alice', handle: null, display_name: null, avatar_url: null };
		render(SessionHeader, { session: unresolved });

		await expect.element(page.getByTestId('session-handle')).toHaveTextContent('did:plc:alice');
	});

	it('shows the sign-in link when signed out', async () => {
		render(SessionHeader, { session: null });

		await expect.element(page.getByTestId('signin-link')).toHaveAttribute('href', '/login');
	});
});
