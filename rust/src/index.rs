use tantivy::collector::TopDocs;
use tantivy::doc;
use tantivy::query::{BooleanQuery, FuzzyTermQuery, Occur, PhraseQuery, QueryParser};
use tantivy::schema::*;
use tantivy::Index;
use tantivy::IndexWriter;
use tantivy::ReloadPolicy;
use tantivy::Term;
use tantivy::TantivyError;
use tantivy::TantivyDocument;
use tantivy::tokenizer::{TokenStream, Tokenizer};
use std::collections::HashMap;
use std::sync::Mutex;
use crate::tokenizer;

pub struct DocInput {
    pub id: String,
    pub header_key: String,
    pub text: String,
}

pub struct IndexManager {
    index: Index,
    writer: Mutex<Option<IndexWriter>>,
    field_id: Field,
    field_header_key: Field,
    field_text: Field,
}

pub type TantivyResult<T> = Result<T, TantivyError>;

fn build_schema() -> (Schema, Field, Field, Field) {
    let mut schema_builder = Schema::builder();
    let field_id = schema_builder.add_text_field("id", STRING | STORED);
    let cjk_text_options = TextOptions::default()
        .set_stored()
        .set_indexing_options(
            TextFieldIndexing::default()
                .set_tokenizer("cjk_bigram")
                .set_index_option(IndexRecordOption::WithFreqsAndPositions),
        );
    let field_header_key = schema_builder.add_text_field("header_key", STRING | STORED);
    let field_text = schema_builder.add_text_field("text", cjk_text_options);
    let schema = schema_builder.build();
    (schema, field_id, field_header_key, field_text)
}

impl IndexManager {
    /// Creates an in-memory index (no disk I/O, data lost on close).
    pub fn new() -> Self {
        let (schema, field_id, field_header_key, field_text) = build_schema();
        let index = Index::create_in_ram(schema);
        tokenizer::register_cjk_tokenizer(&index, "cjk_bigram");
        let writer = index.writer(15_000_000).expect("Failed to create index writer");

        IndexManager {
            index,
            writer: Mutex::new(Some(writer)),
            field_id,
            field_header_key,
            field_text,
        }
    }

    /// Opens an existing index at `path`, or creates one if it doesn't exist.
    /// Data persists across sessions (backed by MmapDirectory).
    pub fn open_or_create(path: &str) -> TantivyResult<Self> {
        let (schema, field_id, field_header_key, field_text) = build_schema();
        let dir = tantivy::directory::MmapDirectory::open(path)?;
        let index = if Index::exists(&dir)? {
            Index::open(dir)?
        } else {
            Index::create(dir, schema, tantivy::IndexSettings::default())?
        };
        tokenizer::register_cjk_tokenizer(&index, "cjk_bigram");
        let writer = index.writer(50_000_000).expect("Failed to create index writer");

        Ok(IndexManager {
            index,
            writer: Mutex::new(Some(writer)),
            field_id,
            field_header_key,
            field_text,
        })
    }

    pub fn rebuild(&mut self, docs: &[DocInput]) -> TantivyResult<()> {
        let mut writer_guard = self.writer.lock().unwrap();
        let writer = writer_guard.as_mut().expect("writer must exist");
        writer.delete_all_documents()?;

        for d in docs {
            let doc = doc!(
                self.field_id => d.id.clone(),
                self.field_header_key => d.header_key.clone(),
                self.field_text => d.text.clone(),
            );
            writer.add_document(doc)?;
        }
        writer.commit()?;
        Ok(())
    }

    pub fn add_or_update(&mut self, id: &str, header_key: &str, text: &str) -> TantivyResult<()> {
        let mut writer_guard = self.writer.lock().unwrap();
        let writer = writer_guard.as_mut().expect("writer must exist");

        let term = Term::from_field_text(self.field_id, id);
        writer.delete_term(term);

        let doc = doc!(
            self.field_id => id,
            self.field_header_key => header_key,
            self.field_text => text,
        );
        writer.add_document(doc)?;
        writer.commit()?;
        Ok(())
    }

    pub fn remove_by_header(&mut self, header_key: &str) -> TantivyResult<()> {
        let mut writer_guard = self.writer.lock().unwrap();
        let writer = writer_guard.as_mut().expect("writer must exist");
        let term = Term::from_field_text(self.field_header_key, header_key);
        writer.delete_term(term);
        writer.commit()?;
        Ok(())
    }

    pub fn search(&self, query_str: &str, top_k: usize) -> TantivyResult<Vec<(String, f32)>> {
        let searcher = self.searcher()?;
        let query_parser = QueryParser::for_index(&self.index, vec![self.field_text]);
        let query = query_parser.parse_query(query_str)?;
        let top_docs = searcher.search(&query, &TopDocs::with_limit(top_k))?;
        collect_results(&searcher, top_docs, self.field_id)
    }

