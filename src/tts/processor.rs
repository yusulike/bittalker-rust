//! Text preprocessing and tokenization for Supertonic TTS.
//! Port of the `UnicodeProcessor` class and `chunk_text` in
//! `src/services/helper.py`.

use anyhow::{anyhow, bail, Result};
use ndarray::{Array2, Array3};
use unicode_normalization::UnicodeNormalization;

pub const AVAILABLE_LANGS: &[&str] = &[
    "en", "ko", "ja", "ar", "bg", "cs", "da", "de", "el", "es", "et", "fi", "fr", "hi", "hr",
    "hu", "id", "it", "lt", "lv", "nl", "pl", "pt", "ro", "ru", "sk", "sl", "sv", "tr", "uk",
    "vi", "na",
];

pub struct UnicodeProcessor {
    /// Flat table: unicode codepoint (0..=0xFFFF) -> token id (-1 = unmapped)
    indexer: Vec<i64>,
}

impl UnicodeProcessor {
    pub fn load(path: &std::path::Path) -> Result<Self> {
        let reader = std::fs::File::open(path)?;
        let indexer = serde_json::from_reader(reader)?;
        Ok(Self { indexer })
    }

    /// Preprocess one text for a target language (NFKD, emoji strip, symbol
    /// normalization, terminal punctuation, language tags).
    pub fn preprocess_text(&self, text: &str, lang: &str) -> Result<String> {
        let mut text: String = text.nfkd().collect();

        // Remove emojis (wide Unicode range)
        text = text.chars().filter(|c| !is_emoji(*c)).collect();

        // Replace various dashes and symbols
        let replacements: &[(char, &str)] = &[
            ('–', "-"),
            ('‑', "-"),
            ('—', "-"),
            ('_', " "),
            ('\u{201c}', "\""), // left double quote "
            ('\u{201d}', "\""), // right double quote "
            ('\u{2018}', "'"),  // left single quote '
            ('\u{2019}', "'"),  // right single quote '
            ('´', "'"),
            ('`', "'"),
            ('[', " "),
            (']', " "),
            ('|', " "),
            ('/', " "),
            ('#', " "),
            ('→', " "),
            ('←', " "),
        ];
        for (from, to) in replacements {
            if text.contains(*from) {
                text = text.replace(*from, to);
            }
        }

        // Remove special symbols
        text = text
            .chars()
            .filter(|c| !matches!(c, '♥' | '☆' | '♡' | '©' | '\\'))
            .collect();

        // Replace known expressions
        for (from, to) in [("@", " at "), ("e.g.,", "for example, "), ("i.e.,", "that is, ")] {
            text = text.replace(from, to);
        }

        // Fix spacing around punctuation
        for (from, to) in [
            (" ,", ","), (" .", "."), (" !", "!"), (" ?", "?"), (" ;", ";"), (" :", ":"),
            (" '", "'"),
        ] {
            text = text.replace(from, to);
        }

        // Remove duplicate quotes
        while text.contains("\"\"") {
            text = text.replace("\"\"", "\"");
        }
        while text.contains("''") {
            text = text.replace("''", "'");
        }
        while text.contains("``") {
            text = text.replace("``", "`");
        }

        // Remove extra spaces
        text = collapse_whitespace(&text);

        // If text doesn't end with punctuation, quotes, or closing brackets,
        // add a period
        if !text
            .chars()
            .last()
            .is_some_and(is_terminal_punctuation)
        {
            text.push('.');
        }

        if !AVAILABLE_LANGS.contains(&lang) {
            bail!("Invalid language: {lang}");
        }
        Ok(format!("<{lang}>{text}</{lang}>"))
    }

    /// Tokenize preprocessed text: returns `(text_ids [1, L], text_mask [1, 1, L])`.
    pub fn encode(&self, text: &str, lang: &str) -> Result<(Array2<i64>, Array3<f32>)> {
        let processed = self.preprocess_text(text, lang)?;
        let unicode_values: Vec<u32> = processed.chars().map(|c| c as u32).collect();
        if let Some(&cp) = unicode_values.iter().find(|&&cp| cp >= 0x10000) {
            bail!("character U+{cp:04X} out of BMP range; input not representable");
        }
        let ids: Vec<i64> = unicode_values
            .iter()
            .map(|&cp| {
                let idx = cp as usize;
                if idx < self.indexer.len() {
                    self.indexer[idx]
                } else {
                    -1
                }
            })
            .collect();

        let len = ids.len();
        let text_ids = Array2::from_shape_vec((1, len), ids)
            .map_err(|e| anyhow!("shape error: {e}"))?;
        let text_mask = length_to_mask_2d(&[len], len);
        Ok((text_ids, text_mask))
    }
}

