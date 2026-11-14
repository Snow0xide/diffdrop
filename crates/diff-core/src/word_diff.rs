//! Highlights the words that changed between two paired lines.
//!
//! Spans are measured in UTF-16 code units from the start of the line, so the
//! frontend can pass them straight to `String.prototype.slice`.

use std::time::{Duration, Instant};

use serde::Serialize;
use similar::{Algorithm, DiffTag, capture_diff_slices_deadline};

/// Upper bound for a single line; a timed out diff is still valid, only coarser.
const LINE_BUDGET: Duration = Duration::from_millis(20);

/// Changed ranges of one modified line, as `[start, end)` pairs.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct WordChanges {
    pub old: Vec<[u32; 2]>,
    pub new: Vec<[u32; 2]>,
}

struct Token<'a> {
    text: &'a str,
    start: u32,
    end: u32,
}

#[derive(PartialEq, Eq)]
enum Class {
    Word,
    Space,
    Other,
}

fn class_of(c: char) -> Class {
    if c.is_alphanumeric() || c == '_' {
        Class::Word
    } else if c.is_whitespace() {
        Class::Space
    } else {
        Class::Other
    }
}

/// Runs of word characters and runs of whitespace become one token each;
/// every other character is a token by itself.
fn tokenize(line: &str) -> Vec<Token<'_>> {
    let mut tokens: Vec<Token<'_>> = Vec::new();
    let mut utf16 = 0u32;
    let mut current: Option<(usize, u32, Class)> = None;
    for (byte, c) in line.char_indices() {
        let class = class_of(c);
        let joins =
            matches!(&current, Some((_, _, prev)) if *prev == class && class != Class::Other);
        if !joins {
            if let Some((from, from16, _)) = current.take() {
                tokens.push(Token {
                    text: &line[from..byte],
                    start: from16,
                    end: utf16,
                });
            }
            current = Some((byte, utf16, class));
        }
        utf16 += c.len_utf16() as u32;
    }
    if let Some((from, from16, _)) = current {
        tokens.push(Token {
            text: &line[from..],
            start: from16,
            end: utf16,
        });
    }
    tokens
}

/// The tokens of a line other than whitespace, sorted, for similarity checks.
pub fn sorted_words(line: &str) -> Vec<&str> {
    let mut words: Vec<&str> = tokenize(line)
        .into_iter()
        .map(|t| t.text)
        .filter(|t| !t.chars().all(char::is_whitespace))
        .collect();
    words.sort_unstable();
    words
}

pub fn word_changes(old: &str, new: &str, ignore_whitespace: bool) -> WordChanges {
    let keep = |t: &Token<'_>| !ignore_whitespace || !t.text.chars().all(char::is_whitespace);
    let old_tokens: Vec<Token<'_>> = tokenize(old).into_iter().filter(keep).collect();
    let new_tokens: Vec<Token<'_>> = tokenize(new).into_iter().filter(keep).collect();
    let old_text: Vec<&str> = old_tokens.iter().map(|t| t.text).collect();
    let new_text: Vec<&str> = new_tokens.iter().map(|t| t.text).collect();

    let deadline = Some(Instant::now() + LINE_BUDGET);
    let ops = capture_diff_slices_deadline(Algorithm::Myers, &old_text, &new_text, deadline);

    let mut changes = WordChanges::default();
    for op in ops {
        let (tag, old_range, new_range) = op.as_tag_tuple();
        if tag == DiffTag::Equal {
            continue;
        }
        push_span(&mut changes.old, &old_tokens[old_range]);
        push_span(&mut changes.new, &new_tokens[new_range]);
    }
    changes
}

fn push_span(spans: &mut Vec<[u32; 2]>, tokens: &[Token<'_>]) {
    let (Some(first), Some(last)) = (tokens.first(), tokens.last()) else {
        return;
    };
    match spans.last_mut() {
        Some(prev) if prev[1] == first.start => prev[1] = last.end,
        _ => spans.push([first.start, last.end]),
    }
}
