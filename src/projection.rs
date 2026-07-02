/// Metadata for a single section produced by [`project`].
#[derive(Debug, Clone, PartialEq)]
pub struct SectionMeta {
    /// Full ancestry path of the section heading (same format as
    /// `Heading::heading_path` from `outline.rs`).
    pub heading_path: Vec<String>,
    /// Estimated token count for the section body (heading line included).
    ///
    /// The estimate uses the heuristic **`chars / 4`**, rounded down, which
    /// approximates the token count produced by common BPE tokenisers such as
    /// cl100k.  This is a documented estimate, not an exact count.
    pub token_estimate: usize,
}

/// Parse `src` and return one [`SectionMeta`] per heading, in document order.
///
/// Each entry covers the heading line itself plus all body lines up to (but
/// not including) the next sibling or ancestor heading — identical to the
/// slice returned by `read_section`.
///
/// The `token_estimate` field is `section_char_count / 4` (integer division).
pub fn project(src: &str) -> Vec<SectionMeta> {
    // Collect (line_index, level, heading_path) for every ATX heading.
    let mut stack: Vec<Option<String>> = vec![None; 7];
    let mut headings: Vec<(usize, u8, Vec<String>)> = Vec::new();

    for (line_idx, line) in src.lines().enumerate() {
        if let Some((level, text)) = parse_atx_heading(line) {
            for slot in stack.iter_mut().take(7).skip(level as usize) {
                *slot = None;
            }
            stack[level as usize] = Some(text.clone());
            let path: Vec<String> = (1..=level as usize)
                .filter_map(|i| stack[i].clone())
                .collect();
            headings.push((line_idx, level, path));
        }
    }

    if headings.is_empty() {
        return Vec::new();
    }

    let all_lines: Vec<&str> = src.lines().collect();
    let total_lines = all_lines.len();

    let mut result = Vec::with_capacity(headings.len());

    for (idx, (start_line, level, heading_path)) in headings.iter().enumerate() {
        // Determine end line: first heading at same or higher level after us,
        // OR the start of the next heading entry that terminates this section.
        // Simpler: scan forward for a heading with level <= ours.
        let end_line = headings[idx + 1..]
            .iter()
            .find(|(_, lvl, _)| *lvl <= *level)
            .map(|(li, _, _)| *li)
            .unwrap_or(total_lines);

        // Compute char count for the section slice.
        let section_chars: usize = all_lines[*start_line..end_line]
            .iter()
            .map(|l| l.chars().count() + 1) // +1 for the '\n' separator
            .sum();
        // Subtract the extra newline added after the last line if the source
        // has no trailing newline and this is the final section — a minor
        // correction that keeps estimates consistent.  In practice the
        // difference is at most 1 char so we accept it; documented as
        // approximate anyway.

        let token_estimate = section_chars / 4;

        result.push(SectionMeta {
            heading_path: heading_path.clone(),
            token_estimate,
        });
    }

    result
}

/// Minimal ATX heading parser (no external dependency).
fn parse_atx_heading(line: &str) -> Option<(u8, String)> {
    let line = line.trim_end();
    let hashes = line.bytes().take_while(|&b| b == b'#').count();
    if hashes == 0 || hashes > 6 {
        return None;
    }
    let rest = &line[hashes..];
    if rest.is_empty() {
        return Some((hashes as u8, String::new()));
    }
    if rest.as_bytes()[0] != b' ' {
        return None;
    }
    let text = rest[1..].trim();
    let text = text.trim_end_matches('#').trim_end();
    Some((hashes as u8, text.to_string()))
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── basic output shape ────────────────────────────────────────────────

    #[test]
    fn empty_document_returns_empty_vec() {
        assert_eq!(project(""), vec![]);
    }

    #[test]
    fn no_headings_returns_empty_vec() {
        assert_eq!(project("just some text\nno headings here\n"), vec![]);
    }

    #[test]
    fn single_h1_with_body() {
        // "# Hello\n" (8 chars) + "body\n" (5 chars) = 13 chars → 13/4 = 3
        let src = "# Hello\nbody\n";
        let result = project(src);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].heading_path, vec!["Hello"]);
        assert_eq!(result[0].token_estimate, 13 / 4);
    }

    #[test]
    fn two_sibling_h1s_each_cover_own_body() {
        // "# A\n" (4) + "alpha\n" (6) = 10 chars → 10/4 = 2
        // "# B\n" (4) + "beta\n" (5) = 9 chars → 9/4 = 2
        let src = "# A\nalpha\n# B\nbeta\n";
        let result = project(src);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].heading_path, vec!["A"]);
        assert_eq!(result[0].token_estimate, 10 / 4);
        assert_eq!(result[1].heading_path, vec!["B"]);
        assert_eq!(result[1].token_estimate, 9 / 4);
    }

    #[test]
    fn nested_headings_have_correct_paths() {
        let src = "# Top\n## Sub\nbody\n";
        let result = project(src);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].heading_path, vec!["Top"]);
        assert_eq!(result[1].heading_path, vec!["Top", "Sub"]);
    }

    #[test]
    fn parent_section_includes_child_heading_lines() {
        // "# Top\n" = 6 chars, then "## Sub\n" = 7 chars, "body\n" = 5 chars
        // Parent covers all three → 18 chars → 18/4 = 4
        let src = "# Top\n## Sub\nbody\n";
        let result = project(src);
        // Child: "## Sub\n" (7) + "body\n" (5) = 12 → 12/4 = 3
        assert_eq!(result[0].token_estimate, 18 / 4);
        assert_eq!(result[1].token_estimate, 12 / 4);
    }

    #[test]
    fn token_estimate_is_chars_div_4() {
        // Verify the documented formula: chars (including '\n') / 4.
        let src = "# Section\nsome content here\n";
        // "# Section\n" = 10, "some content here\n" = 19 → total 29 → 29/4 = 7
        let result = project(src);
        assert_eq!(result.len(), 1);
        let expected = 29 / 4;
        assert_eq!(result[0].token_estimate, expected);
    }

    #[test]
    fn heading_with_no_body_has_small_estimate() {
        // "# Empty\n" = 8 chars → 8/4 = 2
        let src = "# Empty\n# Next\n";
        let result = project(src);
        assert_eq!(result[0].heading_path, vec!["Empty"]);
        assert_eq!(result[0].token_estimate, 8 / 4);
    }

    #[test]
    fn three_level_nesting_paths() {
        let src = "# A\n## B\n### C\nleaf\n";
        let result = project(src);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].heading_path, vec!["A"]);
        assert_eq!(result[1].heading_path, vec!["A", "B"]);
        assert_eq!(result[2].heading_path, vec!["A", "B", "C"]);
    }

    #[test]
    fn document_order_preserved() {
        let src = "# Z\n# A\n# M\n";
        let result = project(src);
        assert_eq!(result[0].heading_path, vec!["Z"]);
        assert_eq!(result[1].heading_path, vec!["A"]);
        assert_eq!(result[2].heading_path, vec!["M"]);
    }
}
