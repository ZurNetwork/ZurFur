import { describe, expect, it } from 'vitest';
import { fieldPointer } from '$lib/types/brand';
import { invalidFieldsProblem, isRenderableErrorStatus, type FieldError } from './problem';

/**
 * Pins the one statement of the renderable error range at its edges — the
 * runtime truth behind `problem-message.ts`'s `isErrorStatus` type predicate
 * (the rulebook's sanctioned `as`-escape), so the claim the checker now
 * accepts silently is at least checked here.
 */
describe('isRenderableErrorStatus', () => {
	it('rejects just below the range', () => {
		expect(isRenderableErrorStatus(399)).toBe(false);
	});

	it('accepts both edges', () => {
		expect(isRenderableErrorStatus(400)).toBe(true);
		expect(isRenderableErrorStatus(599)).toBe(true);
	});

	it('rejects just above the range', () => {
		expect(isRenderableErrorStatus(600)).toBe(false);
	});

	it('rejects the proto3 absent-status decode (0)', () => {
		expect(isRenderableErrorStatus(0)).toBe(false);
	});
});

describe('invalidFieldsProblem', () => {
	it("answers the backend's shape for refused fields", () => {
		const titleError: FieldError = {
			pointer: fieldPointer('#/title'),
			code: 'name_empty',
			detail: "This can't be blank."
		};

		const problem = invalidFieldsProblem([titleError]);

		const expected = {
			type: 'urn:zurfur:error:invalid-request',
			code: 'invalid_request',
			title: 'Invalid request',
			detail: 'Some fields need changing; each one says why.',
			status: 422,
			errors: [titleError]
		};
		expect(problem).toEqual(expected);
	});
});
