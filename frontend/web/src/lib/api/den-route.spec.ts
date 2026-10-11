import { describe, expect, it } from 'vitest';
import { DEN_ROOT, denRootPath } from './den-route';

describe('den-route', () => {
	it('relies on the app having no base path: setting one makes resolve() differ, and fails here first', () => {
		expect(denRootPath()).toBe(DEN_ROOT);
		expect(DEN_ROOT).toBe('/den');
	});
});
