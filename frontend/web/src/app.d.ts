// See https://svelte.dev/docs/kit/types#app.d.ts
// for information about these interfaces
import type { Problem } from '$lib/api/problem';

declare global {
	namespace App {
		// interface Error {}
		// interface Locals {}
		// interface PageData {}
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
