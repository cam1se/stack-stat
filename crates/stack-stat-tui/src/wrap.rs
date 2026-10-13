//! Character-counted word wrapping for terminal content.

/// Wrap text greedily at spaces, hard-breaking tokens wider than `width`.
///
/// Widths smaller than one are treated as one character. The returned lines are
/// never empty for non-empty input.
#[must_use]
pub fn wrap(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    if text.is_empty() {
        return Vec::new();
    }
    if text.trim().is_empty() {
        let mut lines = Vec::new();
        let mut current = String::new();
        let mut current_chars = 0;
        for character in text.chars() {
            current.push(character);
            current_chars += 1;
            if current_chars == width {
                lines.push(std::mem::take(&mut current));
                current_chars = 0;
            }
        }
        if current_chars > 0 {
            lines.push(current);
        }
        return lines;
    }
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut current_chars = 0;
    for word in text.split(' ').filter(|word| !word.is_empty()) {
        let word_chars = word.chars().count();
        if current_chars > 0 && current_chars + 1 + word_chars <= width {
            current.push(' ');
            current.push_str(word);
            current_chars += 1 + word_chars;
            continue;
        }
        if current_chars > 0 {
            lines.push(std::mem::take(&mut current));
            current_chars = 0;
        }

        let mut chunk = String::new();
        let mut chunk_chars = 0;
        for character in word.chars() {
            chunk.push(character);
            chunk_chars += 1;
            if chunk_chars == width {
                lines.push(std::mem::take(&mut chunk));
                chunk_chars = 0;
            }
        }
        if chunk_chars > 0 {
            current = chunk;
            current_chars = chunk_chars;
        }
    }
    if current_chars > 0 {
        lines.push(current);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::wrap;

    #[test]
    fn hard_breaks_long_branch_names() {
        assert_eq!(
            wrap("feature/long-branch-name", 7),
            ["feature", "/long-b", "ranch-n", "ame"]
        );
    }

    #[test]
    fn wraps_multiple_words_greedily() {
        assert_eq!(wrap("one two three", 7), ["one two", "three"]);
    }

    #[test]
    fn keeps_an_exact_fit_on_one_line() {
        assert_eq!(wrap("exact fit", 9), ["exact fit"]);
    }
}