fn is_terminal_punctuation(c: char) -> bool {
    matches!(
        c,
        '.' | '!' | '?'
            | ';'
            | ':'
            | ','
            | '\''
            | '"'
            | ')'
            | ']'
            | '}'
            | '…'
            | '。'
            | '」'
            | '』'
            | '】'
            | '〉'
            | '》'
            | '›'
            | '»'
    )
}

fn is_emoji(c: char) -> bool {
    matches!(c as u32,
        0x1F600..=0x1F64F  // emoticons
        | 0x1F300..=0x1F5FF  // symbols & pictographs
        | 0x1F680..=0x1F6FF  // transport & map symbols
        | 0x1F700..=0x1F77F
        | 0x1F780..=0x1F7FF
        | 0x1F800..=0x1F8FF
        | 0x1F900..=0x1F9FF
        | 0x1FA00..=0x1FA6F
        | 0x1FA70..=0x1FAFF
        | 0x2600..=0x26FF
        | 0x2700..=0x27BF
        | 0x1F1E6..=0x1F1FF
    )
}

/// `\s+` -> " " and trim.
fn collapse_whitespace(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_ws = false;
    for c in text.chars() {
        if c.is_whitespace() {
            in_ws = true;
        } else {
            if in_ws && !out.is_empty() {
                out.push(' ');
            }
            in_ws = false;
            out.push(c);
        }
    }
    out
}

/// Port of `length_to_mask`: lengths `[B]` -> binary mask `[B, 1, max_len]`.
pub fn length_to_mask_2d(lengths: &[usize], max_len: usize) -> Array3<f32> {
    let b = lengths.len();
    let mut data = vec![0f32; b * max_len];
    for (i, &len) in lengths.iter().enumerate() {
        for j in 0..max_len.min(len) {
            data[i * max_len + j] = 1.0;
        }
    }
    Array3::from_shape_vec((b, 1, max_len), data).expect("valid mask shape")
}

/// Split text into chunks by paragraphs and sentences (max length in chars).
/// Port of `chunk_text`; abbreviations that must not split a sentence mirror
/// the original regex lookbehinds.
const ABBREVIATIONS: &[&str] = &[
    "Mr.", "Mrs.", "Ms.", "Dr.", "Prof.", "Sr.", "Jr.", "Ph.D.", "etc.", "e.g.", "i.e.", "vs.",
    "Inc.", "Ltd.", "Co.", "Corp.", "St.", "Ave.", "Blvd.",
];

pub fn chunk_text(text: &str, max_len: usize) -> Vec<String> {
    let mut chunks: Vec<String> = Vec::new();

    for paragraph in split_paragraphs(text) {
        let mut current_chunk = String::new();

        for sentence in split_sentences(&paragraph) {
            let add = if current_chunk.is_empty() { 0 } else { 1 }; // joining space
            if current_chunk.chars().count() + sentence.chars().count() + add <= max_len {
                if !current_chunk.is_empty() {
                    current_chunk.push(' ');
                }
                current_chunk.push_str(&sentence);
            } else {
                if !current_chunk.is_empty() {
                    chunks.push(current_chunk.trim().to_string());
                }
                current_chunk = sentence;
            }
        }
        if !current_chunk.is_empty() {
            chunks.push(current_chunk.trim().to_string());
        }
    }
    chunks
}

/// Split by paragraph boundaries: two or more newlines, possibly with other
/// whitespace between them (`\n\s*\n+`).
fn split_paragraphs(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut pending_newlines = 0usize;

    for c in text.trim().chars() {
        if c == '\n' {
            pending_newlines += 1;
        } else if c.is_whitespace() && pending_newlines > 0 {
            // whitespace inside a potential paragraph separator: keep pending
        } else if pending_newlines >= 2 {
            out.push(cur.trim().to_string());
            cur.clear();
            pending_newlines = 0;
            cur.push(c);
        } else if pending_newlines == 1 {
            cur.push('\n');
            pending_newlines = 0;
            cur.push(c);
        } else {
            cur.push(c);
        }
    }
    out.push(cur.trim().to_string());
    out.into_iter().filter(|p| !p.is_empty()).collect()
}

