//! Every published TOML example parses with the shipped parser (topology-config
//! spec; task 3.3 F).
//!
//! The README and ADR-0013 are the operator's reference; a fence that drifts from the
//! schema the parser accepts is a documentation bug. This harness extracts every
//! ```` ```toml ```` fence from both and feeds it to the shipped parser:
//!
//! - A **full example** — a fence that already carries an `[intent]` table — must
//!   parse verbatim through [`dpaa2_config::parse_str`], the full validate-and-convert
//!   path.
//! - A **fragment** — a lone construct block with no `[intent]` table — is wrapped
//!   with a `schema = 1` preamble and run through [`dpaa2_config::parse_schema`], the
//!   structural gate, since a fragment cannot resolve the tenants and constructs its
//!   siblings declare.
//!
//! If a fence turns out invalid, fix the document — not this test — unless the fence
//! is a deliberate negative example (there are none today).

const README: &str = include_str!("../../../README.md");
const ADR_0013: &str = include_str!("../../../docs/adr/0013-accepted-intent-vocabulary.md");

/// Extracts the body of every ```` ```toml ```` fenced block from Markdown `md`.
fn toml_fences(md: &str) -> Vec<String> {
    let mut fences = Vec::new();
    let mut current: Option<String> = None;
    for line in md.lines() {
        match &mut current {
            None => {
                if line.trim() == "```toml" {
                    current = Some(String::new());
                }
            }
            Some(body) => {
                if line.trim() == "```" {
                    fences.push(std::mem::take(body));
                    current = None;
                } else {
                    body.push_str(line);
                    body.push('\n');
                }
            }
        }
    }
    assert!(current.is_none(), "unterminated ```toml fence");
    fences
}

/// Parses one fence: a full example (has `[intent]`) verbatim, a fragment wrapped.
fn check_fence(source: &str, fence: &str) {
    if fence.contains("[intent]") {
        dpaa2_config::parse_str(fence)
            .unwrap_or_else(|e| panic!("{source}: full example must parse verbatim: {e}\n{fence}"));
    } else {
        let wrapped = format!("[intent]\nschema = 1\n{fence}");
        dpaa2_config::parse_schema(&wrapped).unwrap_or_else(|e| {
            panic!("{source}: fragment must parse structurally: {e}\n{wrapped}")
        });
    }
}

#[test]
fn readme_toml_examples_parse() {
    let fences = toml_fences(README);
    assert!(!fences.is_empty(), "README has at least one ```toml fence");
    for fence in &fences {
        check_fence("README.md", fence);
    }
}

#[test]
fn adr_0013_toml_examples_parse() {
    let fences = toml_fences(ADR_0013);
    assert!(!fences.is_empty(), "ADR-0013 has ```toml fences");
    for fence in &fences {
        check_fence("docs/adr/0013-accepted-intent-vocabulary.md", fence);
    }
}
