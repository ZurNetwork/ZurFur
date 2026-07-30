import { describe, expect, it } from 'vitest';
import { superValidate } from 'sveltekit-superforms';
import { effect } from 'sveltekit-superforms/adapters';
import { deleteAccountForm } from './delete-account';

const HANDLE = 'alice.zurfur.app';

describe('deleteAccountForm', () => {
	it('accepts the exact handle', async () => {
		const submitted = new FormData();
		submitted.set('confirm', HANDLE);

		const form = await superValidate(submitted, effect(deleteAccountForm(HANDLE)));

		expect(form.valid).toBe(true);
		expect(form.data.confirm).toBe(HANDLE);
	});

	it('accepts the handle with surrounding whitespace (Trim runs first)', async () => {
		const submitted = new FormData();
		submitted.set('confirm', `  ${HANDLE}  `);

		const form = await superValidate(submitted, effect(deleteAccountForm(HANDLE)));

		expect(form.valid).toBe(true);
	});

	it('rejects a wrong handle with the schema message on confirm', async () => {
		const submitted = new FormData();
		submitted.set('confirm', 'bob.zurfur.app');

		const form = await superValidate(submitted, effect(deleteAccountForm(HANDLE)));

		expect(form.valid).toBe(false);
		expect(form.errors.confirm).toEqual([`Type ${HANDLE} exactly to confirm`]);
	});

	it('rejects an empty confirm (empty never equals a handle)', async () => {
		const submitted = new FormData();
		submitted.set('confirm', '');

		const form = await superValidate(submitted, effect(deleteAccountForm(HANDLE)));

		expect(form.valid).toBe(false);
		expect(form.errors.confirm).toBeDefined();
	});

	it('fails closed for an empty-string handle — a blank confirm must NOT validate against it', async () => {
		const submitted = new FormData();
		submitted.set('confirm', '');

		const form = await superValidate(submitted, effect(deleteAccountForm('')));

		expect(form.valid).toBe(false);
		expect(form.errors.confirm).toBeDefined();
	});

	it('fails closed for an empty-string handle even for whitespace-only confirm', async () => {
		const submitted = new FormData();
		submitted.set('confirm', '   ');

		const form = await superValidate(submitted, effect(deleteAccountForm('')));

		expect(form.valid).toBe(false);
	});
});
