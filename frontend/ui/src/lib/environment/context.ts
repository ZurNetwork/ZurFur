import { getContext, hasContext, setContext } from 'svelte';
import { NO_MOVES, type Moves } from './moves';
import { NO_SOUNDS, type Sounds } from './sounds';

export interface Environment {
	readonly moves: Moves;
	readonly sounds: Sounds;
}
export const STILL_ENVIRONMENT: Environment = Object.freeze({ moves: NO_MOVES, sounds: NO_SOUNDS });
const KEY = Symbol('zurfur.environment');

/** Hands the environment to every component below. The host calls it once, at init. */
export function provideEnvironment(environment: Environment): void {
	setContext(KEY, environment);
}
/** The provided environment, or the still one; components call it once, at init. */
export function useEnvironment(): Environment {
	return hasContext(KEY) ? getContext<Environment>(KEY) : STILL_ENVIRONMENT;
}
