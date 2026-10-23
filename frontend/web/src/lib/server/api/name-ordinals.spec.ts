// The mock's numbering against the shared case file the backend's rule
// answers to as well, so the two copies can't drift apart unseen.
import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { comparisonKey, nameOrdinals } from './name-ordinals';

/** The shared case file, from the repository's `contract/golden/`. */
const SHARED_CASES = new URL(
	'../../../../../../contract/golden/name-ordinals.json',
	import.meta.url
);

/** One entry of a shared case: its name, whether the viewer sees it, and its expected number. */
interface SharedEntry {
	readonly name: string;
	readonly visible: boolean;
	readonly ordinal: number | null;
}

/** One shared case: what it shows, and its entries in listing order. */
interface SharedCase {
	readonly description: string;
	readonly entries: readonly SharedEntry[];
}

const { cases } = JSON.parse(readFileSync(SHARED_CASES, 'utf8')) as {
	readonly cases: readonly SharedCase[];
};

/** One visible entry with its place in the case's listing, which is also its order in a group. */
interface Placed {
	readonly name: string;
	readonly place: number;
}

/** The number each entry of `entries` is shown with, `null` for none: hidden entries never count. */
function numbersOf(entries: readonly SharedEntry[]): readonly (number | null)[] {
	const visible: Placed[] = entries.flatMap((entry, place) =>
		entry.visible ? [{ name: entry.name, place }] : []
	);
	const ordinals = nameOrdinals(
		visible,
		(entry) => entry.name,
		(left, right) => left.place - right.place
	);
	const byPlace = new Map(visible.map((entry, index) => [entry.place, ordinals[index]]));
	return entries.map((_, place) => byPlace.get(place) ?? null);
}

describe('the shared cases hold', () => {
	it('reads at least one case', () => {
		expect(cases.length).toBeGreaterThan(0);
	});

	it.each(cases.map((sharedCase) => [sharedCase.description, sharedCase] as const))(
		'%s',
		(_, sharedCase) => {
			const expected = sharedCase.entries.map((entry) => entry.ordinal);
			expect(numbersOf(sharedCase.entries)).toEqual(expected);
		}
	);
});

describe('nameOrdinals', () => {
	it('numbers a group in its group order, not its place in the listing', () => {
		const entries = [
			{ name: 'Twin', rank: 9 },
			{ name: 'Twin', rank: 3 },
			{ name: 'Twin', rank: 5 }
		];
		const numbered = nameOrdinals(
			entries,
			(entry) => entry.name,
			(left, right) => left.rank - right.rank
		);
		expect(numbered).toEqual([3, undefined, 2]);
	});

	it('compares names exactly', () => {
		expect(comparisonKey('Test 1')).toBe('Test 1');
		expect(comparisonKey('Test 1')).not.toBe(comparisonKey('test 1'));
		expect(comparisonKey('é')).not.toBe(comparisonKey('é'));
	});
});
