import type { DenRemoved } from '$lib/api/den';

/** The word a soft-deleted item shows: the backend's own, or a neutral one for a word this build doesn't know. */
export function removedWord(removed: DenRemoved): string {
	switch (removed) {
		case 'archived':
			return 'Archived';
		case 'deactivated':
			return 'Deactivated';
		case 'unknown':
			return 'Set aside';
	}
}
