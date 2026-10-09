import { describe, expect, it } from 'vitest';
import { fetchStub, problemResponse, unreachableFetch } from '$lib/testing/http';
import { expectRedirect } from '$lib/testing/redirect';
import { actions, load } from './+page.server';
import type { Session } from '$lib/api/session';
import { did, handleFromTrusted } from '$lib/types/brand';
import type { SuperValidated } from 'sveltekit-superforms';

type LoadEvent = Parameters<typeof load>[0];
type ActionEvent = Parameters<(typeof actions)['default']>[0];

const signinDefaultAction = actions.default;

/** The action's fail payload: everything rides the superform (message = backend Problem). */
type LoginForm = SuperValidated<{ handle: string }>;

const alice: Session = {
	did: did('did:plc:alice'),
	handle: handleFromTrusted('alice.zurfur.app'),
	displayName: 'Alice',
	avatarUrl: undefined
};

function loadEvent(session: Session | undefined, search = ''): LoadEvent {
	const event = {
		parent: () => Promise.resolve({ session }),
		url: new URL(`http://localhost/login${search}`)
	};
	return event as unknown as LoadEvent;
}

/** `load` types its return as possibly-void (it may throw a redirect); pin the data shape. */
async function runLoad(
	event: LoadEvent
): Promise<{ callbackError: string | undefined; form: LoginForm }> {
	return (await load(event)) as { callbackError: string | undefined; form: LoginForm };
}

async function signinAction(fetch: typeof globalThis.fetch, handle: string | null) {
	const body = new URLSearchParams(handle === null ? {} : { handle });
	const request = new Request('http://localhost/login', { method: 'POST', body });
	return (await signinDefaultAction({ request, fetch } as unknown as ActionEvent)) as {
		status: number;
		data: { form: LoginForm };
	};
}

describe('/login load', () => {
	it('renders signed-out with no callback error and a pristine form', async () => {
		const result = await runLoad(loadEvent(undefined));
		expect(result.callbackError).toBeUndefined();
		// superforms deprecates `posted` toward v3 without a drop-in replacement
		// for "did this pristine load produce a submitted form"; revisit on the
		// v3 upgrade.
		// eslint-disable-next-line @typescript-eslint/no-deprecated
		expect(result.form.posted).toBe(false);
		expect(result.form.message).toBeUndefined();
	});

	it('maps a known ?error code to its message', async () => {
		const result = await runLoad(loadEvent(undefined, '?error=denied'));
		expect(result.callbackError).toBe('Sign-in was cancelled at your PDS.');
	});

	it('falls back on an unknown ?error code', async () => {
		const result = await runLoad(loadEvent(undefined, '?error=mystery'));
		expect(result.callbackError).toBe('Sign-in failed. Try again.');
	});

	it('bounces a signed-in visitor home', async () => {
		const redirect = await expectRedirect(() => load(loadEvent(alice)));
		expect(redirect.status).toBe(303);
		expect(redirect.location).toBe('/');
	});
});

describe('/login signin action', () => {
	it('rejects an empty handle locally with a field error, never reaching the backend', async () => {
		const failure = await signinAction(unreachableFetch('must not reach the backend'), '   ');
		expect(failure.status).toBe(422);
		expect(failure.data.form.valid).toBe(false);
		expect(failure.data.form.errors.handle).toContain('Please insert your handle');
	});

	it('rejects a shape-invalid handle locally', async () => {
		const failure = await signinAction(unreachableFetch('must not reach the backend'), 'no-dot');
		expect(failure.status).toBe(422);
		expect(failure.data.form.errors.handle).toContain('This handle is not valid');
	});

	it('accepts a punycode (xn--) handle at sign-in — auth-time, not claim-time (Engineer ruling)', async () => {
		const authorizeUrl = 'https://pds.example/oauth/authorize?request_uri=idn';
		const { fetch } = fetchStub(
			() => new Response(null, { status: 303, headers: { location: authorizeUrl } })
		);
		const redirect = await expectRedirect(() => signinAction(fetch, 'xn--sneaky.example'));
		expect(redirect.status).toBe(303);
		expect(redirect.location).toBe(authorizeUrl);
	});

	it('rejects an over-long handle locally (the 253-char atproto cap)', async () => {
		const failure = await signinAction(
			unreachableFetch('must not reach the backend'),
			`${'a'.repeat(250)}.com`
		);
		expect(failure.status).toBe(422);
		expect(failure.data.form.errors.handle).toContain('This handle is too long');
	});

	it('relays the PDS authorize URL as a 303 navigation, trimming the handle first', async () => {
		const authorizeUrl = 'https://pds.example/oauth/authorize?request_uri=abc';
		const { fetch } = fetchStub(
			() => new Response(null, { status: 303, headers: { location: authorizeUrl } })
		);
		const redirect = await expectRedirect(() => signinAction(fetch, '  alice.test  '));
		expect(redirect.status).toBe(303);
		expect(redirect.location).toBe(authorizeUrl);
	});

	it('hands a backend problem to the page as the form message', async () => {
		const { fetch } = fetchStub(() => problemResponse(422, 'invalid_request'));
		const failure = await signinAction(fetch, 'valid.example');
		expect(failure.status).toBe(422);
		expect(failure.data.form.message).toMatchObject({ code: 'invalid_request' });
		expect(failure.data.form.data.handle).toBe('valid.example');
	});
});
