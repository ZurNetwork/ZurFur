import { describe, expect, it } from 'vitest';
import { sharedType, typeOf } from './pill-type.ts';
import type { PillOption } from './types.ts';

type Id = 'a' | 'b' | 'c';

describe('sharedType', () => {
	it('is the type when every option has the same one', () => {
		const options: readonly PillOption<Id>[] = [
			{ id: 'a', label: 'a', prefix: 'batch:' },
			{ id: 'b', label: 'b', prefix: 'batch:' }
		];
		expect(sharedType(options)).toBe('batch:');
	});

	it('is the type when plain options sit beside the same one', () => {
		const options: readonly PillOption<Id>[] = [
			{ id: 'a', label: 'All' },
			{ id: 'b', label: 'b', prefix: 'batch:' },
			{ id: 'c', label: 'c', prefix: 'batch:' }
		];
		expect(sharedType(options)).toBe('batch:');
	});

	it('is none when two types are mixed', () => {
		const options: readonly PillOption<Id>[] = [
			{ id: 'a', label: 'a', prefix: 'batch:' },
			{ id: 'b', label: 'b', prefix: 'phase:' }
		];
		expect(sharedType(options)).toBeUndefined();
	});

	it('is none when two types are mixed beside a plain option', () => {
		const options: readonly PillOption<Id>[] = [
			{ id: 'a', label: 'All' },
			{ id: 'b', label: 'b', prefix: 'batch:' },
			{ id: 'c', label: 'c', prefix: 'phase:' }
		];
		expect(sharedType(options)).toBeUndefined();
	});

	it('is none when no option is typed, or there are no options', () => {
		const options: readonly PillOption<Id>[] = [
			{ id: 'a', label: 'a' },
			{ id: 'b', label: 'b' }
		];
		expect(sharedType(options)).toBeUndefined();
		expect(sharedType<Id>([])).toBeUndefined();
	});

	it('counts the empty string as plain', () => {
		const onlyEmpty: readonly PillOption<Id>[] = [
			{ id: 'a', label: 'a', prefix: '' },
			{ id: 'b', label: 'b', prefix: '' }
		];
		expect(sharedType(onlyEmpty)).toBeUndefined();
		const beside: readonly PillOption<Id>[] = [
			{ id: 'a', label: 'a', prefix: '' },
			{ id: 'b', label: 'b', prefix: 'batch:' }
		];
		expect(sharedType(beside)).toBe('batch:');
	});
});

describe('typeOf', () => {
	it('is the prefix, and none for a missing or empty one', () => {
		expect(typeOf({ id: 'a', label: 'a', prefix: 'batch:' })).toBe('batch:');
		expect(typeOf({ id: 'a', label: 'a', prefix: '' })).toBeUndefined();
		expect(typeOf({ id: 'a', label: 'a' })).toBeUndefined();
	});
});
