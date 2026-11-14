use diff_core::{Decoded, Encoding, Eol, EolStyle, Op, OpKind, Options, decode, diff, split_lines};

const PLAIN: Options = Options {
    ignore_whitespace: false,
};
const NO_SPACES: Options = Options {
    ignore_whitespace: true,
};

fn kinds(ops: &[Op]) -> Vec<(OpKind, usize, usize)> {
    ops.iter()
        .map(|op| (op.kind, op.old_len, op.new_len))
        .collect()
}

#[test]
fn both_empty() {
    let d = diff("", "", PLAIN);
    assert!(d.ops.is_empty());
    assert_eq!((d.stats.added, d.stats.removed), (0, 0));
    assert_eq!(d.stats.similarity, 1.0);
    assert!(!d.stats.eol_differs);
}

#[test]
fn empty_against_text() {
    let d = diff("", "a\nb\n", PLAIN);
    assert_eq!(kinds(&d.ops), vec![(OpKind::Insert, 0, 2)]);
    assert_eq!((d.stats.added, d.stats.removed), (2, 0));
    assert_eq!(d.stats.similarity, 0.0);

    let d = diff("a\nb\n", "", PLAIN);
    assert_eq!(kinds(&d.ops), vec![(OpKind::Delete, 2, 0)]);
    assert_eq!((d.stats.added, d.stats.removed), (0, 2));
}

#[test]
fn identical() {
    let text = "alpha\nbeta\ngamma\n";
    let d = diff(text, text, PLAIN);
    assert_eq!(kinds(&d.ops), vec![(OpKind::Equal, 3, 3)]);
    assert_eq!((d.stats.added, d.stats.removed), (0, 0));
    assert_eq!(d.stats.similarity, 1.0);
}

#[test]
fn modified_line_is_paired_with_word_highlights() {
    let d = diff(
        "one\nthe quick brown fox\nthree\n",
        "one\nthe quick red fox\nthree\n",
        PLAIN,
    );
    assert_eq!(
        kinds(&d.ops),
        vec![
            (OpKind::Equal, 1, 1),
            (OpKind::Replace, 1, 1),
            (OpKind::Equal, 1, 1)
        ]
    );
    let words = &d.ops[1].words;
    assert_eq!(words.len(), 1);
    assert_eq!(words[0].old, vec![[10, 15]]);
    assert_eq!(words[0].new, vec![[10, 13]]);
    assert_eq!((d.stats.added, d.stats.removed), (1, 1));
    assert!((d.stats.similarity - 4.0 / 6.0).abs() < 1e-9);
}

#[test]
fn insertions_and_deletions() {
    let d = diff("a\nb\nc\nd\n", "a\nc\nd\ne\n", PLAIN);
    assert_eq!(
        kinds(&d.ops),
        vec![
            (OpKind::Equal, 1, 1),
            (OpKind::Delete, 1, 0),
            (OpKind::Equal, 2, 2),
            (OpKind::Insert, 0, 1),
        ]
    );
    assert_eq!((d.stats.added, d.stats.removed), (1, 1));
}

#[test]
fn modified_lines_pair_with_the_most_similar_line() {
    let old = "x\nreturn sum;\ny\n";
    let new = "x\nsum = Math.max(0, sum - discount);\nreturn sum * 1.22;\ny\n";
    let d = diff(old, new, PLAIN);
    assert_eq!(
        kinds(&d.ops),
        vec![
            (OpKind::Equal, 1, 1),
            (OpKind::Insert, 0, 1),
            (OpKind::Replace, 1, 1),
            (OpKind::Equal, 1, 1),
        ]
    );
    assert_eq!((d.ops[2].old_start, d.ops[2].new_start), (1, 2));
    assert_eq!(d.ops[2].words[0].new, vec![[10, 17]]);
}

#[test]
fn dissimilar_lines_pair_in_order() {
    let d = diff("x\naaa\nbbb\ny\n", "x\nccc\ny\n", PLAIN);
    assert_eq!(
        kinds(&d.ops),
        vec![
            (OpKind::Equal, 1, 1),
            (OpKind::Replace, 1, 1),
            (OpKind::Delete, 1, 0),
            (OpKind::Equal, 1, 1),
        ]
    );
}

#[test]
fn word_spans_count_utf16_units() {
    // The emoji takes two UTF-16 code units, the accented letter one.
    let d = diff("😀 caffè latte\n", "😀 caffè macchiato\n", PLAIN);
    let words = &d.ops[0].words[0];
    assert_eq!(words.old, vec![[9, 14]]);
    assert_eq!(words.new, vec![[9, 18]]);
}

