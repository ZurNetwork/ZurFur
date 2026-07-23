import { page } from 'vitest/browser';
import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import Landing from './+page.svelte';

describe('/ landing', () => {
	it('shows the sign-in CTA when signed out', async () => {
		render(Landing, { data: { session: null } });

		await expect.element(page.getByTestId('signin-cta')).toHaveAttribute('href', '/login');
	});

	it('shows who is signed in (ruling 9b: / is the provisioning proof)', async () => {
		const alice = {
			did: 'did:plc:alice',
			handle: 'alice.zurfur.app',
			display_name: 'Alice',
			avatar_url: null
		};
		render(Landing, { data: { session: alice } });

		await expect
			.element(page.getByTestId('signed-in-as'))
			.toHaveTextContent('Signed in as alice.zurfur.app.');
	});
});
