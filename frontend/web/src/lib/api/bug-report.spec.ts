import { describe, expect, it } from 'vitest';
import { BUG_REPORT_FIELDS, isBugReportActionData } from './bug-report';

const ECHO = { description: 'Broken.', page: '/den' };

describe('isBugReportActionData', () => {
	it.each([
		{ kind: 'sent', reference: '3f9a1c2e' },
		{ kind: 'descriptionEmpty', ...ECHO },
		{ kind: 'descriptionTooLong', ...ECHO },
		{ kind: 'hiddenCharacter', item: 'description', ...ECHO },
		{ kind: 'hiddenCharacter', item: 'page', ...ECHO },
		{ kind: 'hiddenCharacter', item: 'browser', ...ECHO },
		{ kind: 'pageAddress', ...ECHO },
		{ kind: 'itemRefused', item: 'window', ...ECHO },
		{ kind: 'limit', ...ECHO },
		{ kind: 'couldNotSend', ...ECHO }
	])('accepts %j', (value) => {
		expect(isBugReportActionData(value)).toBe(true);
	});

	it.each([
		undefined,
		null,
		'sent',
		42,
		{},
		{ kind: 'sent' },
		{ kind: 'sent', reference: '' },
		{ kind: 'sent', reference: 'has space' },
		{ kind: 'sent', reference: 7 },
		{ kind: 'limit' },
		{ kind: 'limit', description: 'Broken.' },
		{ kind: 'couldNotSend', description: 1, page: '' },
		{ kind: 'hiddenCharacter', ...ECHO },
		{ kind: 'hiddenCharacter', item: 'window', ...ECHO },
		{ kind: 'itemRefused', item: 'reporter', ...ECHO },
		{ kind: 'unknownKind', ...ECHO },
		{ kind: 'toString', ...ECHO }
	])('refuses %j', (value) => {
		expect(isBugReportActionData(value)).toBe(false);
	});
});

describe('BUG_REPORT_FIELDS', () => {
	it('names every field once', () => {
		const names = Object.values(BUG_REPORT_FIELDS);
		expect(new Set(names).size).toBe(names.length);
	});

	it('offers no field the server would take the version from', () => {
		expect(Object.values(BUG_REPORT_FIELDS)).not.toContain('webVersion');
	});
});