#[test]
fn line_endings_differ() {
    let d = diff("a\r\nb\r\n", "a\nb\n", PLAIN);
    assert_eq!(kinds(&d.ops), vec![(OpKind::Equal, 2, 2)]);
    assert_eq!(d.stats.old_eol, EolStyle::Crlf);
    assert_eq!(d.stats.new_eol, EolStyle::Lf);
    assert!(d.stats.eol_differs);
    assert_eq!(d.stats.similarity, 1.0);
}

#[test]
fn line_endings_same_or_absent() {
    assert!(!diff("a\r\nb\r\n", "a\r\nc\r\n", PLAIN).stats.eol_differs);
    // A single line without a break has no convention to compare.
    assert!(!diff("a", "a\r\n", PLAIN).stats.eol_differs);
    assert_eq!(diff("a\nb\r\n", "", PLAIN).stats.old_eol, EolStyle::Mixed);
}

#[test]
fn split_keeps_terminators() {
    let text = "a\r\nb\n\nc";
    let lines = split_lines(text);
    let eols: Vec<Eol> = lines.iter().map(|l| l.eol).collect();
    assert_eq!(eols, vec![Eol::Crlf, Eol::Lf, Eol::Lf, Eol::None]);
    let texts: Vec<&str> = lines.iter().map(|l| l.text(text)).collect();
    assert_eq!(texts, vec!["a", "b", "", "c"]);
    // A lone carriage return stays in the content.
    assert_eq!(split_lines("a\rb\n")[0].text("a\rb\n"), "a\rb");
}

#[test]
fn ignore_whitespace() {
    let old = "if (x) {\n  call();\n}\n";
    let new = "if (x)  {\n\tcall( );\n}\n";
    assert_eq!(
        kinds(&diff(old, new, NO_SPACES).ops),
        vec![(OpKind::Equal, 3, 3)]
    );
    assert_eq!(
        kinds(&diff(old, new, PLAIN).ops),
        vec![(OpKind::Replace, 2, 2), (OpKind::Equal, 1, 1)]
    );
}

#[test]
fn ignore_whitespace_skips_space_only_word_changes() {
    let d = diff("let  a = 1;\n", "let a = 2;\n", NO_SPACES);
    let words = &d.ops[0].words[0];
    assert_eq!(words.old, vec![[9, 10]]);
    assert_eq!(words.new, vec![[8, 9]]);
}

fn text(decoded: Decoded) -> (String, Encoding) {
    match decoded {
        Decoded::Text { text, encoding } => (text, encoding),
        Decoded::Binary => panic!("decoded as binary"),
    }
}

#[test]
fn decode_utf8() {
    assert_eq!(
        text(decode("caffè\n".as_bytes())),
        ("caffè\n".into(), Encoding::Utf8)
    );
    assert_eq!(
        text(decode(b"\xEF\xBB\xBFhi")),
        ("hi".into(), Encoding::Utf8Bom)
    );
    assert_eq!(text(decode(b"")), (String::new(), Encoding::Utf8));
}

#[test]
fn decode_utf16() {
    let le: Vec<u8> = "héllo\n"
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    let be: Vec<u8> = "héllo\n"
        .encode_utf16()
        .flat_map(u16::to_be_bytes)
        .collect();

    let with_bom = [&b"\xFF\xFE"[..], &le].concat();
    assert_eq!(
        text(decode(&with_bom)),
        ("héllo\n".into(), Encoding::Utf16Le)
    );
    let with_bom = [&b"\xFE\xFF"[..], &be].concat();
    assert_eq!(
        text(decode(&with_bom)),
        ("héllo\n".into(), Encoding::Utf16Be)
    );

    assert_eq!(text(decode(&le)), ("héllo\n".into(), Encoding::Utf16Le));
    assert_eq!(text(decode(&be)), ("héllo\n".into(), Encoding::Utf16Be));
}

#[test]
fn decode_windows_1252() {
    // 0xE8 is "è" and 0x80 is "€" in Windows-1252; neither is valid UTF-8 here.
    assert_eq!(
        text(decode(b"caff\xE8 \x80 5\n")),
        ("caffè € 5\n".into(), Encoding::Windows1252)
    );
}

#[test]
fn decode_binary() {
    assert_eq!(
        decode(b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\x01\0"),
        Decoded::Binary
    );
    assert_eq!(decode(&[0, 1, 2, 3, 0, 0, 7, 9, 0, 1]), Decoded::Binary);
}
