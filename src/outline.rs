/// A parsed ATX markdown heading.
#[derive(Debug, Clone, PartialEq)]
pub struct Heading {
    /// 1-based heading level (1 = `#`, 2 = `##`, …, 6 = `######`).
    pub level: u8,
    /// The trimmed text of the heading.
    pub text: String,
    /// Full ancestry path ending with `text` itself.
    /// e.g. `["Top", "Sub", "LeafHeading"]`
    pub heading_path: Vec<String>,
}

/// Parse all ATX headings in `src` and return them in document order,
/// each annotated with its ancestor path.
pub fn outline(src: &str) -> Vec<Heading> {
    // Stack tracks the most-recently-seen heading at each level (1-indexed).
    // stack[0] is unused; stack[i] = text of the last heading at level i.
    let mut stack: Vec<Option<String>> = vec![None; 7]; // indices 0-6
    let mut result = Vec::new();

    for line in src.lines() {
        if let Some((level, text)) = parse_atx_heading(line) {
            // Any deeper (or equal) levels are now out of scope.
            for i in (level as usize)..=6 {
                stack[i] = None;
            }
            stack[level as usize] = Some(text.clone());

            // Build the heading_path from level 1 up to `level`.
            let heading_path: Vec<String> = (1..=level as usize)
                .filter_map(|i| stack[i].clone())
                .collect();

            result.push(Heading {
                level,
                text,
                heading_path,
            });
        }
    }
    result
}

/// Returns `Some((level, trimmed_text))` when `line` is an ATX heading,
/// `None` otherwise.
fn parse_atx_heading(line: &str) -> Option<(u8, String)> {
    let line = line.trim_end();
    let hashes = line.bytes().take_while(|&b| b == b'#').count();
    if hashes == 0 || hashes > 6 {
        return None;
    }
    let rest = &line[hashes..];
    // Must be followed by a space (or be an empty heading).
    if rest.is_empty() {
        return Some((hashes as u8, String::new()));
    }
    if rest.as_bytes()[0] != b' ' {
        return None;
    }
    let text = rest[1..].trim();
    // Strip optional closing `#` sequence.
    let text = text.trim_end_matches('#').trim_end();
    Some((hashes as u8, text.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── RED slice: basic flat outline ──────────────────────────────────────

    #[test]
    fn empty_document_gives_no_headings() {
        assert_eq!(outline(""), vec![]);
    }

    #[test]
    fn single_h1() {
        let h = outline("# Hello");
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].level, 1);
        assert_eq!(h[0].text, "Hello");
        assert_eq!(h[0].heading_path, vec!["Hello"]);
    }

    #[test]
    fn flat_sequence_of_h2s() {
        let src = "## Alpha\n## Beta\n## Gamma";
        let h = outline(src);
        assert_eq!(h.len(), 3);
        // No H1, so each H2's path is just itself.
        assert_eq!(h[0].heading_path, vec!["Alpha"]);
        assert_eq!(h[1].heading_path, vec!["Beta"]);
        assert_eq!(h[2].heading_path, vec!["Gamma"]);
    }

    // ── RED slice: nested heading_path ─────────────────────────────────────

    #[test]
    fn nested_h1_h2_h3() {
        let src = "# Top\n## Sub\n### Leaf";
        let h = outline(src);
        assert_eq!(h.len(), 3);

        assert_eq!(h[0].heading_path, vec!["Top"]);
        assert_eq!(h[1].heading_path, vec!["Top", "Sub"]);
        assert_eq!(h[2].heading_path, vec!["Top", "Sub", "Leaf"]);
    }

    #[test]
    fn sibling_resets_deeper_path() {
        // After "## Beta", the "### Leaf2" should use Beta, not Alpha.
        let src = "# Top\n## Alpha\n### Leaf1\n## Beta\n### Leaf2";
        let h = outline(src);
        assert_eq!(h.len(), 5);
        assert_eq!(h[4].heading_path, vec!["Top", "Beta", "Leaf2"]);
    }

    #[test]
    fn h2_without_h1_parent() {
        let src = "## Orphan\n### Child";
        let h = outline(src);
        // No H1, so the path starts at H2.
        assert_eq!(h[0].heading_path, vec!["Orphan"]);
        assert_eq!(h[1].heading_path, vec!["Orphan", "Child"]);
    }

    #[test]
    fn non_heading_lines_ignored() {
        let src = "Some text\n# Title\nMore text\n## Section\nBody";
        let h = outline(src);
        assert_eq!(h.len(), 2);
        assert_eq!(h[0].text, "Title");
        assert_eq!(h[1].text, "Section");
    }

    #[test]
    fn heading_without_space_after_hash_is_not_a_heading() {
        // CommonMark: `#Title` is NOT a heading.
        let h = outline("#Title");
        assert_eq!(h, vec![]);
    }

    #[test]
    fn closing_hashes_stripped() {
        let h = outline("## Section ##");
        assert_eq!(h[0].text, "Section");
    }
}
