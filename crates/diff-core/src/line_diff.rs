//! Line level diff: lines are reduced to integer keys, the common prefix and
//! suffix are cut off, and only the middle goes through `similar`.

use std::borrow::Cow;
use std::collections::HashMap;
use std::time::{Duration, Instant};

use similar::{Algorithm, DiffOp, capture_diff_slices_deadline};

use crate::lines::Line;

/// Past this, `similar` returns a coarser (still correct) result.
const DIFF_BUDGET: Duration = Duration::from_secs(5);

/// Gives equal keys to lines that must compare as equal. Line terminators are
/// never part of the key, so CRLF and LF versions of a line match.
pub fn line_keys<'a>(
    old: &'a str,
    old_lines: &[Line],
    new: &'a str,
    new_lines: &[Line],
    ignore_whitespace: bool,
) -> (Vec<u32>, Vec<u32>) {
    let mut ids: HashMap<Cow<'a, str>, u32> = HashMap::new();
    let mut key_of = |text: &'a str, line: &Line| -> u32 {
        let content = line.text(text);
        let key: Cow<'a, str> = if ignore_whitespace {
            Cow::Owned(content.chars().filter(|c| !c.is_whitespace()).collect())
        } else {
            Cow::Borrowed(content)
        };
        let next = ids.len() as u32;
        *ids.entry(key).or_insert(next)
    };
    let old_keys = old_lines.iter().map(|l| key_of(old, l)).collect();
    let new_keys = new_lines.iter().map(|l| key_of(new, l)).collect();
    (old_keys, new_keys)
}

/// Diff ops covering both sequences completely, in order.
pub fn diff_keys(old: &[u32], new: &[u32]) -> Vec<DiffOp> {
    let prefix = old.iter().zip(new).take_while(|(a, b)| a == b).count();
    let room = old.len().min(new.len()) - prefix;
    let suffix = old[prefix..]
        .iter()
        .rev()
        .zip(new[prefix..].iter().rev())
        .take(room)
        .take_while(|(a, b)| a == b)
        .count();

    let old_mid = &old[prefix..old.len() - suffix];
    let new_mid = &new[prefix..new.len() - suffix];

    let mut ops = Vec::new();
    push(
        &mut ops,
        DiffOp::Equal {
            old_index: 0,
            new_index: 0,
            len: prefix,
        },
    );
    if !old_mid.is_empty() || !new_mid.is_empty() {
        let deadline = Some(Instant::now() + DIFF_BUDGET);
        for op in capture_diff_slices_deadline(Algorithm::Myers, old_mid, new_mid, deadline) {
            push(&mut ops, shift(op, prefix));
        }
    }
    push(
        &mut ops,
        DiffOp::Equal {
            old_index: old.len() - suffix,
            new_index: new.len() - suffix,
            len: suffix,
        },
    );
    ops
}

fn shift(op: DiffOp, by: usize) -> DiffOp {
    match op {
        DiffOp::Equal {
            old_index,
            new_index,
            len,
        } => DiffOp::Equal {
            old_index: old_index + by,
            new_index: new_index + by,
            len,
        },
        DiffOp::Delete {
            old_index,
            old_len,
            new_index,
        } => DiffOp::Delete {
            old_index: old_index + by,
            old_len,
            new_index: new_index + by,
        },
        DiffOp::Insert {
            old_index,
            new_index,
            new_len,
        } => DiffOp::Insert {
            old_index: old_index + by,
            new_index: new_index + by,
            new_len,
        },
        DiffOp::Replace {
            old_index,
            old_len,
            new_index,
            new_len,
        } => DiffOp::Replace {
            old_index: old_index + by,
            old_len,
            new_index: new_index + by,
            new_len,
        },
    }
}

/// Skips empty ops and joins neighbouring equal runs.
fn push(ops: &mut Vec<DiffOp>, op: DiffOp) {
    if op.old_range().is_empty() && op.new_range().is_empty() {
        return;
    }
    if let (Some(DiffOp::Equal { len: prev, .. }), DiffOp::Equal { len, .. }) = (ops.last_mut(), op)
    {
        *prev += len;
        return;
    }
    ops.push(op);
}

/// Above this many candidate pairs a replaced block is simply paired in order.
const PAIRING_LIMIT: usize = 10_000;
/// Two lines at least this similar (0 to 1) are paired even if out of order.
const ANCHOR_SIMILARITY: f64 = 0.5;

