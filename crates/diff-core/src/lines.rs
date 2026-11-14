//! Splitting text into lines while remembering each line terminator, so the
//! original text can be rebuilt byte for byte.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Eol {
    /// Last line of a file that does not end with a line break.
    None,
    Lf,
    Crlf,
}

impl Eol {
    pub fn as_str(self) -> &'static str {
        match self {
            Eol::None => "",
            Eol::Lf => "\n",
            Eol::Crlf => "\r\n",
        }
    }
}

/// A line as byte offsets into its source text. `start..end` is the content
/// without the terminator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Line {
    pub start: usize,
    pub end: usize,
    pub eol: Eol,
}

impl Line {
    pub fn text<'a>(&self, source: &'a str) -> &'a str {
        &source[self.start..self.end]
    }
}

/// A lone `\r` is kept as part of the line content, as git does.
pub fn split_lines(text: &str) -> Vec<Line> {
    let bytes = text.as_bytes();
    let mut lines = Vec::new();
    let mut start = 0;
    for (i, &byte) in bytes.iter().enumerate() {
        if byte != b'\n' {
            continue;
        }
        let (end, eol) = if i > start && bytes[i - 1] == b'\r' {
            (i - 1, Eol::Crlf)
        } else {
            (i, Eol::Lf)
        };
        lines.push(Line { start, end, eol });
        start = i + 1;
    }
    if start < bytes.len() {
        lines.push(Line {
            start,
            end: bytes.len(),
            eol: Eol::None,
        });
    }
    lines
}

/// The line break convention of a whole file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum EolStyle {
    /// No line breaks at all.
    None,
    Lf,
    Crlf,
    Mixed,
}

pub fn eol_style(lines: &[Line]) -> EolStyle {
    let mut style = EolStyle::None;
    for line in lines {
        let this = match line.eol {
            Eol::None => continue,
            Eol::Lf => EolStyle::Lf,
            Eol::Crlf => EolStyle::Crlf,
        };
        if style == EolStyle::None {
            style = this;
        } else if style != this {
            return EolStyle::Mixed;
        }
    }
    style
}
