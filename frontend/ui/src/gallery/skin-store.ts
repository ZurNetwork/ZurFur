/**
 * The gallery's one store: the skin a viewer picked, kept in localStorage.
 * Storage can be missing, blocked or full, so every read falls back to
 * following the OS and every write reports whether it was saved.
 */
import { parseSkin, type Skin } from '../lib/styles/environment.ts';

/** The localStorage key the skin is kept under; the page shell's boot script reads the same key. */
export const SKIN_STORAGE_KEY = 'zurfur:skin';

/** The calls the store makes on browser storage. */
export type SkinStorage = Pick<Storage, 'getItem' | 'setItem' | 'removeItem'>;

/** Hands back the storage; reading it can throw, so it is only ever called inside a `try`. */
export type StorageSource = () => SkinStorage;

const browserStorage: StorageSource = () => window.localStorage;

/** The stored skin, or undefined (follow the OS) when none is stored, it isn't a skin, or storage fails. */
export function readSkin(getStorage: StorageSource = browserStorage): Skin | undefined {
	try {
		return parseSkin(getStorage().getItem(SKIN_STORAGE_KEY) ?? undefined);
	} catch {
		return undefined;
	}
}

/** Stores a skin, or removes it for undefined; true when saved, false when storage failed. */
export function writeSkin(
	skin: Skin | undefined,
	getStorage: StorageSource = browserStorage
): boolean {
	try {
		const storage = getStorage();
		if (skin === undefined) storage.removeItem(SKIN_STORAGE_KEY);
		else storage.setItem(SKIN_STORAGE_KEY, skin);
		return true;
	} catch {
		return false;
	}
}
