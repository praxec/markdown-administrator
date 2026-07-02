/// Errors returned by [`replace_section`] and [`delete_section`].
#[derive(Debug, Clone, PartialEq)]
pub enum PatchError {
    /// No heading matched the supplied path.
    PathNotFound,
    /// Multiple sibling headings share the same text at the same level.
    Ambiguous(Vec<usize>),
}

impl std::fmt::Display for PatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PatchError::PathNotFound => write!(f, "section path not found"),
            PatchError::Ambiguous(lines) => {
                write!(f, "ambiguous section: duplicates at lines {:?}", lines)
            }
        }
    }
}

impl std::error::Error for PatchError {}

/// Replace the body of the section identified by `heading_path` with
/// `new_body`, leaving every other byte in `src` byte-identical.
///
/// The heading line itself is kept unchanged; `new_body` replaces all lines
/// that followed the heading up to (but not including) the next sibling or
/// ancestor heading.  If `new_body` does not end with a newline one is
/// appended automatically unless `new_body` is empty.
///
/// # Errors
/// * [`PatchError::PathNotFound`] — no heading matches the path.
/// * [`PatchError::Ambiguous`]   — two or more sibling headings have
///   identical text at the same level under the same parent.
pub fn replace_section(
    src: &str,
    heading_path: &[String],
    new_body: &str,
) -> Result<String, PatchError> {
    let (before, heading_line, _old_body, after) = split_section(src, heading_path)?;

    let mut result = String::with_capacity(src.len());
    result.push_str(&before);
    result.push_str(&heading_line);
    if !new_body.is_empty() {
        result.push_str(new_body);
        if !new_body.ends_with('\n') {
            result.push('\n');
        }
    }
    result.push_str(&after);
    Ok(result)
}

/// Delete the section identified by `heading_path` (heading line + body)
/// from `src`, leaving every other byte byte-identical.
///
/// # Errors
/// * [`PatchError::PathNotFound`] — no heading matches the path.
/// * [`PatchError::Ambiguous`]   — two or more sibling headings have
///   identical text at the same level under the same parent.
pub fn delete_section(src: &str, heading_path: &[String]) -> Result<String, PatchError> {
    let (before, _heading_line, _old_body, after) = split_section(src, heading_path)?;

    let mut result = String::with_capacity(before.len() + after.len());
    result.push_str(&before);
    result.push_str(&after);
    Ok(result)
}

// ── internals ────────────────────────────────────────────────────────────────

/// Parse an ATX heading.  Returns `Some((level, text))` or `None`.
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

/// Split `src` around the target section.
///
/// Returns `(before, heading_line, old_body, after)` where:
/// * `before`       — everything up to (not including) the target heading line
/// * `heading_line` — the heading line itself, with trailing `\n`
/// * `old_body`     — the body lines of the section (may be empty)
/// * `after`        — everything from the next sibling/ancestor heading onward
///
/// All four parts concatenated reconstruct `src` byte-for-byte.
fn split_section(
    src: &str,
    heading_path: &[String],
) -> Result<(String, String, String, String), PatchError> {
    if heading_path.is_empty() {
        return Err(PatchError::PathNotFound);
    }

    // Build (line_index, level, full_path) for every heading.
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

    // Candidates matching the path.
    let candidates: Vec<&(usize, u8, Vec<String>)> = headings_with_lines
        .iter()
        .filter(|(_, _, p)| p == heading_path)
        .collect();

    match candidates.len() {
        0 => return Err(PatchError::PathNotFound),
        1 => {}
        _ => {
            let line_indices: Vec<usize> = candidates.iter().map(|(li, _, _)| *li).collect();
            return Err(PatchError::Ambiguous(line_indices));
        }
    }

    let (start_line, level, _) = candidates[0];
    let start_line = *start_line;
    let level = *level;

    // Collect all lines, preserving original endings.
    // `src.lines()` strips endings, so we reconstruct using the original bytes.
    let raw_lines: Vec<&str> = collect_raw_lines(src);

    // Find the end of the section (first heading at level <= section level
    // after start_line).
    let mut end_line = raw_lines.len(); // exclusive
    for i in (start_line + 1)..raw_lines.len() {
        // Trim the raw line for heading detection (handles \r\n).
        if let Some((lvl, _)) =
            parse_atx_heading(raw_lines[i].trim_end_matches(['\r', '\n']))
        {
            if lvl <= level {
                end_line = i;
                break;
            }
        }
    }

    // Reconstruct the four parts by joining raw lines.
    let before: String = raw_lines[..start_line].concat();
    let heading_line: String = raw_lines[start_line].to_string();
    let old_body: String = raw_lines[start_line + 1..end_line].concat();
    let after: String = raw_lines[end_line..].concat();

    // Ensure heading_line ends with a newline (it always does unless it is
    // the very last line with no trailing newline in the source).
    let heading_line = if heading_line.ends_with('\n') {
        heading_line
    } else {
        format!("{}\n", heading_line)
    };

    Ok((before, heading_line, old_body, after))
}

