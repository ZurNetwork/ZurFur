import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import type { ComponentProps } from 'svelte';
import EnvironmentProbe from '../../checks/testing/EnvironmentProbe.svelte';
import RecordingEnvironment from '../../checks/testing/RecordingEnvironment.svelte';
import { STILL_ENVIRONMENT, type Environment } from './context.ts';

type SeamCall = Parameters<ComponentProps<typeof RecordingEnvironment>['onRecord']>[0];

describe('useEnvironment', () => {
	it('returns the still environment when nobody provided one', () => {
		const found: Environment[] = [];
		render(EnvironmentProbe, {
			depth: 0,
			onEnvironment: (environment: Environment) => found.push(environment)
		});
		expect(found).toEqual([STILL_ENVIRONMENT]);
		expect(found[0]).toBe(STILL_ENVIRONMENT);
	});

	it('returns the still environment to a grandchild when nobody provided one', () => {
		const found: Environment[] = [];
		render(EnvironmentProbe, {
			depth: 2,
			onEnvironment: (environment: Environment) => found.push(environment)
		});
		expect(found).toHaveLength(1);
		expect(found[0]).toBe(STILL_ENVIRONMENT);
	});

	it('hands a provided environment down to a grandchild', () => {
		const found: Environment[] = [];
		const calls: SeamCall[] = [];
		render(
			EnvironmentProbe,
			{ depth: 2, onEnvironment: (environment: Environment) => found.push(environment) },
			{ wrapper: RecordingEnvironment, wrapperProps: { onRecord: (call) => calls.push(call) } }
		);
		const [environment] = found;
		if (environment === undefined) throw new Error('the grandchild found no environment');
		expect(environment).not.toBe(STILL_ENVIRONMENT);
		expect(Object.isFrozen(environment)).toBe(true);
		environment.sounds.play('toggle');
		expect(calls).toEqual([{ kind: 'sound', cue: 'toggle' }]);
	});
});
