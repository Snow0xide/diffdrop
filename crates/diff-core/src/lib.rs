//! Diff engine for diffdrop. It knows nothing about the user interface: it
//! takes two texts and returns line operations, word highlights and stats.

mod decode;
mod line_diff;
mod lines;
mod word_diff;

use serde::{Deserialize, Serialize};
use similar::DiffOp;

pub use decode::{Decoded, Encoding, decode};
pub use lines::{Eol, EolStyle, Line, eol_style, split_lines};
pub use word_diff::WordChanges;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Options {
    /// Lines that differ only in whitespace count as equal.
    pub ignore_whitespace: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum OpKind {
    Equal,
    Delete,
    Insert,
    /// Modified lines: always as many old lines as new lines, each old line
    /// paired with the new line at the same offset.
    Replace,
}

/// A run of lines. Ops are contiguous: together they cover every line of the
/// old text and every line of the new text exactly once, in order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Op {
    pub kind: OpKind,
    pub old_start: usize,
    pub old_len: usize,
    pub new_start: usize,
    pub new_len: usize,
    /// For `Replace`: one entry per paired line.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub words: Vec<WordChanges>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    pub added: usize,
    pub removed: usize,
    /// Share of lines in common, from 0 to 1.
    pub similarity: f64,
    pub old_eol: EolStyle,
    pub new_eol: EolStyle,
    /// Both files have line breaks and they use different conventions.
    pub eol_differs: bool,
}

#[derive(Debug, Clone)]
pub struct Diff {
    pub old_lines: Vec<Line>,
    pub new_lines: Vec<Line>,
    pub ops: Vec<Op>,
    pub stats: Stats,
}

pub fn diff(old: &str, new: &str, options: Options) -> Diff {
    let old_lines = split_lines(old);
    let new_lines = split_lines(new);
    let (old_keys, new_keys) =
        line_diff::line_keys(old, &old_lines, new, &new_lines, options.ignore_whitespace);

    let line_ops = line_diff::diff_keys(&old_keys, &new_keys);
    let ops: Vec<Op> = line_diff::pair_replacements(line_ops, old, &old_lines, new, &new_lines)
        .into_iter()
        .map(|op| to_op(op, old, &old_lines, new, &new_lines, options))
        .collect();

    let stats = stats(&ops, &old_lines, &new_lines);
    Diff {
        old_lines,
        new_lines,
        ops,
        stats,
    }
}

fn to_op(
    op: DiffOp,
    old: &str,
    old_lines: &[Line],
    new: &str,
    new_lines: &[Line],
    options: Options,
) -> Op {
    let (old_start, old_len, new_start, new_len) = {
        let (o, n) = (op.old_range(), op.new_range());
        (o.start, o.len(), n.start, n.len())
    };
    let kind = match op {
        DiffOp::Equal { .. } => OpKind::Equal,
        DiffOp::Delete { .. } => OpKind::Delete,
        DiffOp::Insert { .. } => OpKind::Insert,
        DiffOp::Replace { .. } => OpKind::Replace,
    };
    let words = if kind == OpKind::Replace {
        (0..old_len.min(new_len))
            .map(|i| {
                let a = old_lines[old_start + i].text(old);
                let b = new_lines[new_start + i].text(new);
                word_diff::word_changes(a, b, options.ignore_whitespace)
            })
            .collect()
    } else {
        Vec::new()
    };
    Op {
        kind,
        old_start,
        old_len,
        new_start,
        new_len,
        words,
    }
}

fn stats(ops: &[Op], old_lines: &[Line], new_lines: &[Line]) -> Stats {
    let mut equal = 0;
    let mut added = 0;
    let mut removed = 0;
    for op in ops {
        if op.kind == OpKind::Equal {
            equal += op.old_len;
        } else {
            removed += op.old_len;
            added += op.new_len;
        }
    }
    let total = old_lines.len() + new_lines.len();
    let similarity = if total == 0 {
        1.0
    } else {
        (2 * equal) as f64 / total as f64
    };

    let old_eol = eol_style(old_lines);
    let new_eol = eol_style(new_lines);
    let eol_differs = old_eol != EolStyle::None && new_eol != EolStyle::None && old_eol != new_eol;

    Stats {
        added,
        removed,
        similarity,
        old_eol,
        new_eol,
        eol_differs,
    }
}
