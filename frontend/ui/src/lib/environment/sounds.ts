export type SoundCue = 'open' | 'close' | 'toggle' | 'celebrate';
export interface Sounds {
	/** Plays the cue's blip, or nothing when sound may not play now. */
	readonly play: (cue: SoundCue) => void;
}
export const NO_SOUNDS: Sounds = Object.freeze({ play: () => undefined });
