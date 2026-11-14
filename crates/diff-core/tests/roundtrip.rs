//! Random texts: the ops must cover both texts exactly, rebuild them byte for
//! byte, and only call lines equal when they really are.

use diff_core::{Diff, OpKind, Options, diff};

/// SplitMix64, so the test needs no extra dependencies and is reproducible.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    fn pick<'a>(&mut self, items: &[&'a str]) -> &'a str {
        items[self.below(items.len())]
    }
}

/// Few distinct lines, so that the diff finds plenty of matches.
const VOCABULARY: &[&str] = &[
    "",
    "a",
    "b",
    "fn main() {",
    "}",
    "  let x = 1;",
    "\tlet x = 1;",
    "let  x = 2;",
    "caffè latte",
    "😀 emoji",
    "a\rb",
    "the quick brown fox",
];

fn random_lines(rng: &mut Rng, max: usize) -> Vec<String> {
    (0..rng.below(max + 1))
        .map(|_| rng.pick(VOCABULARY).to_owned())
        .collect()
}

/// Edits a copy of `lines`: deletions, insertions and in-place changes.
fn mutate(rng: &mut Rng, lines: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for line in lines {
        match rng.below(10) {
            0 => {}
            1 => {
                out.push(rng.pick(VOCABULARY).to_owned());
                out.push(line.clone());
            }
            2 => out.push(format!("{line} changed")),
            _ => out.push(line.clone()),
        }
    }
    if rng.below(3) == 0 {
        out.push(rng.pick(VOCABULARY).to_owned());
    }
    out
}

fn join(rng: &mut Rng, lines: &[String]) -> String {
    let style = rng.below(3); // all LF, all CRLF, or mixed
    let mut text = String::new();
    for (i, line) in lines.iter().enumerate() {
        text.push_str(line);
        let last = i + 1 == lines.len();
        if last && rng.below(4) == 0 {
            break; // no line break at end of file
        }
        let crlf = match style {
            0 => false,
            1 => true,
            _ => rng.below(2) == 0,
        };
        text.push_str(if crlf { "\r\n" } else { "\n" });
    }
    text
}

fn content(text: &str, d: &Diff, old: bool, i: usize) -> String {
    let line = if old { d.old_lines[i] } else { d.new_lines[i] };
    line.text(text).to_owned()
}

fn without_spaces(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).collect()
}

fn check(old: &str, new: &str, options: Options) {
    let d = diff(old, new, options);
    let same = |a: &str, b: &str| {
        if options.ignore_whitespace {
            without_spaces(a) == without_spaces(b)
        } else {
            a == b
        }
    };

    let (mut old_pos, mut new_pos) = (0, 0);
    let (mut rebuilt_old, mut rebuilt_new) = (String::new(), String::new());
    // Applying the ops to the old lines must give the new lines, and back.
    let (mut patched, mut unpatched) = (Vec::new(), Vec::new());
    let mut equal = 0;

    for op in &d.ops {
        assert_eq!(
            (op.old_start, op.new_start),
            (old_pos, new_pos),
            "ops must be contiguous"
        );
        assert!(op.old_len + op.new_len > 0, "empty op");
        match op.kind {
            OpKind::Equal => {
                assert_eq!(op.old_len, op.new_len);
                equal += op.old_len;
            }
            OpKind::Delete => assert_eq!((op.old_len > 0, op.new_len), (true, 0)),
            OpKind::Insert => assert_eq!((op.old_len, op.new_len > 0), (0, true)),
            OpKind::Replace => {
                assert!(op.old_len > 0 && op.old_len == op.new_len);
                assert_eq!(op.words.len(), op.old_len);
            }
        }

        for line in &d.old_lines[op.old_start..op.old_start + op.old_len] {
            rebuilt_old.push_str(line.text(old));
            rebuilt_old.push_str(line.eol.as_str());
        }
        for line in &d.new_lines[op.new_start..op.new_start + op.new_len] {
            rebuilt_new.push_str(line.text(new));
            rebuilt_new.push_str(line.eol.as_str());
        }

        if op.kind == OpKind::Equal {
            for k in 0..op.old_len {
                let a = content(old, &d, true, op.old_start + k);
                let b = content(new, &d, false, op.new_start + k);
                assert!(
                    same(&a, &b),
                    "equal op pairs different lines: {a:?} / {b:?}"
                );
                patched.push(a);
                unpatched.push(b);
            }
        } else {
            patched.extend(
                (op.new_start..op.new_start + op.new_len).map(|i| content(new, &d, false, i)),
            );
            unpatched.extend(
                (op.old_start..op.old_start + op.old_len).map(|i| content(old, &d, true, i)),
            );
        }

        for (k, words) in op.words.iter().enumerate() {
            let a = content(old, &d, true, op.old_start + k);
            let b = content(new, &d, false, op.new_start + k);
            check_spans(&words.old, &a);
            check_spans(&words.new, &b);
        }

        old_pos += op.old_len;
        new_pos += op.new_len;
    }

    assert_eq!(
        (old_pos, new_pos),
        (d.old_lines.len(), d.new_lines.len()),
        "ops must cover everything"
    );
    assert_eq!(rebuilt_old, old);
    assert_eq!(rebuilt_new, new);

    let new_content: Vec<String> = (0..d.new_lines.len())
        .map(|i| content(new, &d, false, i))
        .collect();
    let old_content: Vec<String> = (0..d.old_lines.len())
        .map(|i| content(old, &d, true, i))
        .collect();
    if options.ignore_whitespace {
        assert_eq!(patched.len(), new_content.len());
        assert_eq!(unpatched.len(), old_content.len());
    } else {
        assert_eq!(patched, new_content);
        assert_eq!(unpatched, old_content);
    }

    assert_eq!(d.stats.removed + equal, d.old_lines.len());
    assert_eq!(d.stats.added + equal, d.new_lines.len());
    assert!((0.0..=1.0).contains(&d.stats.similarity));
}

fn check_spans(spans: &[[u32; 2]], line: &str) {
    let len = line.encode_utf16().count() as u32;
    let mut prev_end = 0;
    for &[start, end] in spans {
        assert!(
            start < end && end <= len,
            "span {start}..{end} outside line of {len}"
        );
        assert!(start >= prev_end, "spans overlap or are unsorted");
        prev_end = end;
    }
}

#[test]
fn random_edits_rebuild_both_texts() {
    let mut rng = Rng(0x00D1_FFD7_0F00);
    for _ in 0..3000 {
        let old_lines = random_lines(&mut rng, 40);
        let new_lines = if rng.below(5) == 0 {
            random_lines(&mut rng, 40)
        } else {
            mutate(&mut rng, &old_lines)
        };
        let old = join(&mut rng, &old_lines);
        let new = join(&mut rng, &new_lines);
        for ignore_whitespace in [false, true] {
            check(&old, &new, Options { ignore_whitespace });
        }
    }
}
