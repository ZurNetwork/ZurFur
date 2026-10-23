import { Schema } from 'effect';
import { fieldPointer, type FieldPointer } from '$lib/types/brand';

/**
 * The "New commission" form: the title as typed. It states no rule of its
 * own; the backend's field errors carry every refusal.
 */
export const createCommissionForm = Schema.Struct({ title: Schema.String });

/** The form field a backend pointer names: `#/title` is the title. */
export function createCommissionField(pointer: FieldPointer): 'title' | undefined {
	return pointer === fieldPointer('#/title') ? 'title' : undefined;
}
