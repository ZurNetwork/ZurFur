/**
 * Every registered HTTP status code (RFC 9110 + IANA registry), named — so
 * call sites read `HttpStatus.UnprocessableContent` instead of a magic 422.
 *
 * Deliberately an `as const` object rather than a `const enum`: under
 * SvelteKit's `isolatedModules` the compiler cannot inline const-enum members
 * across files (Vite transpiles one file at a time), so the construct
 * silently loses its only advantage — while this shape is fully erasable
 * syntax with the same call-site ergonomics.
 */
export const HttpStatus = {
	// 1xx — informational
	Continue: 100,
	SwitchingProtocols: 101,
	Processing: 102,
	EarlyHints: 103,
	// 2xx — success
	Ok: 200,
	Created: 201,
	Accepted: 202,
	NonAuthoritativeInformation: 203,
	NoContent: 204,
	ResetContent: 205,
	PartialContent: 206,
	MultiStatus: 207,
	AlreadyReported: 208,
	ImUsed: 226,
	// 3xx — redirection
	MultipleChoices: 300,
	MovedPermanently: 301,
	Found: 302,
	SeeOther: 303,
	NotModified: 304,
	TemporaryRedirect: 307,
	PermanentRedirect: 308,
	// 4xx — client error
	BadRequest: 400,
	Unauthorized: 401,
	PaymentRequired: 402,
	Forbidden: 403,
	NotFound: 404,
	MethodNotAllowed: 405,
	NotAcceptable: 406,
	ProxyAuthenticationRequired: 407,
	RequestTimeout: 408,
	Conflict: 409,
	Gone: 410,
	LengthRequired: 411,
	PreconditionFailed: 412,
	ContentTooLarge: 413,
	UriTooLong: 414,
	UnsupportedMediaType: 415,
	RangeNotSatisfiable: 416,
	ExpectationFailed: 417,
	ImATeapot: 418,
	MisdirectedRequest: 421,
	UnprocessableContent: 422,
	Locked: 423,
	FailedDependency: 424,
	TooEarly: 425,
	UpgradeRequired: 426,
	PreconditionRequired: 428,
	TooManyRequests: 429,
	RequestHeaderFieldsTooLarge: 431,
	UnavailableForLegalReasons: 451,
	// 5xx — server error
	InternalServerError: 500,
	NotImplemented: 501,
	BadGateway: 502,
	ServiceUnavailable: 503,
	GatewayTimeout: 504,
	HttpVersionNotSupported: 505,
	VariantAlsoNegotiates: 506,
	InsufficientStorage: 507,
	LoopDetected: 508,
	NotExtended: 510,
	NetworkAuthenticationRequired: 511
} as const;

/** The union of every registered status code value. */
export type HttpStatus = (typeof HttpStatus)[keyof typeof HttpStatus];