/// Splits replaced blocks so that each modified line is paired with the most
/// similar line on the other side. Similar pairs are found first; the lines
/// between them are paired in order, and what is left over becomes a plain
/// deletion or insertion.
pub fn pair_replacements(
    ops: Vec<DiffOp>,
    old: &str,
    old_lines: &[Line],
    new: &str,
    new_lines: &[Line],
) -> Vec<DiffOp> {
    let mut out = Vec::with_capacity(ops.len());
    for op in ops {
        let DiffOp::Replace {
            old_index,
            old_len,
            new_index,
            new_len,
        } = op
        else {
            out.push(op);
            continue;
        };
        if old_len == 1 && new_len == 1 {
            out.push(op);
            continue;
        }
        let similar_pairs = if old_len * new_len > PAIRING_LIMIT {
            Vec::new()
        } else {
            let words = |text, lines: &[Line], start, len| -> Vec<Vec<&str>> {
                lines[start..start + len]
                    .iter()
                    .map(|l| crate::word_diff::sorted_words(l.text(text)))
                    .collect()
            };
            anchors(
                &words(old, old_lines, old_index, old_len),
                &words(new, new_lines, new_index, new_len),
            )
        };

        let mut pos = (0, 0);
        for anchor in similar_pairs.into_iter().chain([(old_len, new_len)]) {
            let gap_old = anchor.0 - pos.0;
            let gap_new = anchor.1 - pos.1;
            let paired = gap_old.min(gap_new);
            let at = |o: usize, n: usize| (old_index + o, new_index + n);

            let (o, n) = at(pos.0, pos.1);
            push_replace(&mut out, o, n, paired);
            let (o, n) = at(pos.0 + paired, pos.1 + paired);
            if gap_old > paired {
                out.push(DiffOp::Delete {
                    old_index: o,
                    old_len: gap_old - paired,
                    new_index: n,
                });
            }
            if gap_new > paired {
                out.push(DiffOp::Insert {
                    old_index: o,
                    new_index: n,
                    new_len: gap_new - paired,
                });
            }
            if anchor != (old_len, new_len) {
                let (o, n) = at(anchor.0, anchor.1);
                push_replace(&mut out, o, n, 1);
            }
            pos = (anchor.0 + 1, anchor.1 + 1);
        }
    }
    out
}

/// Appends `len` paired lines, extending the previous replace when adjacent.
fn push_replace(ops: &mut Vec<DiffOp>, old_index: usize, new_index: usize, len: usize) {
    if len == 0 {
        return;
    }
    if let Some(DiffOp::Replace {
        old_index: o,
        old_len,
        new_index: n,
        new_len,
    }) = ops.last_mut()
        && *o + *old_len == old_index
        && *n + *new_len == new_index
    {
        *old_len += len;
        *new_len += len;
        return;
    }
    ops.push(DiffOp::Replace {
        old_index,
        old_len: len,
        new_index,
        new_len: len,
    });
}

/// In-order pairs of similar lines with the highest total similarity.
fn anchors(old: &[Vec<&str>], new: &[Vec<&str>]) -> Vec<(usize, usize)> {
    let (n, m) = (old.len(), new.len());
    let width = m + 1;
    let mut score = vec![0.0f64; (n + 1) * width];
    let similarity: Vec<f64> = old
        .iter()
        .flat_map(|a| new.iter().map(move |b| dice(a, b)))
        .collect();
    let sim = |i: usize, j: usize| similarity[i * m + j];

    for i in 1..=n {
        for j in 1..=m {
            let mut best = score[(i - 1) * width + j].max(score[i * width + j - 1]);
            let s = sim(i - 1, j - 1);
            if s >= ANCHOR_SIMILARITY {
                best = best.max(score[(i - 1) * width + j - 1] + s);
            }
            score[i * width + j] = best;
        }
    }

    let mut pairs = Vec::new();
    let (mut i, mut j) = (n, m);
    while i > 0 && j > 0 {
        let here = score[i * width + j];
        if here == score[(i - 1) * width + j] {
            i -= 1;
        } else if here == score[i * width + j - 1] {
            j -= 1;
        } else {
            pairs.push((i - 1, j - 1));
            i -= 1;
            j -= 1;
        }
    }
    pairs.reverse();
    pairs
}

/// Dice coefficient of two sorted word lists.
fn dice(a: &[&str], b: &[&str]) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    let (mut i, mut j, mut common) = (0, 0, 0);
    while i < a.len() && j < b.len() {
        match a[i].cmp(b[j]) {
            std::cmp::Ordering::Less => i += 1,
            std::cmp::Ordering::Greater => j += 1,
            std::cmp::Ordering::Equal => {
                common += 1;
                i += 1;
                j += 1;
            }
        }
    }
    (2 * common) as f64 / (a.len() + b.len()) as f64
}
