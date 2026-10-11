/**
 * The rule a path segment follows, wherever the web builds a path from
 * pieces (a Den address, an API resource path): 1 to 2048 ASCII letters,
 * digits or `. _ : % ~ -`, never `.` or `..`, and at most 16 of them. Every
 * key a path piece can hold (a DID, a UUID, a fixed folder name) fits; the
 * ASCII allowlist also shuts out `/`, `\`, NUL, whitespace and every Unicode
 * look-alike of a dot or a slash. Effect-free, so both sides of the seam
 * read the same rule.
 */

/** Every character a segment may hold, 1 to 2048 of them. */
export const SEGMENT_PATTERN = /^[A-Za-z0-9._:%~-]{1,2048}$/;

/** The most segments a path may have. */
export const MAX_SEGMENTS = 16;

/** Whether `piece` is a segment: allowlisted, sized, and not a dot segment. */
export function isSegment(piece: string): boolean {
	if (piece === '.' || piece === '..') return false;
	return SEGMENT_PATTERN.test(piece);
}
