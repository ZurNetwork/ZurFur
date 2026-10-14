use super::*;

/// An untyped startup-phase message carrying `code`, then `rest`.
fn startup(code: i32, rest: &[u8]) -> Vec<u8> {
    let length = i32::try_from(8 + rest.len()).expect("a short message");
    let mut message = length.to_be_bytes().to_vec();
    message.extend_from_slice(&code.to_be_bytes());
    message.extend_from_slice(rest);
    message
}

/// The protocol 3.0 startup message for user `postgres`.
fn startup_message() -> Vec<u8> {
    startup(196_608, b"user\0postgres\0\0")
}

/// A typed message of `kind` around `body`.
fn typed(kind: u8, body: &[u8]) -> Vec<u8> {
    let length = i32::try_from(4 + body.len()).expect("a short message");
    let mut message = vec![kind];
    message.extend_from_slice(&length.to_be_bytes());
    message.extend_from_slice(body);
    message
}

/// `text` NUL-terminated.
fn text(text: &str) -> Vec<u8> {
    let mut bytes = text.as_bytes().to_vec();
    bytes.push(0);
    bytes
}

fn parse(name: &str, sql: &str) -> Vec<u8> {
    let mut body = text(name);
    body.extend(text(sql));
    body.extend(0_i16.to_be_bytes());
    typed(b'P', &body)
}

/// A Bind of `statement` to `portal` with `parameters`, all in the binary format.
fn bind(portal: &str, statement: &str, parameters: &[Option<&[u8]>]) -> Vec<u8> {
    let mut body = text(portal);
    body.extend(text(statement));
    body.extend(1_i16.to_be_bytes());
    body.extend(1_i16.to_be_bytes());
    let count = i16::try_from(parameters.len()).expect("a few parameters");
    body.extend(count.to_be_bytes());
    for parameter in parameters {
        match parameter {
            Some(bytes) => {
                let length = i32::try_from(bytes.len()).expect("a short value");
                body.extend(length.to_be_bytes());
                body.extend_from_slice(bytes);
            }
            None => body.extend((-1_i32).to_be_bytes()),
        }
    }
    body.extend(0_i16.to_be_bytes());
    typed(b'B', &body)
}

fn execute(portal: &str) -> Vec<u8> {
    let mut body = text(portal);
    body.extend(0_i32.to_be_bytes());
    typed(b'E', &body)
}

fn sync() -> Vec<u8> {
    typed(b'S', &[])
}

/// `bytes` fed one byte at a time, so no message arrives whole.
fn fed_bytewise(reader: &mut FrontendReader, bytes: &[u8]) -> Vec<Statement> {
    bytes.chunks(1).flat_map(|byte| reader.feed(byte)).collect()
}

#[test]
fn a_simple_query_after_the_startup_is_one_statement() {
    let mut stream = startup_message();
    stream.extend(typed(b'Q', &text("BEGIN")));
    let mut reader = FrontendReader::default();

    let executed = reader.feed(&stream);

    let begin = Statement {
        sql: "BEGIN".to_owned(),
        parameters: Vec::new(),
    };
    assert_eq!(executed, [begin]);
}

#[test]
fn an_encryption_request_keeps_the_startup_phase_open() {
    let mut stream = startup(SSL_REQUEST, &[]);
    stream.extend(startup_message());
    stream.extend(typed(b'Q', &text("SELECT 1")));
    let mut reader = FrontendReader::default();

    let executed = fed_bytewise(&mut reader, &stream);

    let sql: Vec<&str> = executed
        .iter()
        .map(|statement| statement.sql.as_str())
        .collect();
    assert_eq!(sql, ["SELECT 1"]);
}

#[test]
fn an_execute_logs_the_bound_statement_with_its_parameters_once_per_execute() {
    let mut stream = startup_message();
    stream.extend(parse("sqlx_s_1", "SELECT $1, $2"));
    stream.extend(typed(b'D', &text("Ssqlx_s_1")));
    stream.extend(sync());
    stream.extend(bind("", "sqlx_s_1", &[Some(b"abc"), None]));
    stream.extend(execute(""));
    stream.extend(sync());
    stream.extend(bind("", "sqlx_s_1", &[Some(b"xyz"), Some(b"")]));
    stream.extend(execute(""));
    stream.extend(sync());
    let mut reader = FrontendReader::default();

    let executed = fed_bytewise(&mut reader, &stream);

    let first = Statement {
        sql: "SELECT $1, $2".to_owned(),
        parameters: vec![Some(b"abc".to_vec()), None],
    };
    let second = Statement {
        sql: "SELECT $1, $2".to_owned(),
        parameters: vec![Some(b"xyz".to_vec()), Some(Vec::new())],
    };
    assert_eq!(executed, [first, second]);
}

