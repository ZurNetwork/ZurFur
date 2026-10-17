-- Bug reports a signed-in User files from the web app: what went wrong, and
-- the page and browser it went wrong in. App-private; a report is written once
-- and never changed.
--
-- id             App-minted UUIDv7, so it sorts by filing time. A reporter
--                quotes its last 8 characters.
-- reporter_id    The reporter's DID, from the session alone. The foreign key
--                onto `users` refuses any other actor kind: `users (id, kind)`
--                is kind-checked against `actor_identity`.
-- description    What went wrong: trimmed, 1 to 4,000 code points.
-- page, page_kind, web_version, browser
--                Each one tick box: NULL when unticked or not sent, never an
--                empty string, and capped in bytes. A page never starts with
--                `//`, which a browser reads as another host, and holds no
--                backslash, which a browser reads as `/`.
-- window_width, window_height
--                One tick box: both set or both NULL; 1 to 100,000 pixels.
-- reduced_motion, more_contrast, forced_colors
--                One tick box: all three set or all three NULL.
-- error_status   Only when filed from an error page: 400 to 599.
-- api_version    The receiving API's build id, or "unknown": 1 to 64 bytes.
-- received_at    The server's clock when the report arrived.
CREATE TABLE bug_report (
    id             uuid        PRIMARY KEY,
    reporter_id    text        NOT NULL,
    description    text        NOT NULL,
    page           text,
    page_kind      text,
    web_version    text,
    browser        text,
    window_width   integer,
    window_height  integer,
    reduced_motion boolean,
    more_contrast  boolean,
    forced_colors  boolean,
    error_status   smallint,
    api_version    text        NOT NULL,
    received_at    timestamptz NOT NULL,

    CONSTRAINT bug_report_reporter
        FOREIGN KEY (reporter_id) REFERENCES users (id),

    CONSTRAINT bug_report_description_length
        CHECK (char_length(description) BETWEEN 1 AND 4000),

    CONSTRAINT bug_report_page_max_bytes
        CHECK (octet_length(page) <= 2048),
    CONSTRAINT bug_report_page_starts_with_slash
        CHECK (left(page, 1) = '/'),
    CONSTRAINT bug_report_page_no_query_or_fragment
        CHECK (strpos(page, '?') = 0 AND strpos(page, '#') = 0),
    CONSTRAINT bug_report_page_not_empty
        CHECK (octet_length(page) >= 1),
    CONSTRAINT bug_report_page_not_protocol_relative
        CHECK (left(page, 2) <> '//'),
    CONSTRAINT bug_report_page_no_backslash
        CHECK (position(E'\\' in page) = 0),

    CONSTRAINT bug_report_page_kind_max_bytes
        CHECK (octet_length(page_kind) <= 64),
    CONSTRAINT bug_report_page_kind_not_empty
        CHECK (octet_length(page_kind) >= 1),

    CONSTRAINT bug_report_web_version_max_bytes
        CHECK (octet_length(web_version) <= 64),
    CONSTRAINT bug_report_web_version_not_empty
        CHECK (octet_length(web_version) >= 1),

    CONSTRAINT bug_report_browser_max_bytes
        CHECK (octet_length(browser) <= 512),
    CONSTRAINT bug_report_browser_not_empty
        CHECK (octet_length(browser) >= 1),

    CONSTRAINT bug_report_window_width_range
        CHECK (window_width BETWEEN 1 AND 100000),
    CONSTRAINT bug_report_window_height_range
        CHECK (window_height BETWEEN 1 AND 100000),
    CONSTRAINT bug_report_window_both_or_neither
        CHECK ((window_width IS NULL) = (window_height IS NULL)),

    CONSTRAINT bug_report_display_all_or_none
        CHECK (num_nulls(reduced_motion, more_contrast, forced_colors) IN (0, 3)),

    CONSTRAINT bug_report_error_status_range
        CHECK (error_status BETWEEN 400 AND 599),

    CONSTRAINT bug_report_api_version_bytes
        CHECK (octet_length(api_version) BETWEEN 1 AND 64)
);

-- The rate limit's count: one reporter's reports since a moment. It also
-- serves the `users` foreign key, which Postgres does not index on its own.
CREATE INDEX bug_report_by_reporter ON bug_report (reporter_id, received_at);
