import { describe, expect, it } from 'vitest';
import {
	readSkin,
	SKIN_STORAGE_KEY,
	writeSkin,
	type SkinStorage,
	type StorageSource
} from './skin-store.ts';

/** An in-memory storage, with optional failures planted in its calls. */
class FakeStorage implements SkinStorage {
	readonly entries = new Map<string, string>();
	failSet = false;
	failRemove = false;

	getItem(key: string): string | null {
		return this.entries.get(key) ?? null;
	}

	removeItem(key: string): void {
		if (this.failRemove) throw new Error('SecurityError');
		this.entries.delete(key);
	}

	setItem(key: string, value: string): void {
		if (this.failSet) throw new Error('QuotaExceededError');
		this.entries.set(key, value);
	}
}

function holding(value: string | undefined): FakeStorage {
	const storage = new FakeStorage();
	if (value !== undefined) storage.entries.set(SKIN_STORAGE_KEY, value);
	return storage;
}

const throwingSource: StorageSource = () => {
	throw new Error('SecurityError');
};

const nullSource = (() => null) as unknown as StorageSource;

const throwingGetItem: StorageSource = () => {
	const storage = new FakeStorage();
	storage.getItem = () => {
		throw new Error('SecurityError');
	};
	return storage;
};

describe('readSkin', () => {
	it.each(['dark', 'light'] as const)('reads a stored %j', (skin) => {
		expect(readSkin(() => holding(skin))).toBe(skin);
	});

	it.each([
		['a missing value', undefined],
		['LIGHT', 'LIGHT'],
		['a trailing space', 'light '],
		['markup', 'light"><script>'],
		['constructor', 'constructor'],
		['__proto__', '__proto__'],
		['toString', 'toString'],
		['an empty string', '']
	])('follows the OS for %s', (_label, stored) => {
		expect(readSkin(() => holding(stored))).toBeUndefined();
	});

	it.each([
		['reading the store throws', throwingSource],
		['the store is null', nullSource],
		['getItem throws', throwingGetItem]
	])('follows the OS when %s', (_label, source) => {
		expect(readSkin(source)).toBeUndefined();
	});

	it('reads only its own key', () => {
		const storage = new FakeStorage();
		storage.entries.set('skin', 'light');
		expect(readSkin(() => storage)).toBeUndefined();
	});
});

describe('writeSkin', () => {
	it.each(['dark', 'light'] as const)('stores %j and says it saved', (skin) => {
		const storage = holding(undefined);
		expect(writeSkin(skin, () => storage)).toBe(true);
		expect([...storage.entries]).toEqual([[SKIN_STORAGE_KEY, skin]]);
	});

	it('removes the key for undefined and says it saved', () => {
		const storage = holding('light');
		expect(writeSkin(undefined, () => storage)).toBe(true);
		expect(storage.entries.size).toBe(0);
	});

	it('says not saved when setItem throws', () => {
		const storage = holding('dark');
		storage.failSet = true;
		expect(writeSkin('light', () => storage)).toBe(false);
		expect(storage.entries.get(SKIN_STORAGE_KEY)).toBe('dark');
	});

	it('says not saved when removeItem throws', () => {
		const storage = holding('dark');
		storage.failRemove = true;
		expect(writeSkin(undefined, () => storage)).toBe(false);
	});

	it.each([
		['reading the store throws', throwingSource],
		['the store is null', nullSource]
	])('says not saved when %s', (_label, source) => {
		expect(writeSkin('dark', source)).toBe(false);
		expect(writeSkin(undefined, source)).toBe(false);
	});
});
