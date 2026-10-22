//! CI guard: no actor port takes or returns a `NodeId`.
//!
//! Actors are addressed by DID; a node id is internal to the Den. The guard reads every
//! port of `identity` and every port of `kernel` but the node port itself, and refuses a
//! function signature naming `NodeId`, directly or through a renamed import or a type alias
//! made anywhere in either crate. A source-text scan: an alias compiles fine.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// The node key no actor port may carry.
const NODE_ID: &str = "NodeId";

/// The names of unit-test and property-test files, which hold no port.
const TEST_FILES: [&str; 2] = ["tests.rs", "proptests.rs"];

/// One token of a Rust source: an identifier, or one punctuation character. Comments,
/// literals, numbers and lifetimes leave none.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    Ident(String),
    Punct(char),
}

fn is_ident_char(character: char) -> bool {
    character.is_alphanumeric() || character == '_'
}

/// The index past the identifier characters starting at `at`.
fn word_end(chars: &[char], at: usize) -> usize {
    let length = chars[at..]
        .iter()
        .take_while(|character| is_ident_char(**character))
        .count();
    at + length
}

/// The index past the escaped literal whose body starts at `at` and ends at `close`.
fn escaped_end(chars: &[char], mut at: usize, close: char) -> usize {
    while at < chars.len() {
        match chars[at] {
            '\\' => at += 2,
            character if character == close => return at + 1,
            _ => at += 1,
        }
    }
    at
}

/// The index past the raw string whose hashes start at `at`.
fn raw_string_end(chars: &[char], at: usize) -> usize {
    let hashes = chars[at..].iter().take_while(|c| **c == '#').count();
    let mut index = at + hashes + 1;
    while index < chars.len() {
        let closes = chars[index] == '"'
            && chars[index + 1..]
                .iter()
                .take(hashes)
                .filter(|c| **c == '#')
                .count()
                == hashes;
        if closes {
            return index + 1 + hashes;
        }
        index += 1;
    }
    index
}

/// The index past the block comment opening at `at`; block comments nest.
fn block_comment_end(chars: &[char], mut at: usize) -> usize {
    let mut depth = 0;
    while at < chars.len() {
        let pair = (chars[at], chars.get(at + 1).copied());
        match pair {
            ('/', Some('*')) => {
                depth += 1;
                at += 2;
            }
            ('*', Some('/')) => {
                depth -= 1;
                at += 2;
                if depth == 0 {
                    return at;
                }
            }
            _ => at += 1,
        }
    }
    at
}

/// The index past the character literal or lifetime whose quote is at `at`.
fn quote_end(chars: &[char], at: usize) -> usize {
    if chars.get(at + 1) == Some(&'\\') {
        return escaped_end(chars, at + 3, '\'');
    }
    if chars.get(at + 2) == Some(&'\'') {
        return at + 3;
    }
    word_end(chars, at + 1)
}

/// The tokens of `source`.
fn tokens(source: &str) -> Vec<Token> {
    let chars: Vec<char> = source.chars().collect();
    let mut out = Vec::new();
    let mut at = 0;
    while at < chars.len() {
        let character = chars[at];
        let next = chars.get(at + 1).copied();
        if character.is_whitespace() {
            at += 1;
        } else if character == '/' && next == Some('/') {
            at += chars[at..].iter().take_while(|c| **c != '\n').count();
        } else if character == '/' && next == Some('*') {
            at = block_comment_end(&chars, at);
        } else if character == '"' {
            at = escaped_end(&chars, at + 1, '"');
        } else if character == '\'' {
            at = quote_end(&chars, at);
        } else if character.is_ascii_digit() {
            at = word_end(&chars, at);
        } else if is_ident_char(character) {
            let end = word_end(&chars, at);
            let word: String = chars[at..end].iter().collect();
            let after = chars.get(end).copied();
            let raw_prefix = matches!(word.as_str(), "r" | "br" | "cr");
            let string_prefix = matches!(word.as_str(), "b" | "c");
            let raw_ident = word == "r"
                && after == Some('#')
                && chars.get(end + 1).is_some_and(|c| is_ident_char(*c));
            if raw_ident {
                let ident_end = word_end(&chars, end + 1);
                out.push(Token::Ident(chars[end + 1..ident_end].iter().collect()));
                at = ident_end;
            } else if raw_prefix && matches!(after, Some('"' | '#')) {
                at = raw_string_end(&chars, end);
            } else if string_prefix && after == Some('"') {
                at = escaped_end(&chars, end + 1, '"');
            } else if word == "b" && after == Some('\'') {
                at = quote_end(&chars, end);
            } else {
                out.push(Token::Ident(word));
                at = end;
            }
        } else {
            out.push(Token::Punct(character));
            at += 1;
        }
    }
    out
}

fn is_ident(token: &Token, name: &str) -> bool {
    matches!(token, Token::Ident(ident) if ident == name)
}

