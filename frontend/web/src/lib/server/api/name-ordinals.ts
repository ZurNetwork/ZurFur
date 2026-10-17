/**
 * The mock's copy of the rule that tells equal names in one Den folder apart:
 * the first of each group of equal names shows plain, each later one gets a
 * number. The backend keeps the rule; both copies answer to the shared case
 * file `contract/golden/name-ordinals.json`.
 */

import { nameOrdinal, type NameOrdinal } from '$lib/types/brand';

/** The first number a later entry of the same name can get. */
const FIRST = 2;

/**
 * The key under which two names count as the same: for now the exact name,
 * so case and every other difference keep names apart. Comparing NFC forms
 * instead changes only this function.
 */
export function comparisonKey(name: string): string {
	return name;
}

/** One entry with its place in the listing. */
interface Placed<T> {
	readonly entry: T;
	readonly place: number;
}

/** The smallest number from `start` up whose "name (n)" key is not `taken`, with that key. */
function freeNumber(
	name: string,
	start: number,
	taken: ReadonlySet<string>
): { readonly number: number; readonly key: string } {
	let number = start;
	let key = comparisonKey(`${name} (${String(number)})`);
	while (taken.has(key)) {
		number += 1;
		key = comparisonKey(`${name} (${String(number)})`);
	}
	return { number, key };
}

/**
 * The number each of `entries` (the viewer's whole listing, in listing order,
 * hidden entries already left out) gets. Entries group by their names' key,
 * groups in the order they first appear; within a group, `groupOrder` puts
 * them in order. The first of a group gets none; each later one gets the
 * smallest number, rising through the group, whose "name (n)" key is neither
 * another entry's key nor a key already given.
 */
export function nameOrdinals<T>(
	entries: readonly T[],
	nameOf: (entry: T) => string,
	groupOrder: (left: T, right: T) => number
): readonly (NameOrdinal | undefined)[] {
	const taken = new Set(entries.map((entry) => comparisonKey(nameOf(entry))));
	const groups = new Map<string, Placed<T>[]>();
	entries.forEach((entry, place) => {
		const key = comparisonKey(nameOf(entry));
		const group = groups.get(key) ?? [];
		group.push({ entry, place });
		groups.set(key, group);
	});
	const numbered: (NameOrdinal | undefined)[] = entries.map(() => undefined);
	for (const group of groups.values()) {
		const ordered = group.toSorted((left, right) => groupOrder(left.entry, right.entry));
		let next = FIRST;
		for (const { entry, place } of ordered.slice(1)) {
			const { number, key } = freeNumber(nameOf(entry), next, taken);
			taken.add(key);
			numbered[place] = nameOrdinal(number);
			next = number + 1;
		}
	}
	return numbered;
}
