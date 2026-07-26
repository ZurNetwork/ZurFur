// The tolerant-reader pin (contract VERSIONING.md §6, client side): the
// SHIPPED decode path must ignore unknown fields — protobuf-es rejects them
// by default, and the contract's additive-only evolution is void without this
// option. These tests exercise `decodeContract` itself (not a hand-rolled
// `fromJson` call), so removing `ignoreUnknownFields: true` from the one
// shipped construction site is a failing test here, never a production
// incident when the backend adds its first field.
import { describe, expect, it } from 'vitest';
import { decodeContract } from './zurfur-api';
import { GetMeResponseSchema } from './generated/zurfur/api/v1/session_pb';

describe('the contract decode path (tolerant reader, §6)', () => {
	it('ignores unknown fields — an additive server change must not break this client', () => {
		const body = {
			did: 'did:plc:tolerant',
			handle: 'tolerant.bsky.social',
			aFieldFromTheFuture: 'added in a later additive release'
		};

		const message = decodeContract(GetMeResponseSchema, body);

		expect(message.did).toBe('did:plc:tolerant');
		expect(message.handle).toBe('tolerant.bsky.social');
	});

	it('still rejects a malformed body — tolerance is for unknown keys, not garbage', () => {
		expect(() => decodeContract(GetMeResponseSchema, { did: 42 })).toThrow();
	});

	it('treats absent optionals as undefined — null is not on the wire (R4)', () => {
		const message = decodeContract(GetMeResponseSchema, { did: 'did:plc:bare' });

		expect(message.handle).toBeUndefined();
		expect(message.displayName).toBeUndefined();
	});
});
