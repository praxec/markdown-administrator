/// Errors returned by [`read_section`].
#[derive(Debug, Clone, PartialEq)]
pub enum SectionError {
    /// No heading matched the supplied path.
    PathNotFound,
    /// Multiple sibling headings share the same text at the same level;
    /// the `Vec<usize>` holds the 0-based line indices of the duplicates.
    Ambiguous(Vec<usize>),
}

impl std::fmt::Display for SectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SectionError::PathNotFound => write!(f, "section path not found"),
            SectionError::Ambiguous(lines) => {
                write!(f, "ambiguous section: duplicates at lines {:?}", lines)
            }
        }
    }
}

impl std::error::Error for SectionError {}

/// Return the exact source lines for the section identified by `heading_path`.
///
/// The returned string runs from the heading line up to (but not including)
/// the next heading at the same or higher level (lower level number),
/// preserving original line endings.
///
/// # Errors
/// * [`SectionError::PathNotFound`] — no heading matches the path.
/// * [`SectionError::Ambiguous`]   — two or more sibling headings have
///   identical text at the same level under the same parent.
pub fn read_section(src: &str, heading_path: &[String]) -> Result<String, SectionError> {
    if heading_path.is_empty() {
        return Err(SectionError::PathNotFound);
    }

    // Collect all headings annotated with their source-line index.
    let headings_with_lines: Vec<(usize, u8, Vec<String>)> = {
        let mut stack: Vec<Option<String>> = vec![None; 7];
        let mut acc = Vec::new();
        for (line_idx, line) in src.lines().enumerate() {
            if let Some((level, text)) = parse_atx_heading(line) {
                for slot in stack.iter_mut().take(7).skip(level as usize) {
                    *slot = None;
                }
                stack[level as usize] = Some(text.clone());

                let path: Vec<String> = (1..=level as usize)
                    .filter_map(|i| stack[i].clone())
                    .collect();

                acc.push((line_idx, level, path));
            }
        }
        acc
    };

    // Find candidates whose heading_path matches.
    let candidates: Vec<&(usize, u8, Vec<String>)> = headings_with_lines
        .iter()
        .filter(|(_, _, path)| path == heading_path)
        .collect();

    match candidates.len() {
        0 => Err(SectionError::PathNotFound),
        1 => {
            let (start_line, level, _) = candidates[0];
            let section_text = extract_section(src, *start_line, *level);
            Ok(section_text)
        }
        _ => {
            let line_indices: Vec<usize> = candidates.iter().map(|(li, _, _)| *li).collect();
            Err(SectionError::Ambiguous(line_indices))
        }
    }
}

/// Extract lines from `start_line` up to (not including) the next heading
/// whose level <= `section_level`.
fn extract_section(src: &str, start_line: usize, section_level: u8) -> String {
    let lines: Vec<&str> = src.lines().collect();
    let mut end_line = lines.len(); // exclusive

    for (i, line) in lines.iter().enumerate().skip(start_line + 1) {
        if let Some((lvl, _)) = parse_atx_heading(line) {
            if lvl <= section_level {
                end_line = i;
                break;
            }
        }
    }

    let selected: Vec<&str> = lines[start_line..end_line].to_vec();
    if selected.is_empty() {
        return String::new();
    }

    // Check whether the original source ends with a newline.
    let trailing_newline = src.ends_with('\n');
    let last_selected_is_last_of_src = end_line == lines.len();

    let mut out = selected.join("\n");
    if last_selected_is_last_of_src {
        if trailing_newline {
            out.push('\n');
        }
    } else {
        // There are more lines after this section — always add newline.
        out.push('\n');
    }
    out
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

#[cfg(test)]
mod tests {
    use super::*;

    fn path(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|s| s.to_string()).collect()
    }

    // ── basic section extraction ───────────────────────────────────────────

    #[test]
    fn returns_single_h1_section() {
        let src = "# Hello\nsome text\nmore text\n";
        let result = read_section(src, &path(&["Hello"])).unwrap();
        assert_eq!(result, "# Hello\nsome text\nmore text\n");
    }

    #[test]
    fn section_ends_before_next_same_level_heading() {
        let src = "# First\ntext1\n# Second\ntext2\n";
        let result = read_section(src, &path(&["First"])).unwrap();
        assert_eq!(result, "# First\ntext1\n");
    }

    #[test]
    fn section_ends_before_higher_level_heading() {
        // H2 section ends when H1 appears.
        let src = "# Top\n## Sub\nbody\n# Next Top\n";
        let result = read_section(src, &path(&["Top", "Sub"])).unwrap();
        assert_eq!(result, "## Sub\nbody\n");
    }

    #[test]
    fn nested_subsection_not_included_at_parent_boundary() {
        let src = "# Top\n## Sub\nbody\n# Other\n";
        let result = read_section(src, &path(&["Top"])).unwrap();
        assert_eq!(result, "# Top\n## Sub\nbody\n");
    }

    // ── PathNotFound ───────────────────────────────────────────────────────

    #[test]
    fn path_not_found_returns_error() {
        let src = "# Hello\ntext\n";
        let err = read_section(src, &path(&["Goodbye"])).unwrap_err();
        assert_eq!(err, SectionError::PathNotFound);
    }

    #[test]
    fn empty_path_returns_not_found() {
        let src = "# Hello\n";
        let err = read_section(src, &path(&[])).unwrap_err();
        assert_eq!(err, SectionError::PathNotFound);
    }

    #[test]
    fn wrong_parent_returns_not_found() {
        let src = "# Top\n## Sub\n";
        let err = read_section(src, &path(&["Other", "Sub"])).unwrap_err();
        assert_eq!(err, SectionError::PathNotFound);
    }

    // ── Ambiguous ─────────────────────────────────────────────────────────

    #[test]
    fn duplicate_sibling_headings_return_ambiguous() {
        let src = "# Intro\n## Notes\nfirst\n## Notes\nsecond\n";
        let err = read_section(src, &path(&["Intro", "Notes"])).unwrap_err();
        match err {
            SectionError::Ambiguous(lines) => {
                assert_eq!(lines.len(), 2);
                assert_eq!(lines[0], 1);
                assert_eq!(lines[1], 3);
            }
            other => panic!("expected Ambiguous, got {:?}", other),
        }
    }

    #[test]
    fn duplicate_top_level_headings_return_ambiguous() {
        let src = "# FAQ\nq1\n# FAQ\nq2\n";
        let err = read_section(src, &path(&["FAQ"])).unwrap_err();
        match err {
            SectionError::Ambiguous(lines) => {
                assert_eq!(lines.len(), 2);
                assert_eq!(lines[0], 0);
                assert_eq!(lines[1], 2);
            }
            other => panic!("expected Ambiguous, got {:?}", other),
        }
    }

    // ── edge cases ────────────────────────────────────────────────────────

    #[test]
    fn section_with_no_body_returns_just_heading() {
        let src = "# Empty\n# Next\n";
        let result = read_section(src, &path(&["Empty"])).unwrap();
        assert_eq!(result, "# Empty\n");
    }

    #[test]
    fn three_level_nesting() {
        let src = "# A\n## B\n### C\nleaf body\n### D\nother\n";
        let result = read_section(src, &path(&["A", "B", "C"])).unwrap();
        assert_eq!(result, "### C\nleaf body\n");
    }
}