/// The tokens from `start` to the first of `stops` outside parentheses, brackets and braces.
fn up_to<'a>(tokens: &'a [Token], start: usize, stops: &[char]) -> &'a [Token] {
    let mut depth = 0_i32;
    for (index, token) in tokens.iter().enumerate().skip(start) {
        let Token::Punct(punct) = token else {
            continue;
        };
        if depth == 0 && stops.contains(punct) {
            return &tokens[start..index];
        }
        match punct {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            _ => {}
        }
    }
    &tokens[start..]
}

/// A function's signature: its tokens from `fn` to its body or its `;`.
fn signature(tokens: &[Token], start: usize) -> &[Token] {
    up_to(tokens, start, &[';', '{'])
}

/// A `use` or `type` item: its tokens up to its `;`.
fn statement(tokens: &[Token], start: usize) -> &[Token] {
    up_to(tokens, start, &[';'])
}

/// Every name `tokens` bind to a forbidden one: `use … X as Y;` (never `_`) and
/// `type Y = …X…;`.
fn aliases_in(tokens: &[Token], forbidden: &BTreeSet<String>, out: &mut BTreeSet<String>) {
    for (index, token) in tokens.iter().enumerate() {
        if is_ident(token, "use") {
            let statement = statement(tokens, index);
            for window in statement.windows(3) {
                let [Token::Ident(original), as_word, Token::Ident(bound)] = window else {
                    continue;
                };
                if is_ident(as_word, "as") && forbidden.contains(original) && bound != "_" {
                    out.insert(bound.clone());
                }
            }
        }
        if is_ident(token, "type") {
            let Some(Token::Ident(bound)) = tokens.get(index + 1) else {
                continue;
            };
            let item = statement(tokens, index);
            let Some(equals) = item.iter().position(|token| *token == Token::Punct('=')) else {
                continue;
            };
            let names_forbidden = item[equals..]
                .iter()
                .any(|token| matches!(token, Token::Ident(ident) if forbidden.contains(ident)));
            if names_forbidden {
                out.insert(bound.clone());
            }
        }
    }
}

/// `NodeId` and every name `sources` bind to it, through any chain of aliases.
fn forbidden_names(sources: &[Vec<Token>]) -> BTreeSet<String> {
    let mut forbidden = BTreeSet::from([NODE_ID.to_owned()]);
    loop {
        let mut grown = forbidden.clone();
        for source in sources {
            aliases_in(source, &forbidden, &mut grown);
        }
        if grown == forbidden {
            return forbidden;
        }
        forbidden = grown;
    }
}

/// The name of each function in `tokens` whose signature names a forbidden name.
fn signatures_naming(tokens: &[Token], forbidden: &BTreeSet<String>) -> Vec<String> {
    let mut out = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        let Some(Token::Ident(name)) = tokens.get(index + 1) else {
            continue;
        };
        if !is_ident(token, "fn") {
            continue;
        }
        let signature = signature(tokens, index);
        let names_forbidden = signature
            .iter()
            .any(|token| matches!(token, Token::Ident(ident) if forbidden.contains(ident)));
        if names_forbidden {
            out.push(name.clone());
        }
    }
    out
}

/// The `fn` of each port in `ports` whose signature names a node id, as `<file>: fn <name>`;
/// aliases come from `ports` and `elsewhere` both.
fn offenders(ports: &[(String, String)], elsewhere: &[String]) -> Vec<String> {
    let port_tokens: Vec<(String, Vec<Token>)> = ports
        .iter()
        .map(|(file, source)| (file.clone(), tokens(source)))
        .collect();
    let all_tokens: Vec<Vec<Token>> = port_tokens
        .iter()
        .map(|(_, tokens)| tokens.clone())
        .chain(elsewhere.iter().map(|source| tokens(source)))
        .collect();
    let forbidden = forbidden_names(&all_tokens);
    port_tokens
        .iter()
        .flat_map(|(file, tokens)| {
            signatures_naming(tokens, &forbidden)
                .into_iter()
                .map(move |name| format!("{file}: fn {name}"))
        })
        .collect()
}

fn crates() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the crates folder")
        .to_path_buf()
}

/// Every `*.rs` file under `dir`, recursively, test files left out.
fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let entries = std::fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("guard cannot read {}: {error}", dir.display()));
    for entry in entries {
        let path = entry.expect("dir entry").path();
        let is_test_file = path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| TEST_FILES.contains(&name));
        if path.is_dir() {
            out.extend(rust_files(&path));
        } else if path.extension().is_some_and(|extension| extension == "rs") && !is_test_file {
            out.push(path);
        }
    }
    out
}

