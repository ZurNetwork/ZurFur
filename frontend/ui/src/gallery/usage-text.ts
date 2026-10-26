/** One run of a usage line: plain text, or the text between a pair of backticks. */
export interface UsageSegment {
	readonly kind: 'text' | 'code';
	readonly text: string;
}

const CODE_SPAN = /`([^`]+)`/g;

function textSegment(text: string): readonly UsageSegment[] {
	return text === '' ? [] : [{ kind: 'text', text }];
}

/**
 * Splits a usage line on backticks into text and code segments, in order and
 * without the backticks that paired up. An unmatched backtick, and an empty
 * pair, stay in the text around them. The segments are data: the caller
 * renders each one as text.
 */
export function splitUsage(line: string): readonly UsageSegment[] {
	const segments: UsageSegment[] = [];
	let cursor = 0;
	for (const match of line.matchAll(CODE_SPAN)) {
		const code = match[1];
		if (code === undefined) continue;
		segments.push(...textSegment(line.slice(cursor, match.index)));
		segments.push({ kind: 'code', text: code });
		cursor = match.index + match[0].length;
	}
	segments.push(...textSegment(line.slice(cursor)));
	return segments;
}
