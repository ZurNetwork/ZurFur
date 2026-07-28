import { describe, expect, it } from 'vitest';
import { Schema } from 'effect';
import { superValidate } from 'sveltekit-superforms';
import { effect } from 'sveltekit-superforms/adapters';

/**
 * Infrastructure pin for the Superforms + Effect Schema adapter (Engineer
 * ruling 2026-07-28): proves the adapter and our pinned `effect` agree before
 * any real form schema builds on the pair. The schema here is a throwaway —
 * real form schemas live beside this file, one module per form.
 */
const pinSchema = Schema.Struct({
	name: Schema.Trim.pipe(Schema.minLength(1))
});

describe('the superforms effect adapter', () => {
	it('accepts a valid form and hands back typed, coerced data', async () => {
		const submitted = new FormData();
		submitted.set('name', '  Alice  ');

		const form = await superValidate(submitted, effect(pinSchema));

		expect(form.valid).toBe(true);
		expect(form.data.name).toBe('Alice');
	});

	it('rejects an invalid form with a per-field error, not a throw', async () => {
		const submitted = new FormData();
		submitted.set('name', '');

		const form = await superValidate(submitted, effect(pinSchema));

		expect(form.valid).toBe(false);
		expect(form.errors.name).toBeDefined();
	});
});
