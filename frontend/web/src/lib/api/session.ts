/**
 * The component-facing session shape. Since DD 39944194 the session *calls*
 * live server-side ({@link import('../server/session')}); this type is the
 * plain data that crosses the runes seam into layouts and components.
 */

/**
 * The JSON `/me` contract (ZMVP-151 slice 1): `did` always present for a
 * live session; the profile fields are `null` when the PDS was unreachable
 * and nothing was cached — callers fall back to showing the DID. The wire
 * schema decoding into this shape is `SessionSchema` in the server's
 * `ZurfurApi` — its `me` signature is the compile-time parity proof.
 */
export interface Session {
	did: string;
	handle: string | null;
	display_name: string | null;
	avatar_url: string | null;
}