/// Split `src` into raw lines that each include their trailing `\n` (or `\r\n`).
/// The last line is included even if it has no trailing newline.
fn collect_raw_lines(src: &str) -> Vec<&str> {
    let mut lines = Vec::new();
    let mut start = 0;
    let bytes = src.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\n' {
            lines.push(&src[start..=i]);
            start = i + 1;
        }
        i += 1;
    }
    if start < src.len() {
        lines.push(&src[start..]);
    }
    lines
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn p(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|s| s.to_string()).collect()
    }

    // ── replace_section ───────────────────────────────────────────────────

    #[test]
    fn replace_body_of_sole_h1() {
        let src = "# Hello\nold body\n";
        let result = replace_section(src, &p(&["Hello"]), "new body\n").unwrap();
        assert_eq!(result, "# Hello\nnew body\n");
    }

    #[test]
    fn replace_leaves_surrounding_sections_byte_identical() {
        let src = "# First\nbefore\n# Target\nold\n# Last\nafter\n";
        let result = replace_section(src, &p(&["Target"]), "new content\n").unwrap();
        assert_eq!(
            result,
            "# First\nbefore\n# Target\nnew content\n# Last\nafter\n"
        );
    }

    #[test]
    fn replace_nested_section() {
        let src = "# Top\n## Sub\nold sub body\n## Other\nkept\n";
        let result = replace_section(src, &p(&["Top", "Sub"]), "new sub body\n").unwrap();
        assert_eq!(result, "# Top\n## Sub\nnew sub body\n## Other\nkept\n");
    }

    #[test]
    fn replace_with_empty_body_removes_body() {
        let src = "# A\nbody\n# B\n";
        let result = replace_section(src, &p(&["A"]), "").unwrap();
        assert_eq!(result, "# A\n# B\n");
    }

    #[test]
    fn replace_appends_newline_if_missing() {
        let src = "# A\nold\n";
        let result = replace_section(src, &p(&["A"]), "no newline").unwrap();
        assert_eq!(result, "# A\nno newline\n");
    }

    #[test]
    fn replace_returns_path_not_found() {
        let src = "# Real\nbody\n";
        let err = replace_section(src, &p(&["Missing"]), "x").unwrap_err();
        assert_eq!(err, PatchError::PathNotFound);
    }

    #[test]
    fn replace_returns_ambiguous() {
        let src = "# Dup\nbody1\n# Dup\nbody2\n";
        let err = replace_section(src, &p(&["Dup"]), "x").unwrap_err();
        match err {
            PatchError::Ambiguous(lines) => assert_eq!(lines.len(), 2),
            other => panic!("expected Ambiguous, got {:?}", other),
        }
    }

    // ── delete_section ────────────────────────────────────────────────────

    #[test]
    fn delete_sole_section() {
        let src = "# Hello\nbody\n";
        let result = delete_section(src, &p(&["Hello"])).unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn delete_leaves_surrounding_sections_byte_identical() {
        let src = "# First\nbefore\n# Target\nremove me\n# Last\nafter\n";
        let result = delete_section(src, &p(&["Target"])).unwrap();
        assert_eq!(result, "# First\nbefore\n# Last\nafter\n");
    }

    #[test]
    fn delete_nested_section() {
        let src = "# Top\n## Remove\nbody\n## Keep\nkept body\n";
        let result = delete_section(src, &p(&["Top", "Remove"])).unwrap();
        assert_eq!(result, "# Top\n## Keep\nkept body\n");
    }

    #[test]
    fn delete_returns_path_not_found() {
        let src = "# Real\n";
        let err = delete_section(src, &p(&["Ghost"])).unwrap_err();
        assert_eq!(err, PatchError::PathNotFound);
    }

    #[test]
    fn delete_returns_ambiguous() {
        let src = "# Dup\nfirst\n# Dup\nsecond\n";
        let err = delete_section(src, &p(&["Dup"])).unwrap_err();
        match err {
            PatchError::Ambiguous(lines) => assert_eq!(lines.len(), 2),
            other => panic!("expected Ambiguous, got {:?}", other),
        }
    }

    // ── roundtrip / byte-identity ─────────────────────────────────────────

    #[test]
    fn before_region_is_byte_identical_after_replace() {
        let src = "# A\nbefore content line1\nbefore content line2\n# B\nchange me\n# C\nafter\n";
        let result = replace_section(src, &p(&["B"]), "replaced\n").unwrap();
        // The "# A\n..." prefix must be unchanged.
        assert!(result.starts_with("# A\nbefore content line1\nbefore content line2\n"));
        // The "# C\n..." suffix must be unchanged.
        assert!(result.ends_with("# C\nafter\n"));
    }

    #[test]
    fn after_region_is_byte_identical_after_delete() {
        let src = "# Keep1\nk1 body\n# Delete\nremove\n# Keep2\nk2 body\n";
        let result = delete_section(src, &p(&["Delete"])).unwrap();
        assert_eq!(result, "# Keep1\nk1 body\n# Keep2\nk2 body\n");
    }
}
