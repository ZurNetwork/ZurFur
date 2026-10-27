import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import type { ComponentProps } from 'svelte';
import RecordingEnvironment from './RecordingEnvironment.svelte';
import EnvironmentProbe from './EnvironmentProbe.svelte';
import Toggle from '../../lib/controls/Toggle.svelte';
import type { Environment } from '../../lib/environment/context.ts';

type SeamCall = Parameters<ComponentProps<typeof RecordingEnvironment>['onRecord']>[0];

/** The recording environment, as a component below it finds it, and the calls it records. */
function recording(): { readonly environment: Environment; readonly calls: SeamCall[] } {
	const found: Environment[] = [];
	const calls: SeamCall[] = [];
	render(
		EnvironmentProbe,
		{ depth: 0, onEnvironment: (environment: Environment) => found.push(environment) },
		{ wrapper: RecordingEnvironment, wrapperProps: { onRecord: (call) => calls.push(call) } }
	);
	const [environment] = found;
	if (environment === undefined) throw new Error('the probe found no environment');
	return { environment, calls };
}

describe('RecordingEnvironment', () => {
	it('records each attachment once, however often the control below re-renders', async () => {
		const calls: SeamCall[] = [];
		const screen = render(
			Toggle,
			{ label: 'Show my queue', checked: false, onCheckedChange: () => undefined },
			{ wrapper: RecordingEnvironment, wrapperProps: { onRecord: (call) => calls.push(call) } }
		);
		await screen.rerender({ checked: true });
		await screen.rerender({ checked: false });
		const squishes = calls.filter((call) => call.kind === 'move' && call.move === 'squish');
		expect(squishes).toHaveLength(1);
	});

	it('records every move with its arguments and the element it landed on', () => {
		const { environment, calls } = recording();
		const { moves } = environment;
		const element = document.createElement('div');
		const onEnd = vi.fn();
		const attachments = [
			moves.dim(),
			moves.loop('spin'),
			moves.loop('breathe'),
			moves.burst(),
			moves.life(onEnd)
		];
		for (const attachment of attachments) attachment(element);
		expect(calls).toEqual([
			{ kind: 'move', move: 'dim', args: [], element },
			{ kind: 'move', move: 'loop', args: ['spin'], element },
			{ kind: 'move', move: 'loop', args: ['breathe'], element },
			{ kind: 'move', move: 'burst', args: [], element },
			{ kind: 'move', move: 'life', args: [onEnd], element }
		]);
		expect(onEnd).not.toHaveBeenCalled();
	});

	it("hands a spec life's onEnd, so the spec can end the life", () => {
		const { environment, calls } = recording();
		const onEnd = vi.fn();
		environment.moves.life(onEnd)(document.createElement('div'));
		const [call] = calls;
		expect(call).toMatchObject({ kind: 'move', move: 'life' });
		const args: unknown = Reflect.get(Object(call), 'args');
		const recorded: unknown = Array.isArray(args) ? args[0] : undefined;
		if (typeof recorded !== 'function') throw new Error('no onEnd was recorded');
		Reflect.apply(recorded, undefined, []);
		expect(onEnd).toHaveBeenCalledOnce();
	});

	it('records the celebrate cue', () => {
		const { environment, calls } = recording();
		environment.sounds.play('celebrate');
		expect(calls).toEqual([{ kind: 'sound', cue: 'celebrate' }]);
	});
});
