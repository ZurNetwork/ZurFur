import { describe, expect, it } from 'vitest';
import { Schema } from 'effect';
import { superValidate } from 'sveltekit-superforms';
import { effect } from 'sveltekit-superforms/adapters';
import {
	invalidFieldsProblem,
	NODE_NOT_FOUND_PROBLEM,
	type FieldError,
	type Problem
} from '$lib/api/problem';
import { fieldPointer, type FieldPointer } from '$lib/types/brand';
import { problemOnFields } from './field-errors';
import { problemMessage } from './problem-message';

/** A throwaway form with two fields, standing in for a commission form. */
const commissionSchema = Schema.Struct({
	title: Schema.String,
	summary: Schema.String
});

/** A fresh, valid form with a title and a summary filled in. */
async function commissionForm() {
	const submitted = new FormData();
	submitted.set('title', ' ');
	submitted.set('summary', 'A fox in the snow.');
	return superValidate(submitted, effect(commissionSchema));
}

/** This form's fields by pointer: only `#/title` and `#/summary` have one. */
function fieldOf(pointer: FieldPointer) {
	switch (pointer) {
		case fieldPointer('#/title'):
			return 'title' as const;
		case fieldPointer('#/summary'):
			return 'summary' as const;
		default:
			return undefined;
	}
}

const titleError: FieldError = {
	pointer: fieldPointer('#/title'),
	code: 'name_empty',
	detail: "This can't be blank."
};

const strangerError: FieldError = {
	pointer: fieldPointer('#/linkedChannel'),
	code: 'channel_unknown',
	detail: "That channel isn't one we know."
};

describe('problemOnFields', () => {
	it('a title error lands under the title field', async () => {
		const form = await commissionForm();
		const problem = invalidFieldsProblem([titleError]);

		const failure = problemOnFields(form, problem, fieldOf);

		expect(form.errors.title).toEqual(["This can't be blank."]);
		expect(form.errors.summary).toBeUndefined();
		expect(form.message).toBeUndefined();
		expect(form.valid).toBe(false);
		expect(failure).toMatchObject({ status: 422 });
	});

	it('an error no field takes becomes the form message, and the mapped ones still land', async () => {
		const form = await commissionForm();
		const problem = invalidFieldsProblem([titleError, strangerError]);

		const failure = problemOnFields(form, problem, fieldOf);

		expect(form.errors.title).toEqual(["This can't be blank."]);
		expect(form.message).toEqual(problem);
		expect(failure).toMatchObject({ status: 422 });
	});

	it("an error whose pointer didn't parse becomes the form message", async () => {
		const form = await commissionForm();
		const unparsed: FieldError = { pointer: undefined, code: 'name_empty', detail: 'Fix this.' };
		const problem = invalidFieldsProblem([unparsed]);

		problemOnFields(form, problem, fieldOf);

		expect(form.message).toEqual(problem);
	});

	it('with no errors it is problemMessage', async () => {
		const problem: Problem = NODE_NOT_FOUND_PROBLEM;
		const bridged = await commissionForm();
		const plain = await commissionForm();

		const failure = problemOnFields(bridged, problem, fieldOf);
		const expected = problemMessage(plain, problem);

		expect(failure).toEqual(expected);
		expect(bridged.message).toEqual(plain.message);
		expect(bridged.errors).toEqual(plain.errors);
	});

	it('a field refusal keeps the problem status (422, never 400)', async () => {
		const form = await commissionForm();
		const problem = invalidFieldsProblem([titleError]);

		const failure = problemOnFields(form, problem, fieldOf);

		expect(failure).toMatchObject({ status: 422 });
		expect(failure).not.toMatchObject({ status: 400 });
	});

	it('a field refusal under another status keeps that status', async () => {
		const form = await commissionForm();
		const problem: Problem = { ...invalidFieldsProblem([titleError]), status: 409 };

		const failure = problemOnFields(form, problem, fieldOf);

		expect(failure).toMatchObject({ status: 409 });
	});
});
