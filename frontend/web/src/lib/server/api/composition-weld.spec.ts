// The both-tier weld for the flat composition (ZMVP-163; DD 45514754, wire
// half of DD 42762241's surviving decisions).
//
// The Rust tier's `golden_wire` test pins what `GET /api/v1/commissions/{id}`
// EMITS; this pins that the same bytes DECODE here, through the same generated
// artifact and the same shipped door (`decodeContract`). The corpus is
// authoritative over both tiers, so a field that only one of them understands
// is the failure mode the contract exists to make impossible — and only a test
// that runs the two against one payload can see it.
//
// The payload below is transcribed from the Rust suite's own assertions
// (`backend/crates/api/tests/commission_get.rs`), canonical ProtoJSON as
// pbjson emits it: lowerCamelCase keys, absent optionals with no key at all,
// implicit-presence defaults omitted.
import { describe, expect, it } from 'vitest';
import { decodeContract } from './zurfur-api';
import { GetCommissionResponseSchema } from './generated/zurfur/api/v1/commission_pb';

/** 2^53 + 1 — the integer that must survive as digits, not as a float. */
const BEYOND_DOUBLE = '9007199254740993';

/** What the Rust tier emits for a commission with one contributed element. */
const emitted = {
	id: '019fe39b-37e8-7561-862c-5e726018df84',
	title: 'A ref sheet',
	lifecycle: 'draft',
	visibility: 'private',
	createdAt: '2026-08-08T12:00:00Z',
	tabs: [{ id: '019fe39b-37e8-7561-862c-5e726018df85', tab: 'main', mode: 'total' }],
	surfaces: [{ surface: 'content', tabId: '019fe39b-37e8-7561-862c-5e726018df85', mode: 'total' }],
	elements: [
		{
			id: '019fe39b-37e8-7561-862c-5e726018df86',
			tabId: '019fe39b-37e8-7561-862c-5e726018df85',
			surface: 'content',
			kind: 'note',
			mode: 'total',
			opaqueJson: `{"big":${BEYOND_DOUBLE},"text":"三毛猫 🐾"}`
		}
	]
};

describe('the flat composition weld (both tiers, one artifact)', () => {
	it('decodes what the Rust tier emits', () => {
		const message = decodeContract(GetCommissionResponseSchema, emitted);

		expect(message.id).toBe(emitted.id);
		expect(message.title).toBe('A ref sheet');
		expect(message.visibility).toBe('private');

		// The skeleton, and the by-id addressing that replaces parent pointers.
		expect(message.tabs).toHaveLength(1);
		expect(message.tabs[0].tab).toBe('main');
		expect(message.surfaces[0].tabId).toBe(message.tabs[0].id);
		expect(message.elements[0].tabId).toBe(message.tabs[0].id);
		expect(message.elements[0].surface).toBe('content');
	});

	it('carries the payload as an opaque string, undamaged beyond 2^53', () => {
		const message = decodeContract(GetCommissionResponseSchema, emitted);
		const payload = message.elements[0].payload;

		// The `oneof` arrives as a discriminated union; the arm is the one the
		// corpus declares, and its value is TEXT — not parsed structure.
		expect(payload.case).toBe('opaqueJson');
		if (payload.case !== 'opaqueJson') throw new Error('unreachable');
		expect(typeof payload.value).toBe('string');

		// THE regression `google.protobuf.Struct` was disqualified for (DD
		// 42762241 D4): Struct floats every integer — pbjson errors above 2^53
		// while protobuf-es silently TRUNCATES, so this value would have arrived
		// as 9007199254740992 here and as an error there. As text it is exact,
		// and stays exact until the consumer decides how to read it.
		expect(payload.value).toContain(BEYOND_DOUBLE);
		expect(JSON.parse(payload.value).text).toBe('三毛猫 🐾');
	});

	it('reads an omitted empty composition as empty, and withholding as stated', () => {
		// Canonical ProtoJSON omits empty repeated fields and false booleans
		// entirely, so a commission with nothing in it arrives with no
		// `elements` key. The decoded message must still present an array —
		// otherwise "absent" and "empty" would differ across the tiers.
		const bare = {
			id: emitted.id,
			title: 'A ref sheet',
			lifecycle: 'draft',
			visibility: 'private',
			createdAt: emitted.createdAt
		};

		const message = decodeContract(GetCommissionResponseSchema, bare);
		expect(message.elements).toEqual([]);
		expect(message.tabs).toEqual([]);
		expect(message.compositionWithheld).toBe(false);

		// And the discriminant is what separates "nothing here" from "not for
		// you" (R4): the same empty lists, one explicit bit apart.
		const withheld = decodeContract(GetCommissionResponseSchema, {
			...bare,
			compositionWithheld: true
		});
		expect(withheld.compositionWithheld).toBe(true);
		expect(withheld.elements).toEqual([]);
	});

	it('tolerates an element field from a later additive release (§6)', () => {
		// The contract grows additively; a client that breaks on a new field
		// makes that promise void. This is the tolerant reader applied to the
		// composition specifically, because the element envelope is where the
		// next fields land (typed payload arms, and whatever the type catalog
		// mints).
		const future = {
			...emitted,
			elements: [{ ...emitted.elements[0], aFieldFromTheFuture: 'additive' }]
		};

		const message = decodeContract(GetCommissionResponseSchema, future);
		expect(message.elements[0].kind).toBe('note');
	});
});
