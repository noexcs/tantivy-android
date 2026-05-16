use tantivy::Index;
use tantivy::tokenizer::*;

/// A simple CJK bigram tokenizer. Consecutive CJK characters are split into
/// overlapping character pairs (bigrams). Non-CJK text is passed through
/// as-is.
#[derive(Clone)]
pub(crate) struct CJKBigramTokenizer;

impl Tokenizer for CJKBigramTokenizer {
    type TokenStream<'a> = BoxTokenStream<'a>;

    fn token_stream<'a>(&mut self, text: &'a str) -> BoxTokenStream<'a> {
        let mut tokens = Vec::new();
        let chars: Vec<char> = text.chars().collect();
        let len = chars.len();

        let mut i = 0;
        while i < len {
            if is_cjk(chars[i]) {
                let cjk_start = i;
                while i < len && is_cjk(chars[i]) {
                    i += 1;
                }
                let run_len = i - cjk_start;
                // Emit unigrams and bigrams at consecutive character positions.
                // Each bigram shares the position of its first character and has
                // position_length=2 to indicate it spans two character positions.
                for j in 0..run_len {
                    let ch = chars[cjk_start + j];
                    let char_offset = chars.iter().take(cjk_start + j).map(|c| c.len_utf8()).sum();
                    tokens.push(Token {
                        offset_from: char_offset,
                        offset_to: char_offset + ch.len_utf8(),
                        position: tokens.len(),
                        text: ch.to_string(),
                        position_length: 1,
                    });
                    if j + 1 < run_len {
                        let next = chars[cjk_start + j + 1];
                        let bigram = format!("{}{}", ch, next);
                        tokens.push(Token {
                            offset_from: char_offset,
                            offset_to: char_offset + ch.len_utf8() + next.len_utf8(),
                            position: tokens.len(),
                            text: bigram,
                            position_length: 2,
                        });
                    }
                }
            } else if chars[i].is_whitespace() {
                i += 1;
            } else {
                let start = i;
                while i < len && !is_cjk(chars[i]) && !chars[i].is_whitespace() {
                    i += 1;
                }
                let word: String = chars[start..i].iter().collect();
                let offset = chars.iter().take(start).map(|c| c.len_utf8()).sum();
                tokens.push(Token {
                    offset_from: offset,
                    offset_to: offset + word.len(),
                    position: tokens.len(),
                    text: word,
                    position_length: 1,
                });
            }
        }

        BoxTokenStream::new(CJKTokenStream {
            tokens,
            index: 0,
        })
    }
}

struct CJKTokenStream {
    tokens: Vec<Token>,
    index: usize,
}

impl TokenStream for CJKTokenStream {
    fn advance(&mut self) -> bool {
        if self.index < self.tokens.len() {
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn token(&self) -> &Token {
        &self.tokens[self.index - 1]
    }

    fn token_mut(&mut self) -> &mut Token {
        &mut self.tokens[self.index - 1]
    }
}

fn is_cjk(ch: char) -> bool {
    matches!(ch,
        '\u{4E00}'..='\u{9FFF}'   // CJK Unified Ideographs
        | '\u{3400}'..='\u{4DBF}'   // CJK Unified Ideographs Extension A
        | '\u{20000}'..='\u{2A6DF}' // CJK Unified Ideographs Extension B
        | '\u{F900}'..='\u{FAFF}'   // CJK Compatibility Ideographs
        | '\u{3040}'..='\u{309F}'   // Hiragana
        | '\u{30A0}'..='\u{30FF}'   // Katakana
        | '\u{AC00}'..='\u{D7AF}'   // Hangul Syllables
        | '\u{1100}'..='\u{11FF}'   // Hangul Jamo
    )
}

/// Registers the CJK bigram tokenizer under the given name.
pub fn register_cjk_tokenizer(index: &Index, name: &str) {
    let analyzer = TextAnalyzer::from(CJKBigramTokenizer);
    index.tokenizers().register(name, analyzer);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokenize(text: &str) -> Vec<String> {
        let mut tokenizer = CJKBigramTokenizer;
        let mut stream = tokenizer.token_stream(text);
        let mut tokens = Vec::new();
        while stream.advance() {
            tokens.push(stream.token().text.clone());
        }
        tokens
    }

    #[test]
    fn test_cjk_bigram() {
        let tokens = tokenize("北京天气");
        assert!(tokens.contains(&"北京".to_string()), "expected 北京 in {:?}", tokens);
        assert!(tokens.contains(&"天气".to_string()), "expected 天气 in {:?}", tokens);
    }

    #[test]
    fn test_mixed_text() {
        let tokens = tokenize("I love 北京 and 上海");
        assert!(tokens.contains(&"I".to_string()));
        assert!(tokens.contains(&"love".to_string()));
        assert!(tokens.contains(&"北京".to_string()));
        assert!(tokens.contains(&"上海".to_string()));
    }

    #[test]
    fn test_single_cjk_char() {
        let tokens = tokenize("北");
        assert!(tokens.contains(&"北".to_string()));
        assert_eq!(tokens.len(), 1);
    }

    #[test]
    fn test_empty() {
        let tokens = tokenize("");
        assert!(tokens.is_empty());
    }

    #[test]
    fn test_korean_bigram() {
        let tokens = tokenize("한국");
        assert!(tokens.contains(&"한국".to_string()), "expected 한국 in {:?}", tokens);
    }
}