/// The port files of `krate`: `src/ports.rs` and every file under `src/ports/`.
fn port_files(krate: &str) -> Vec<PathBuf> {
    let src = crates().join(krate).join("src");
    let facade = src.join("ports.rs");
    assert!(facade.is_file(), "guard expects `{krate}/src/ports.rs`");
    let mut files = rust_files(&src.join("ports"));
    files.push(facade);
    files
}

/// The node port's own files, which the guard leaves out.
fn node_port_files() -> Vec<PathBuf> {
    let ports = crates().join("kernel/src/ports");
    let mut files = vec![ports.join("node.rs")];
    let leaves = ports.join("node");
    if leaves.is_dir() {
        files.extend(rust_files(&leaves));
    }
    files
}

/// The actor ports: every port file of `identity` and `kernel`, the node port's left out.
fn actor_port_files() -> Vec<PathBuf> {
    let left_out = node_port_files();
    port_files("identity")
        .into_iter()
        .chain(port_files("kernel"))
        .filter(|path| !left_out.contains(path))
        .collect()
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("guard cannot read {}: {error}", path.display()))
}

#[test]
fn the_guard_reads_identity_s_ports_and_kernel_s_but_the_node_port() {
    let files = actor_port_files();

    let user_port = crates().join("identity/src/ports/user.rs");
    let node_port = crates().join("kernel/src/ports/node.rs");
    assert!(files.contains(&user_port), "{files:#?}");
    assert!(node_port.is_file());
    assert!(!files.contains(&node_port), "{files:#?}");
}

#[test]
fn no_actor_port_takes_or_returns_a_node_id() {
    let ports: Vec<(String, String)> = actor_port_files()
        .iter()
        .map(|path| (path.display().to_string(), read(path)))
        .collect();
    let elsewhere: Vec<String> = ["kernel", "identity"]
        .iter()
        .flat_map(|krate| rust_files(&crates().join(krate).join("src")))
        .map(|path| read(&path))
        .collect();

    let found = offenders(&ports, &elsewhere);

    assert!(
        found.is_empty(),
        "actor ports take or return a node id — an actor is addressed by DID:\n  {}",
        found.join("\n  ")
    );
}

fn port(source: &str) -> (String, String) {
    ("port.rs".to_owned(), source.to_owned())
}

#[test]
fn the_guard_refuses_a_node_id_however_it_is_named() {
    let planted = [
        (
            "find",
            "pub trait UserStore { async fn find(&self, id: &NodeId) -> anyhow::Result<()>; }",
        ),
        (
            "find",
            "pub trait UserStore { fn find(&self) -> anyhow::Result<Option<kernel::elements::node::NodeId>>; }",
        ),
        (
            "find",
            "pub trait UserStore { fn find(&self, id: r#NodeId); }",
        ),
        (
            "find",
            "use kernel::elements::node::NodeId as Address; pub trait UserStore { fn find(&self, id: &Address); }",
        ),
        (
            "find",
            "use crate::elements::node::{NodeRow, NodeId as Address}; pub trait UserStore { fn find(&self, id: Address); }",
        ),
        (
            "find",
            "pub type Address = NodeId; pub trait UserStore { fn find(&self) -> Address; }",
        ),
        (
            "all",
            "type Address = NodeId; type Many = Vec<Address>; pub trait UserStore { fn all(&self) -> Many; }",
        ),
        (
            "each",
            "pub trait UserStore { fn each(&self, visit: &dyn Fn(NodeId)); }",
        ),
    ];

    for (name, source) in planted {
        let found = offenders(&[port(source)], &[]);

        let expected = [format!("port.rs: fn {name}")];
        assert_eq!(found, expected, "{source}");
    }
}

#[test]
fn an_alias_made_in_another_file_is_seen() {
    let elsewhere = ["pub type Address = super::node::NodeId;".to_owned()];
    let planted = port("pub trait UserStore { fn find(&self, id: &Address); }");

    let found = offenders(&[planted], &elsewhere);

    assert_eq!(found, ["port.rs: fn find"]);
}

#[test]
fn the_guard_passes_what_names_no_node_id() {
    let allowed = [
        "pub trait UserStore { fn find(&self, did: &Did) -> Option<UserRow>; }",
        "/// Never a NodeId.\npub trait UserStore { fn find(&self, did: &Did); }",
        "pub trait UserStore { fn find(&self, did: &Did); } // NodeId",
        "pub trait UserStore { /* NodeId */ fn find(&self, did: &Did); }",
        "pub trait UserStore { fn kind(&self) -> &'static str { \"NodeId\" } }",
        "pub trait UserStore { fn first(&self) -> char { 'N' } fn find(&self, did: &Did); }",
        "use crate::elements::node::NodeId as _; pub trait UserStore { fn find(&self, id: &_); }",
        "pub trait UserStore { fn find(&self, id: &NodeIdentity); }",
    ];

    for source in allowed {
        let found = offenders(&[port(source)], &[]);
        assert_eq!(found, Vec::<String>::new(), "{source}");
    }
}