    /// Phrase search: matches documents containing the exact phrase.
    pub fn search_phrase(&self, phrase: &str, top_k: usize) -> TantivyResult<Vec<(String, f32)>> {
        let searcher = self.searcher()?;

        let query_terms = self.get_query_terms(phrase);
        let mut phrase_terms: Vec<(usize, Term)> = query_terms
            .iter()
            .map(|(pos, text)| (*pos, Term::from_field_text(self.field_text, text)))
            .collect();

        if phrase_terms.is_empty() {
            return Ok(Vec::new());
        }

        // Build PhraseQuery with (offset, term) pairs
        phrase_terms.sort_by_key(|(pos, _)| *pos);
        let terms_with_offsets: Vec<(usize, Term)> = phrase_terms.into_iter().collect();

        let phrase_query = PhraseQuery::new_with_offset(terms_with_offsets);
        let top_docs = searcher.search(&phrase_query, &TopDocs::with_limit(top_k))?;
        collect_results(&searcher, top_docs, self.field_id)
    }

    /// Fuzzy search: matches documents tolerating up to `distance` edits per term.
    pub fn search_fuzzy(
        &self,
        query_str: &str,
        distance: u8,
        top_k: usize,
    ) -> TantivyResult<Vec<(String, f32)>> {
        let searcher = self.searcher()?;

        let query_terms = self.get_query_terms(query_str);
        let mut subqueries: Vec<(Occur, Box<dyn tantivy::query::Query>)> = Vec::new();
        for (_pos, text) in &query_terms {
            let term = Term::from_field_text(self.field_text, text);
            subqueries.push((
                Occur::Should,
                Box::new(FuzzyTermQuery::new(term, distance, true)),
            ));
        }

        if subqueries.is_empty() {
            return Ok(Vec::new());
        }

        let query = BooleanQuery::new(subqueries);
        let top_docs = searcher.search(&query, &TopDocs::with_limit(top_k))?;
        collect_results(&searcher, top_docs, self.field_id)
    }

    /// Search with facet counts grouped by headerKey.
    /// Returns (top_results, facet_counts) where facet_counts maps headerKey → count.
    pub fn search_with_facets(
        &self,
        query_str: &str,
        top_k: usize,
    ) -> TantivyResult<(Vec<(String, f32)>, HashMap<String, u64>)> {
        let searcher = self.searcher()?;

        let query_parser = QueryParser::for_index(&self.index, vec![self.field_text]);
        let query = query_parser.parse_query(query_str)?;

        // Get top results
        let top_docs = searcher.search(&query, &TopDocs::with_limit(top_k))?;
        let results = collect_results(&searcher, top_docs, self.field_id)?;

        // Count by headerKey over a larger set for facets
        let all_docs = searcher.search(&query, &TopDocs::with_limit(1000))?;
        let mut facet_counts: HashMap<String, u64> = HashMap::new();
        for (_score, doc_addr) in all_docs {
            if let Ok(doc) = searcher.doc::<TantivyDocument>(doc_addr) {
                if let Some(hk) = doc.get_first(self.field_header_key) {
                    if let Some(hk_str) = hk.as_str() {
                        *facet_counts.entry(hk_str.to_string()).or_default() += 1;
                    }
                }
            }
        }

        Ok((results, facet_counts))
    }

    fn searcher(&self) -> TantivyResult<tantivy::Searcher> {
        let reader = self
            .index
            .reader_builder()
            .reload_policy(ReloadPolicy::Manual)
            .try_into()?;
        reader.reload()?;
        Ok(reader.searcher())
    }

    fn get_query_terms(&self, text: &str) -> Vec<(usize, String)> {
        let mut tokenizer = crate::tokenizer::CJKBigramTokenizer;
        let mut stream = tokenizer.token_stream(text);
        let mut terms = Vec::new();
        while stream.advance() {
            let token = stream.token();
            terms.push((token.position, token.text.clone()));
        }
        terms
    }
}

