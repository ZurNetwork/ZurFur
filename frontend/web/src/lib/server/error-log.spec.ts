import type { RequestEvent } from '@sveltejs/kit';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { logUnexpectedError, UNEXPECTED_ERROR_MESSAGE } from './error-log';

type ErrorInput = Parameters<typeof logUnexpectedError>[0];

/** A Den address holding another person's DID, a commission id and a page token. */
const SECRET_URL =
	'http://127.0.0.1:5174/den/accounts/did:plc:secretsecretsecret/commissions/01a0ef9c-secret?pageToken=v1.secret-token';

/** An error whose own text carries the real path, as a contract error might. */
const LEAKY_ERROR = new Error(
	'API contract violation: /den/accounts/did:plc:secretsecretsecret responded 200 — no node'
);

/** Run the hook for an error at the Den route, capturing everything it logs. */
function runHook(routeId: string | null): { result: unknown; logged: string } {
	const errorLog = vi.spyOn(console, 'error').mockImplementation(() => undefined);
	const event = { route: { id: routeId }, url: new URL(SECRET_URL) } as unknown as RequestEvent;
	const input: ErrorInput = { error: LEAKY_ERROR, event, status: 500, message: 'Internal Error' };
	const result = logUnexpectedError(input);
	const logged = JSON.stringify(errorLog.mock.calls);
	return { result, logged };
}

afterEach(() => {
	vi.restoreAllMocks();
});

describe('logUnexpectedError', () => {
	it('logs the route template and the status', () => {
		const { logged } = runHook('/(session)/den/[...path]');

		expect(logged).toContain('/(session)/den/[...path]');
		expect(logged).toContain('500');
	});

	it('never logs the real path, a segment, a page token or the error’s own text', () => {
		const { logged } = runHook('/(session)/den/[...path]');

		expect(logged).not.toContain('secret');
		expect(logged).not.toContain('did:plc');
		expect(logged).not.toContain('pageToken');
		expect(logged).not.toContain('contract violation');
	});

	it('hands the page a fixed message, never the error’s own', () => {
		const { result } = runHook('/(session)/den/[...path]');

		expect(result).toEqual({ message: UNEXPECTED_ERROR_MESSAGE });
	});

	it('still logs when no route matched', () => {
		const { logged } = runHook(null);

		expect(logged).toContain('(no route)');
		expect(logged).not.toContain('secret');
	});
});
