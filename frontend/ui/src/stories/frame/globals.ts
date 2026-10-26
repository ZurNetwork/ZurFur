/**
 * The toolbar's two globals, read from the text Storybook hands over (it
 * comes from the page's address, so anything can arrive): unknown text means
 * Follow the OS and Lively.
 */
import { parseSkin, type MotionMode, type Skin } from '@zurfur/ui';

/** The Skin menu's choice: follow the OS, or one skin. */
export type SkinChoice = 'os' | Skin;

/** Reads the Skin global: exactly `dark` or `light`, anything else follows the OS. */
export function parseSkinChoice(raw: unknown): SkinChoice {
	const skin = typeof raw === 'string' ? parseSkin(raw) : undefined;
	return skin ?? 'os';
}

/** Reads the Motion global: exactly `still`, anything else is lively. */
export function parseMotion(raw: unknown): MotionMode {
	return raw === 'still' ? 'still' : 'lively';
}
