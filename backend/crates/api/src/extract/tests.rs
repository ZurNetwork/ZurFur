use std::path::{Path, PathBuf};

/// A route file the scan reads: its path under `src/routes` and its text.
struct RouteFile {
    relative: PathBuf,
    text: String,
}

/// The directory the scan reads: every route module.
fn routes_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("routes")
}

/// Names each `.rs` file under `dir`, recursively.
fn route_paths(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("the scan cannot read {}: {e}", dir.display()));
    for entry in entries {
        let path = entry.expect("dir entry").path();
        if path.is_dir() {
            route_paths(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// Reads every `.rs` file under `src/routes`.
fn route_files() -> Vec<RouteFile> {
    let routes = routes_dir();
    let mut paths = Vec::new();
    route_paths(&routes, &mut paths);
    paths
        .into_iter()
        .map(|path| {
            let text = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("the scan cannot read {}: {e}", path.display()));
            let relative = path
                .strip_prefix(&routes)
                .expect("a scanned file sits under src/routes")
                .to_path_buf();
            RouteFile { relative, text }
        })
        .collect()
}

/// Whether `byte` can sit inside an identifier: an ASCII letter, digit or `_`.
fn is_ident_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// Whether `line` is a `//` comment line, `///` and `//!` docs included.
fn is_comment_line(line: &str) -> bool {
    line.trim_start().starts_with("//")
}

/// A file's code lines joined with newlines, remembering where each one came from.
struct CodeText {
    joined: String,
    /// Each joined line's starting byte offset and its 1-based source line number.
    starts: Vec<(usize, usize)>,
}

impl From<&str> for CodeText {
    /// Drops the comment lines of a file's text and joins the rest.
    fn from(text: &str) -> Self {
        let mut joined = String::new();
        let mut starts = Vec::new();
        let code_lines = text
            .lines()
            .enumerate()
            .filter(|(_, line)| !is_comment_line(line));
        for (index, line) in code_lines {
            if !starts.is_empty() {
                joined.push('\n');
            }
            starts.push((joined.len(), index + 1));
            joined.push_str(line);
        }
        Self { joined, starts }
    }
}

impl CodeText {
    /// The source line number of the byte at `offset` in the joined text.
    fn line_of(&self, offset: usize) -> usize {
        let index = self.starts.partition_point(|(start, _)| *start <= offset) - 1;
        self.starts[index].1
    }
}

/// The text after a `Path` word when it opens a generic argument: `<…` or `::<…`,
/// from just past the `<`.
fn opened_generic(after: &str) -> Option<&str> {
    let unturbofished = after.strip_prefix("::").unwrap_or(after);
    unturbofished.trim_start().strip_prefix('<')
}

/// The generic argument that starts `opened`, up to its matching `>`, or the
/// rest of the text when it never closes.
fn generic_argument(opened: &str) -> &str {
    let mut depth = 0_usize;
    for (offset, byte) in opened.bytes().enumerate() {
        match byte {
            b'<' => depth += 1,
            b'>' if depth == 0 => return &opened[..offset],
            b'>' => depth -= 1,
            _ => {}
        }
    }
    opened
}

/// Whether `text` holds the whole word `CommissionId`.
fn names_commission_id(text: &str) -> bool {
    text.match_indices("CommissionId").any(|(start, word)| {
        let before = &text[..start];
        let after = &text[start + word.len()..];
        !before.bytes().next_back().is_some_and(is_ident_byte)
            && !after.bytes().next().is_some_and(is_ident_byte)
    })
}

/// The lines of `text` where a `Path<…>` starts whose generic argument names
/// `CommissionId`. Comment lines are dropped first, so a type may span lines.
fn raw_commission_id_path_lines(text: &str) -> Vec<usize> {
    let code = CodeText::from(text);
    code.joined
        .match_indices("Path")
        .filter(|(start, word)| {
            let before = &code.joined[..*start];
            let after = &code.joined[start + word.len()..];
            let whole_word = !before.bytes().next_back().is_some_and(is_ident_byte);
            whole_word
                && opened_generic(after)
                    .is_some_and(|opened| names_commission_id(generic_argument(opened)))
        })
        .map(|(start, _)| code.line_of(start))
        .collect()
}

#[test]
fn a_commission_id_inside_a_path_extractor_is_flagged() {
    let snippets = [
        "Path(commission_id): Path<CommissionId>,",
        "Path((commission_id, file_key)): Path<(CommissionId, FileKey)>,",
        "Path((file_key, commission_id)): Path<(FileKey, CommissionId)>,",
        "axum::extract::Path<domain::elements::commission::CommissionId>",
        "Path::<CommissionId>::from_request_parts(parts, state)",
        "Path<(FileKey, Option<CommissionId>)>",
        "Path<\n    (CommissionId, FileKey),\n>",
        "Path<(\n    // the commission\n    CommissionId,\n    FileKey,\n)>",
    ];
    let first_line = vec![1];

    for snippet in snippets {
        assert_eq!(
            raw_commission_id_path_lines(snippet),
            first_line,
            "the scan should flag {snippet:?}"
        );
    }
}

#[test]
fn comments_commission_path_and_other_path_extractors_pass() {
    let snippets = [
        "// Path<CommissionId>",
        "/// Path<(CommissionId, FileKey)>",
        "    //! Path<CommissionId>",
        "CommissionPath(commission_id): CommissionPath,",
        "Path((_, file_key)): Path<(IgnoredAny, FileKey)>,",
        "Path(account_id): Path<AccountId>,",
        "Path(file_key): Path<FileKey>, commission_id: CommissionId,",
        "Path<CommissionIds>",
        "RawPathParams<CommissionId>",
        "let commission_id: CommissionId = raw.parse()?;",
    ];
    let no_lines: Vec<usize> = Vec::new();

    for snippet in snippets {
        assert_eq!(
            raw_commission_id_path_lines(snippet),
            no_lines,
            "the scan flagged {snippet:?}"
        );
    }
}

#[test]
fn a_raw_commission_id_path_is_reported_at_its_source_line() {
    let snippet = "// A comment line.\nlet a = 1;\n\n/// Docs.\nasync fn handler(\n    Path(id): Path<CommissionId>,\n) {}";
    let path_line = vec![6];

    assert_eq!(raw_commission_id_path_lines(snippet), path_line);
}

/// Every commission route reads its id through `CommissionPath`, so a bad id
/// answers the commission's one 404. No code line under `src/routes` puts
/// `CommissionId` inside a `Path<…>`.
#[test]
fn every_route_reads_a_commission_id_through_commission_path() {
    let files = route_files();
    let delete_route = Path::new("commissions").join("delete.rs");
    assert!(
        files.iter().any(|file| file.relative == delete_route),
        "the scan missed {} among {} route files",
        delete_route.display(),
        files.len(),
    );

    let raw_paths: Vec<String> = files
        .iter()
        .flat_map(|file| {
            raw_commission_id_path_lines(&file.text)
                .into_iter()
                .map(|line| format!("routes/{}:{line}", file.relative.display()))
        })
        .collect();

    assert!(
        raw_paths.is_empty(),
        "a commission id is read through CommissionPath, never Path<…>:\n  {}",
        raw_paths.join("\n  "),
    );
}
