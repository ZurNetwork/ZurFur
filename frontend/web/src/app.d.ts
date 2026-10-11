// See https://svelte.dev/docs/kit/types#app.d.ts
// for information about these interfaces
import type { Problem } from '$lib/api/problem';
import type { Session } from '$lib/api/session';
import type { Trail } from '$lib/api/trail';

declare global {
	namespace App {
		// interface Error {}
		/** What the server hooks hand the loads of one request. */
		interface Locals {
			/** The signed-in visitor, set by the sign-in gate on every `(session)` route. */
			session?: Session | undefined;
		}

		/** What any page may hand the frame. */
		interface PageData {
			/** The path bar's steps down to this page; a page without one shows an empty path. */
			trail?: Trail | undefined;
		}

		// interface PageState {}
		// interface Platform {}

		namespace Superforms {
			/**
			 * The default status-message type for every superform: a backend
			 * RFC 9457 Problem (the one-channel rule — values, field errors,
			 * and the backend Problem all ride the form). Declared once here so
			 * no call site needs the `superValidate<…, Problem>` generic.
			 */
			type Message = Problem;
		}
	}
}

export {};