fn collect_results(
    searcher: &tantivy::Searcher,
    top_docs: Vec<(f32, tantivy::DocAddress)>,
    field_id: Field,
) -> TantivyResult<Vec<(String, f32)>> {
    let mut results = Vec::new();
    for (score, doc_addr) in top_docs {
        let doc: TantivyDocument = searcher.doc(doc_addr)?;
        if let Some(id) = doc.get_first(field_id) {
            if let Some(id_str) = id.as_str() {
                results.push((id_str.to_string(), score));
            }
        }
    }
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_doc(id: &str, header_key: &str, text: &str) -> DocInput {
        DocInput {
            id: id.to_string(),
            header_key: header_key.to_string(),
            text: text.to_string(),
        }
    }

    fn assert_contains(result: &[(String, f32)], expected_id: &str) {
        assert!(
            result.iter().any(|(id, _)| id == expected_id),
            "Expected doc {} in results: {:?}",
            expected_id,
            result
        );
    }

    // ─── CJK tokenization ───

    #[test]
    fn cjk_search_finds_chinese_text() {
        let mut mgr = IndexManager::new();
        mgr.rebuild(&[make_doc("1", "news", "今天北京的天气很好")]).unwrap();
        let results = mgr.search("北京", 5).unwrap();
        assert_contains(&results, "1");
    }

    #[test]
    fn cjk_single_char_search_in_long_run() {
        let mut mgr = IndexManager::new();
        mgr.rebuild(&[make_doc("1", "news", "今天北京的天气很好")]).unwrap();
        // Single character "北" from the middle of the run should match
        assert_contains(&mgr.search("北", 5).unwrap(), "1");
        assert_contains(&mgr.search("京", 5).unwrap(), "1");
        assert_contains(&mgr.search("天", 5).unwrap(), "1");
    }

    #[test]
    fn cjk_bigram_without_spaces() {
        let mut mgr = IndexManager::new();
        // No spaces between CJK characters
        mgr.rebuild(&[make_doc("1", "n", "我愛日本食物")]).unwrap();
        assert_contains(&mgr.search("日本", 5).unwrap(), "1");
        assert_contains(&mgr.search("食物", 5).unwrap(), "1");
        assert_contains(&mgr.search("愛", 5).unwrap(), "1");
    }

    #[test]
    fn cjk_japanese_without_spaces() {
        let mut mgr = IndexManager::new();
        mgr.rebuild(&[make_doc("1", "news", "今日は良い天気です")]).unwrap();
        assert_contains(&mgr.search("天気", 5).unwrap(), "1");
        assert_contains(&mgr.search("今日", 5).unwrap(), "1");
        assert_contains(&mgr.search("良", 5).unwrap(), "1");
    }

    #[test]
    fn cjk_search_matches_bigram() {
        let mut mgr = IndexManager::new();
        mgr.rebuild(&[make_doc("1", "news", "我喜欢吃苹果")]).unwrap();
        let results = mgr.search("苹果", 5).unwrap();
        assert_contains(&results, "1");
    }

    #[test]
    fn cjk_search_korean() {
        let mut mgr = IndexManager::new();
        mgr.rebuild(&[make_doc("1", "news", "오늘 한국 날씨가 좋아요")]).unwrap();
        let results = mgr.search("한국", 5).unwrap();
        assert_contains(&results, "1");
    }

    // ─── BM25 search ───

    #[test]
    fn search_returns_top_k_ordered() {
        let mut mgr = IndexManager::new();
        mgr.rebuild(&[
            make_doc("1", "a", "Rust programming language"),
            make_doc("2", "a", "Python programming language"),
            make_doc("3", "a", "Rust is great for systems programming"),
        ])
        .unwrap();

        let results = mgr.search("Rust programming", 2).unwrap();
        assert_eq!(results.len(), 2);
        // Doc 1 has "Rust" and "programming" twice as many occurrences
        assert!(results[0].1 > results[1].1, "results should be sorted by score desc");
    }

    #[test]
    fn search_top_k_truncates() {
        let mut mgr = IndexManager::new();
        let docs: Vec<_> = (0..20)
            .map(|i| make_doc(&i.to_string(), "t", &format!("doc number {}", i)))
            .collect();
        mgr.rebuild(&docs).unwrap();

        let results = mgr.search("doc", 3).unwrap();
        assert_eq!(results.len(), 3);
    }

    // ─── CRUD ───

    #[test]
    fn rebuild_replaces_all_docs() {
        let mut mgr = IndexManager::new();
        mgr.rebuild(&[make_doc("1", "a", "old text")]).unwrap();
        mgr.rebuild(&[make_doc("2", "b", "new text")]).unwrap();

        let results = mgr.search("old", 5).unwrap();
        assert!(results.is_empty());
        let results = mgr.search("new", 5).unwrap();
        assert_contains(&results, "2");
    }

    #[test]
    fn add_or_update_inserts_and_updates() {
        let mut mgr = IndexManager::new();
        mgr.rebuild(&[make_doc("1", "a", "original text")]).unwrap();

        // Insert new doc
        mgr.add_or_update("2", "b", "second document").unwrap();
        let results = mgr.search("second", 5).unwrap();
        assert_contains(&results, "2");

        // Update existing doc — old content should not be searchable
        mgr.add_or_update("1", "a", "replacement content").unwrap();
        let results = mgr.search("replacement", 5).unwrap();
        assert_contains(&results, "1");
        let results = mgr.search("original", 5).unwrap();
        assert!(
            !results.iter().any(|(id, _)| id == "1"),
            "old content 'original' should be gone after update"
        );
    }

    #[test]
    fn remove_by_header_deletes_docs() {
        let mut mgr = IndexManager::new();
        mgr.rebuild(&[
            make_doc("1", "user", "Alice in Wonderland"),
            make_doc("2", "project", "Rust project"),
            make_doc("3", "user", "Bob the Builder"),
        ])
        .unwrap();

        mgr.remove_by_header("user").unwrap();
        let results = mgr.search("Alice", 5).unwrap();
        assert!(results.is_empty(), "Alice should be removed");
        let results = mgr.search("Bob", 5).unwrap();
        assert!(results.is_empty(), "Bob should be removed");
        let results = mgr.search("Rust", 5).unwrap();
        assert_contains(&results, "2");
    }

    // ─── Edge cases ───

    #[test]
    fn search_empty_index() {
        let mgr = IndexManager::new();
        let results = mgr.search("anything", 5).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn search_no_match() {
        let mut mgr = IndexManager::new();
        mgr.rebuild(&[make_doc("1", "a", "hello world")]).unwrap();
        let results = mgr.search("nonexistent", 5).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn search_english_text() {
        let mut mgr = IndexManager::new();
        mgr.rebuild(&[make_doc("1", "doc", "The quick brown fox jumps over the lazy dog")])
            .unwrap();
        let results = mgr.search("brown fox", 5).unwrap();
        assert_contains(&results, "1");
    }

    #[test]
    fn large_document_set() {
        let mut mgr = IndexManager::new();
        let docs: Vec<_> = (0..1000)
            .map(|i| make_doc(&i.to_string(), "bulk", &format!("bulk document {}", i)))
            .collect();
        mgr.rebuild(&docs).unwrap();

        let results = mgr.search("document", 10).unwrap();
        assert_eq!(results.len(), 10);
        assert!(results[0].1 > 0.0, "should have positive score");
    }

    // ─── Advanced features ───

    #[test]
    fn phrase_search_matches_exact() {
        let mut mgr = IndexManager::new();
        mgr.rebuild(&[
            make_doc("1", "a", "the quick brown fox"),
            make_doc("2", "a", "quick brown fox jumps"),
        ])
        .unwrap();

        // "brown fox" appears in both
        let results = mgr.search_phrase("brown fox", 5).unwrap();
        assert_contains(&results, "1");
        assert_contains(&results, "2");

        // "quick brown fox" appears in both (doc2 starts with it)
        let exact = mgr.search_phrase("quick brown fox", 5).unwrap();
        assert_contains(&exact, "1");
        assert_contains(&exact, "2");
    }

    #[test]
    fn fuzzy_search_tolerates_typo() {
        let mut mgr = IndexManager::new();
        mgr.rebuild(&[make_doc("1", "a", "hello world programming")]).unwrap();
        let results = mgr.search_fuzzy("proggraming", 2, 5).unwrap();
        assert_contains(&results, "1");
    }

    #[test]
    fn search_with_facets_counts() {
        let mut mgr = IndexManager::new();
        mgr.rebuild(&[
            make_doc("1", "user", "Alice"),
            make_doc("2", "user", "Bob"),
            make_doc("3", "project", "Rust"),
        ])
        .unwrap();

        let (results, facets) = mgr.search_with_facets("Alice", 5).unwrap();
        assert_contains(&results, "1");
        assert_eq!(facets.get("user"), Some(&1));
    }

    // ─── Disk-backed index ───

    #[test]
    fn disk_index_create_and_search() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_str().unwrap();

        let mut mgr = IndexManager::open_or_create(path).unwrap();
        mgr.add_or_update("1", "a", "persistent hello world").unwrap();

        let results = mgr.search("persistent", 5).unwrap();
        assert_contains(&results, "1");
    }

    #[test]
    fn disk_index_survives_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_str().unwrap();

        {
            let mut mgr = IndexManager::open_or_create(path).unwrap();
            mgr.add_or_update("1", "a", "data that survives").unwrap();
        } // mgr dropped, index files remain on disk

        {
            let mgr = IndexManager::open_or_create(path).unwrap();
            let results = mgr.search("survives", 5).unwrap();
            assert_contains(&results, "1");
        }
    }
}
