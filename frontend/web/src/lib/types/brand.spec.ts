import { describe, expect, it } from 'vitest';
import { accountId, did, handle, handleFromTrusted } from './brand';
import { HANDLE_MAX_LEN } from './handle-format';

describe('handle (the validated mint)', () => {
	it('mints a well-shaped handle, trimmed', () => {
		expect(handle('  alice.zurfur.app  ')).toBe('alice.zurfur.app');
	});

	it('rejects empty and whitespace-only input', () => {
		expect(handle('')).toBeUndefined();
		expect(handle('   ')).toBeUndefined();
	});

	it('rejects input over the atproto length cap', () => {
		const overlong = `${'a'.repeat(HANDLE_MAX_LEN)}.com`;
		expect(handle(overlong)).toBeUndefined();
	});

	it('rejects a shape that is not an atproto handle (no dot)', () => {
		expect(handle('no-dot')).toBeUndefined();
	});

	it('rejects punycode (xn--) labels — claim-time rule set (DD 26050561)', () => {
		expect(handle('xn--sneaky.example')).toBeUndefined();
	});
});

describe('the trusted nominal casts', () => {
	it('pass their input through unchanged (brand only, no runtime work)', () => {
		expect(accountId('0198c5f2-aaaa-7bbb-8ccc-ddddeeee0001')).toBe(
			'0198c5f2-aaaa-7bbb-8ccc-ddddeeee0001'
		);
		expect(did('did:plc:alice')).toBe('did:plc:alice');
		expect(handleFromTrusted('alice.zurfur.app')).toBe('alice.zurfur.app');
	});
});
