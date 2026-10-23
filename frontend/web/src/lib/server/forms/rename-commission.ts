import { Schema } from 'effect';
import { isSegment } from '$lib/types/segment-format';
import { commissionId, type CommissionId } from '$lib/types/brand';
import { createCommissionField } from './create-commission';

/**
 * The "Rename" form: the commission's id from a hidden field, and the title
 * as typed. It states no rule of its own; the backend's field errors carry
 * every refusal of the title.
 */
export const renameCommissionForm = Schema.Struct({ id: Schema.String, title: Schema.String });

/** The form field a backend pointer names: `#/title` is the title, as on create. */
export const renameCommissionField = createCommissionField;

/**
 * The commission the hidden field names: minted through {@link commissionId},
 * and only when it is a piece the path builder takes, so an id the rename
 * call would refuse is refused before anything is called.
 */
export function renameTarget(raw: string): CommissionId | undefined {
	const id = commissionId(raw);
	return id !== undefined && isSegment(id) ? id : undefined;
}