#[test]
fn a_parse_and_describe_without_an_execute_logs_nothing() {
    let mut stream = startup_message();
    stream.extend(parse("sqlx_s_2", "SELECT 2"));
    stream.extend(typed(b'D', &text("Ssqlx_s_2")));
    stream.extend(sync());
    let mut reader = FrontendReader::default();

    let executed = reader.feed(&stream);

    assert_eq!(executed, []);
}

#[test]
fn a_closed_statement_is_forgotten_so_binding_it_is_marked() {
    let mut stream = startup_message();
    stream.extend(parse("sqlx_s_3", "SELECT 3"));
    let mut close = vec![b'S'];
    close.extend(text("sqlx_s_3"));
    stream.extend(typed(b'C', &close));
    stream.extend(bind("", "sqlx_s_3", &[]));
    stream.extend(execute(""));
    let mut reader = FrontendReader::default();

    let executed = reader.feed(&stream);

    let marked: Vec<bool> = executed.iter().map(Statement::is_unreadable).collect();
    assert_eq!(marked, [true, true], "{executed:?}");
}

#[test]
fn a_statement_read_from_a_query_file_is_named_by_it() {
    let sql = include_str!("../../../../adapter-pg/queries/commission/find_summary.sql");
    let summary = Statement {
        sql: sql.to_owned(),
        parameters: Vec::new(),
    };
    let ad_hoc = Statement {
        sql: "SELECT\n  1".to_owned(),
        parameters: Vec::new(),
    };

    assert_eq!(summary.to_string(), "commission/find_summary");
    assert_eq!(ad_hoc.to_string(), "SELECT 1");
}

#[test]
fn a_parse_bind_and_execute_in_one_chunk_log_the_statement() {
    let mut reader = FrontendReader::default();
    reader.feed(&startup_message());
    let mut chunk = parse("sqlx_s_4", "SELECT $1");
    chunk.extend(bind("", "sqlx_s_4", &[Some(b"one")]));
    chunk.extend(execute(""));
    chunk.extend(sync());
    chunk.extend(typed(b'Q', &text("COMMIT")));

    let executed = reader.feed(&chunk);

    let select = Statement {
        sql: "SELECT $1".to_owned(),
        parameters: vec![Some(b"one".to_vec())],
    };
    let commit = Statement {
        sql: "COMMIT".to_owned(),
        parameters: Vec::new(),
    };
    assert_eq!(executed, [select, commit]);
}

#[test]
fn a_bind_it_cannot_read_is_marked_and_the_next_execute_never_replays_the_last_portal() {
    let mut reader = FrontendReader::default();
    reader.feed(&startup_message());
    let mut first = parse("sqlx_s_5", "SELECT 5");
    first.extend(bind("", "sqlx_s_5", &[]));
    first.extend(execute(""));
    // A Bind whose parameter count promises a value its body doesn't carry.
    let mut broken_body = text("");
    broken_body.extend(text("sqlx_s_5"));
    broken_body.extend(0_i16.to_be_bytes());
    broken_body.extend(1_i16.to_be_bytes());
    let mut second = typed(b'B', &broken_body);
    second.extend(execute(""));

    let before = reader.feed(&first);
    let after = reader.feed(&second);

    let sql: Vec<&str> = before
        .iter()
        .map(|statement| statement.sql.as_str())
        .collect();
    assert_eq!(sql, ["SELECT 5"]);
    let marked: Vec<bool> = after.iter().map(Statement::is_unreadable).collect();
    assert_eq!(marked, [true, true], "{after:?}");
}

#[test]
fn a_bind_of_an_unknown_statement_is_marked() {
    let mut reader = FrontendReader::default();
    reader.feed(&startup_message());
    let mut stream = bind("", "never_parsed", &[]);
    stream.extend(execute(""));

    let executed = reader.feed(&stream);

    let marked: Vec<bool> = executed.iter().map(Statement::is_unreadable).collect();
    assert_eq!(marked, [true, true], "{executed:?}");
}

#[test]
fn a_length_no_message_has_marks_every_later_chunk() {
    let mut reader = FrontendReader::default();
    reader.feed(&startup_message());
    let mut broken = vec![b'Q'];
    broken.extend(2_i32.to_be_bytes());

    let at_the_break = reader.feed(&broken);
    let later = reader.feed(&typed(b'Q', &text("SELECT 6")));

    assert!(
        at_the_break.iter().all(Statement::is_unreadable),
        "{at_the_break:?}"
    );
    assert!(later.iter().all(Statement::is_unreadable), "{later:?}");
    assert!(!later.is_empty());
}