/// Split a paragraph into sentences at `.`/`!`/`?` followed by whitespace,
/// skipping boundaries right after common abbreviations or a single capital
/// letter (e.g. "F."). Equivalent to the original regex:
/// `(?<!Mr\.)...(?<!\b[A-Z]\.)(?<=[.!?])\s+`
fn split_sentences(paragraph: &str) -> Vec<String> {
    let chars: Vec<char> = paragraph.chars().collect();
    let mut sentences: Vec<String> = Vec::new();
    let mut current: Vec<char> = Vec::new();

    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        current.push(c);
        if matches!(c, '.' | '!' | '?') && i + 1 < chars.len() && chars[i + 1].is_whitespace() {
            let word_before = word_ending_before(&chars, i);
            let is_boundary = !ABBREVIATIONS.iter().any(|abbr| word_before == *abbr)
                && !(c == '.' && is_single_capital(&word_before));
            if is_boundary {
                sentences.push(current.iter().collect::<String>().trim().to_string());
                current.clear();
            }
        }
        i += 1;
    }
    if !current.is_empty() {
        sentences.push(current.iter().collect::<String>().trim().to_string());
    }
    sentences
}

/// The contiguous non-whitespace run ending at index `dot` (inclusive of it),
/// e.g. in "Dr. Smith" at the '.' -> "Dr."
fn word_ending_before(chars: &[char], dot: usize) -> String {
    let start = chars[..=dot]
        .iter()
        .rposition(|c| c.is_whitespace())
        .map(|p| p + 1)
        .unwrap_or(0);
    chars[start..=dot].iter().collect()
}

fn is_single_capital(word: &str) -> bool {
    let mut it = word.chars();
    match (it.next(), it.next()) {
        (Some(c), None) => c.is_ascii_uppercase(),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pp(text: &str, lang: &str) -> String {
        let mut p = UnicodeProcessor { indexer: vec![0; 65536] };
        p.indexer[0] = -1;
        p.preprocess_text(text, lang).unwrap()
    }

    #[test]
    fn wraps_with_lang_tags_and_period() {
        assert_eq!(pp("Hello world", "en"), "<en>Hello world.</en>");
        assert_eq!(
            pp("안녕하세요", "ko"),
            format!("<ko>{}</ko>", "안녕하세요.".nfkd().collect::<String>())
        );
    }

    #[test]
    fn keeps_existing_terminal_punctuation() {
        assert_eq!(pp("Hello!", "en"), "<en>Hello!</en>");
        assert_eq!(
            pp("구만 오천오백달러를 돌파했습니다.", "ko"),
            format!("<ko>{}</ko>", "구만 오천오백달러를 돌파했습니다.".nfkd().collect::<String>())
        );
    }

    #[test]
    fn replaces_symbols() {
        assert_eq!(pp("a_b", "en"), "<en>a b.</en>");
        assert_eq!(pp("a [b] #c", "en"), "<en>a b c.</en>");
        assert_eq!(pp("user@host", "en"), "<en>user at host.</en>");
    }

    #[test]
    fn strips_emoji() {
        assert_eq!(pp("hi 🔢 there", "en"), "<en>hi there.</en>");
    }

    #[test]
    fn collapses_whitespace_and_space_punct() {
        assert_eq!(pp("a  b   c , d", "en"), "<en>a b c, d.</en>");
    }

    #[test]
    fn rejects_unknown_lang() {
        let p = UnicodeProcessor { indexer: vec![0; 65536] };
        assert!(p.preprocess_text("hi", "xx").is_err());
    }

    #[test]
    fn chunks_single_sentence() {
        let chunks = chunk_text("구만 오천오백달러를 돌파했습니다.", 120);
        assert_eq!(chunks, vec!["구만 오천오백달러를 돌파했습니다."]);
    }

    #[test]
    fn chunks_by_sentence() {
        let text = "One sentence here. Another sentence follows! A third one?";
        let chunks = chunk_text(text, 25);
        assert_eq!(
            chunks,
            vec![
                "One sentence here.",
                "Another sentence follows!",
                "A third one?"
            ]
        );
    }

    #[test]
    fn respects_abbreviations() {
        let text = "Mr. Smith went home. He slept.";
        let chunks = chunk_text(text, 100);
        assert_eq!(chunks, vec!["Mr. Smith went home. He slept."]);
    }

    #[test]
    fn respects_single_capital_initial() {
        let text = "F. Scott Fitzgerald wrote. He did.";
        let chunks = chunk_text(text, 100);
        assert_eq!(chunks, vec!["F. Scott Fitzgerald wrote. He did."]);
    }

    #[test]
    fn splits_paragraphs() {
        let text = "First para.\n\nSecond para.";
        let chunks = chunk_text(text, 100);
        assert_eq!(chunks, vec!["First para.", "Second para."]);
    }

    #[test]
    fn mask_shape() {
        let m = length_to_mask_2d(&[5], 8);
        assert_eq!(m.shape(), &[1, 1, 8]);
        assert_eq!(m.sum(), 5.0);
    }
}
