use pylinefix::{format_source, Options};

fn opts(line_length: usize) -> Options {
    Options {
        line_length,
        embedded_enabled: false, // most tests don't need shell-out tools
        ..Options::default()
    }
}

fn format(input: &str, opts: &Options) -> String {
    format_source(input, opts).expect("format")
}

fn assert_idempotent(formatted: &str, opts: &Options) {
    let again = format(formatted, opts);
    pretty_assertions::assert_eq!(again, formatted, "second pass changed output");
}

#[test]
fn splits_long_dict_value_string() {
    let input = r#"tooltips = {
    "Description": "Free-form description. Combined with German Name and Synonyms into the COMMENT ON COLUMN.",
}
"#;
    let opts = opts(88);
    let out = format(input, &opts);
    assert!(out.contains("\"Description\": ("), "{out}");
    assert!(out.contains("(\n        \""), "{out}");
    assert!(out.contains("    ),"), "{out}");
    for line in out.lines() {
        assert!(line.chars().count() <= 88, "line too long: {line:?}");
    }
    assert_idempotent(&out, &opts);
}

#[test]
fn does_not_split_typing_literal() {
    let input = r#"from typing import Literal

Mode = Literal["this is a very very very very very very very very very long literal"]
"#;
    let opts = opts(40);
    let out = format(input, &opts);
    pretty_assertions::assert_eq!(out, input);
}

#[test]
fn does_not_split_triple_quoted() {
    let input = "X = \"\"\"abcdefg hijklmn opqrstuvwxyz abcdefg hijklmn opqrstuvwxyz abcdefg hijklmn opqrstuv\"\"\"\n";
    let opts = opts(40);
    let out = format(input, &opts);
    pretty_assertions::assert_eq!(out, input);
}

#[test]
fn preserves_string_prefix() {
    let input = "X = f\"alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi omicron pi rho\"\n";
    let opts = opts(60);
    let out = format(input, &opts);
    assert!(out.contains("f\""), "f prefix lost: {out}");
    assert_idempotent(&out, &opts);
}

#[test]
fn skips_fstring_with_space_in_braces() {
    let input = "X = f\"alpha {a + b} gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi omicron\"\n";
    let opts = opts(50);
    let out = format(input, &opts);
    pretty_assertions::assert_eq!(out, input, "f-string with brace expression should not split");
}

#[test]
fn wraps_long_docstring_simple() {
    let input = r#"def f():
    """Fetch active users from the database, returning a list of User objects sorted by creation date descending, optionally filtered by tenant."""
    pass
"#;
    let opts = opts(88);
    let out = format(input, &opts);
    for line in out.lines() {
        assert!(line.chars().count() <= 88, "line too long: {line:?}\nOUT:\n{out}");
    }
    assert!(out.contains("\"\"\"\n"), "closing on its own line: {out}");
    assert_idempotent(&out, &opts);
}

#[test]
fn preserves_section_headers() {
    let input = r#"def f():
    """Do something.

    Args:
        x: A short description.

    Returns:
        Something.
    """
    pass
"#;
    let opts = opts(88);
    let out = format(input, &opts);
    pretty_assertions::assert_eq!(out, input);
}

#[test]
fn preserves_fenced_code_block() {
    let input = r#"def f():
    """Do something.

    Example:

    ```python
    do(it)
    ```
    """
    pass
"#;
    let opts = opts(88);
    let out = format(input, &opts);
    pretty_assertions::assert_eq!(out, input);
}

#[test]
fn skips_short_strings() {
    let input = "x = \"short\"\n";
    let opts = opts(88);
    let out = format(input, &opts);
    pretty_assertions::assert_eq!(out, input);
}

#[test]
fn long_string_in_function_call_no_extra_parens() {
    let input = r#"print(
    "alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi omicron pi rho",
)
"#;
    let opts = opts(60);
    let out = format(input, &opts);
    // First content char of split body should not be '(' — context already
    // provides parens via the function call.
    assert!(!out.contains("(\n        ("), "extra parens added: {out}");
    assert_idempotent(&out, &opts);
}

#[test]
fn wraps_long_prose_comment() {
    let input = "# This is a very long prose comment that exceeds the line length and should be wrapped at word boundaries by pylinefix.\n";
    let opts = opts(80);
    let out = format(input, &opts);
    for line in out.lines() {
        assert!(line.chars().count() <= 80, "line too long: {line:?}");
    }
    assert!(out.starts_with("# "), "{out}");
    assert_idempotent(&out, &opts);
}

#[test]
fn wraps_commented_out_code_too() {
    // Previously skipped via heuristic; user explicitly wants these wrapped.
    let input = "    # \"key\": \"value\",  # commented-out code with quotes is now wrapped at word boundaries\n";
    let opts = opts(80);
    let out = format(input, &opts);
    for line in out.lines() {
        assert!(line.chars().count() <= 80, "line too long: {line:?}\n{out}");
    }
    assert!(out.lines().count() >= 2, "should be wrapped: {out}");
    assert_idempotent(&out, &opts);
}

#[test]
fn skips_noqa_directive() {
    let input = "x = 1  # noqa: E501 some explanatory text after the directive that should be preserved verbatim\n";
    let opts = opts(80);
    let out = format(input, &opts);
    // It's an inline comment anyway, but also the directive should not be wrapped.
    pretty_assertions::assert_eq!(out, input);
}

#[test]
fn hoists_inline_trailing_comment() {
    let input = "x = some_value  # this is a long trailing comment that should be hoisted above\n";
    let opts = opts(60);
    let out = format(input, &opts);
    let lines: Vec<&str> = out.lines().collect();
    assert!(lines[0].starts_with("# "), "first line not comment: {out}");
    let code_line = lines.iter().find(|l| !l.starts_with('#')).copied();
    assert_eq!(code_line, Some("x = some_value"), "{out}");
    for line in &lines {
        assert!(line.chars().count() <= 60, "line too long: {line:?}\n{out}");
    }
    assert_idempotent(&out, &opts);
}

#[test]
fn does_not_hoist_noqa_directive() {
    let input = "x = some_value  # noqa: E501 long enough to push the whole line past the budget\n";
    let opts = opts(60);
    let out = format(input, &opts);
    pretty_assertions::assert_eq!(out, input, "noqa must stay attached: {out}");
}

#[test]
fn embedded_json_formatted_when_jq_present() {
    if which("jq").is_none() {
        eprintln!("skipping: jq not on PATH");
        return;
    }
    let opts = Options {
        line_length: 88,
        embedded_enabled: true,
        ..Options::default()
    };
    let input = "CONFIG_JSON = \"\"\"{\"host\":\"localhost\",\"port\":5432,\"options\":{\"timeout\":30,\"retry\":true}}\"\"\"\n";
    let out = format(input, &opts);
    assert!(out.contains("\"host\": \"localhost\""), "{out}");
    assert!(out.contains("\"options\": {"), "{out}");
    assert_idempotent(&out, &opts);
}

fn which(cmd: &str) -> Option<std::path::PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(cmd);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

#[test]
fn idempotent_combined() {
    let input = r#"def fetch():
    """Fetch active users from the database, returning a list of User objects sorted by creation date descending, optionally filtered by tenant."""
    return None


x = "alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi omicron pi rho"
"#;
    let opts = opts(80);
    let pass1 = format(input, &opts);
    let pass2 = format(&pass1, &opts);
    pretty_assertions::assert_eq!(pass1, pass2);
}
