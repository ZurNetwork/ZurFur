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
		render(SessionHeader, { session: alice, denServed: false });

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
		render(SessionHeader, { session: alice, denServed: false });

		await expect.element(page.getByTestId('accounts-link')).toHaveAttribute('href', '/accounts');
	});

	it('shows the My Den link for a session', async () => {
		const alice = {
			did: did('did:plc:alice'),
			handle: handleFromTrusted('alice.zurfur.app'),
			displayName: 'Alice',
			avatarUrl: undefined
		};
		render(SessionHeader, { session: alice, denServed: true });

		await expect.element(page.getByTestId('den-link')).toHaveAttribute('href', '/den');
	});

	it('hides the My Den link where this run serves no Den', async () => {
		const alice = {
			did: did('did:plc:alice'),
			handle: handleFromTrusted('alice.zurfur.app'),
			displayName: 'Alice',
			avatarUrl: undefined
		};
		render(SessionHeader, { session: alice, denServed: false });

		await expect.element(page.getByTestId('accounts-link')).toBeInTheDocument();
		await expect.element(page.getByTestId('den-link')).not.toBeInTheDocument();
	});

	it('falls back to the DID when the profile did not resolve', async () => {
		const unresolved = {
			did: did('did:plc:alice'),
			handle: undefined,
			displayName: undefined,
			avatarUrl: undefined
		};
		render(SessionHeader, { session: unresolved, denServed: false });

		await expect.element(page.getByTestId('session-handle')).toHaveTextContent('did:plc:alice');
	});

	it('shows the sign-in link when signed out', async () => {
		render(SessionHeader, { session: undefined, denServed: false });

		await expect.element(page.getByTestId('signin-link')).toHaveAttribute('href', '/login');
	});

	it('hides the accounts nav link when signed out', async () => {
		render(SessionHeader, { session: undefined, denServed: false });

		await expect.element(page.getByTestId('accounts-link')).not.toBeInTheDocument();
	});
});
