<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { Attachment } from 'svelte/attachments';
	import { provideEnvironment, type Environment } from '../../lib/environment/context.ts';
	import type { MoveName, Moves, NoteOrigin } from '../../lib/environment/moves.ts';
	import type { SoundCue, Sounds } from '../../lib/environment/sounds.ts';

	/** One call into the seam: a move attached to an element, or a cue played. */
	type SeamCall =
		| {
				readonly kind: 'move';
				readonly move: MoveName;
				readonly args: readonly NoteOrigin[];
				readonly element: HTMLElement;
		  }
		| { readonly kind: 'sound'; readonly cue: SoundCue };

	interface Props {
		/** Called once for every move attached and every cue played below. */
		readonly onRecord: (call: SeamCall) => void;
		readonly children: Snippet;
	}

	let { onRecord, children }: Props = $props();

	/** An attachment that records `move`, with its arguments, against the element it lands on. */
	function recorded(move: MoveName, args: readonly NoteOrigin[]): Attachment<HTMLElement> {
		return (element) => {
			onRecord({ kind: 'move', move, args, element });
		};
	}

	const moves: Moves = Object.freeze({
		squish: () => recorded('squish', []),
		note: (origin: NoteOrigin) => recorded('note', [origin])
	});

	const sounds: Sounds = Object.freeze({
		play: (cue: SoundCue) => {
			onRecord({ kind: 'sound', cue });
		}
	});

	const environment: Environment = Object.freeze({ moves, sounds });

	provideEnvironment(environment);
</script>

{@render children()}
