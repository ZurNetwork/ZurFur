/**
 * The component-facing Account shapes — plain data mapped down from the
 * generated `account_pb` messages at the `ZurfurApi` boundary
 * ({@link import('../server/api/zurfur-api')}), mirroring how {@link import('./session').Session}
 * is handled. Generated types never cross the runes seam (DD 39944194).
 *
 * `id`/`handle`/`did` are branded ({@link import('../types/brand')}), not
 * bare `string` — the decode boundary casts through the trusted-decode
 * constructors (`accountId`, `handleFromTrusted`, `did`), so a bare string
 * can no longer be passed where one of these is expected by accident.
 */

import type { AccountId, Did, Handle } from '$lib/types/brand';

/**
 * One row of `GET /accounts`: an account the caller holds a role in, with
 * the caller's OWN role riding along — load-bearing for the detail screen's
 * delete affordance.
 */
export interface AccountMembership {
	id: AccountId;
	did: Did;
	handle: Handle;
	name: string;
	/**
	 * Extensible string vocabulary (`contract/VERSIONING.md` R8): `owner` |
	 * `admin` | `manager` | `member`, open to future values. Passed through
	 * as-is — an unrecognized value is NOT normalized here. The fail-closed
	 * rule R8 requires (no delete affordance for a role this client build
	 * doesn't recognize) is the rendering screen's job; this type only makes
	 * the value expressible.
	 */
	role: string;
}

/** The account `POST /accounts` founded — the caller becomes its Owner. */
export interface CreatedAccount {
	id: AccountId;
	did: Did;
	handle: Handle;
	name: string;
}

/**
 * Which deletion `DELETE /accounts/{id}` performed (DD 23003138): `'soft'`
 * (fact-bearing — row kept, handle reserved, DID live) or `'hard'`
 * (fact-free — row gone, handle freed, DID tombstoned separately/async).
 * `'unknown'` is the R8-required defined fallback for an outcome value this
 * client build doesn't recognize yet — it must never be read as confirming
 * either soft or hard deletion.
 */
export type DeleteOutcome = 'soft' | 'hard' | 'unknown';
