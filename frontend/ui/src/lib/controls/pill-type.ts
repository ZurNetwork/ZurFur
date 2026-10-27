import type { PillOption } from './types.ts';

/** The type an option carries: its `prefix`, with the empty string counted as none. */
export function typeOf<Id extends string>(option: PillOption<Id>): string | undefined {
	return option.prefix === '' ? undefined : option.prefix;
}

/**
 * The one type every typed option shares, for the switch to show once; `undefined` when no option
 * is typed or two types differ. Untyped options, such as "All", do not stop it.
 */
export function sharedType<Id extends string>(
	options: readonly PillOption<Id>[]
): string | undefined {
	const types = new Set<string>();
	for (const option of options) {
		const type = typeOf(option);
		if (type !== undefined) types.add(type);
	}
	if (types.size !== 1) return undefined;
	const [only] = types;
	return only;
}
