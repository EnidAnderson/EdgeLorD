use tantivy::schema::*;
use tantivy::{Index, IndexWriter, IndexReader, ReloadPolicy, doc};
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::TantivyDocument;
use std::sync::{Arc, RwLock};

pub struct LoogleIndex {
    index: Index,
    schema: Schema,
    reader: IndexReader,
    writer: Arc<RwLock<IndexWriter>>,
}

impl LoogleIndex {
    pub fn new_in_memory() -> tantivy::Result<Self> {
        let mut schema_builder = Schema::builder();
        
        // Core fields for lemma search
        schema_builder.add_text_field("name", STRING | STORED);
        schema_builder.add_text_field("lhs_fingerprint", STRING | STORED);
        schema_builder.add_text_field("rhs_fingerprint", STRING | STORED);
        schema_builder.add_text_field("doc", TEXT | STORED);
        schema_builder.add_text_field("provenance", STRING | STORED);
        
        let schema = schema_builder.build();
        let index = Index::create_in_ram(schema.clone());
        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::Manual)
            .try_into()?;
        
        let writer = Arc::new(RwLock::new(index.writer(50_000_000)?));
        
        Ok(Self { index, schema, reader, writer })
    }

    /// Index a lemma from the workspace bundle
    pub fn index_lemma(
        &self,
        name: &str,
        lhs_fp: &str,
        rhs_fp: &str,
        doc: &str,
        provenance: Option<&str>,
    ) -> tantivy::Result<()> {
        let mut writer = self.writer.write().unwrap();
        
        let name_field = self.schema.get_field("name").unwrap();
        let lhs_field = self.schema.get_field("lhs_fingerprint").unwrap();
        let rhs_field = self.schema.get_field("rhs_fingerprint").unwrap();
        let doc_field = self.schema.get_field("doc").unwrap();
        let prov_field = self.schema.get_field("provenance").unwrap();
        
        writer.add_document(doc!(
            name_field => name,
            lhs_field => lhs_fp,
            rhs_field => rhs_fp,
            doc_field => doc,
            prov_field => provenance.unwrap_or(""),
        ))?;
        
        writer.commit()?;
        
        // Reload reader to see newly committed documents
        self.reader.reload()?;
        
        Ok(())
    }

    /// Search for lemmas by structural fingerprint
    pub fn search(&self, query_fp: &str, limit: usize) -> tantivy::Result<Vec<LoogleResult>> {
        let searcher = self.reader.searcher();
        
        let lhs_field = self.schema.get_field("lhs_fingerprint").unwrap();
        let rhs_field = self.schema.get_field("rhs_fingerprint").unwrap();
        
        let query_parser = QueryParser::for_index(&self.index, vec![lhs_field, rhs_field]);
        let query = query_parser.parse_query(query_fp)?;
        
        let top_docs = searcher.search(&query, &TopDocs::with_limit(limit))?;
        
        let mut results = Vec::new();
        for (_score, doc_address) in top_docs {
            let retrieved_doc: TantivyDocument = searcher.doc(doc_address)?;
            let name_field = self.schema.get_field("name").unwrap();
            let doc_field = self.schema.get_field("doc").unwrap();
            
            if let Some(name) = retrieved_doc.get_first(name_field).and_then(|v| v.as_str()) {
                let doc_text = retrieved_doc
                    .get_first(doc_field)
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                
                results.push(LoogleResult {
                    name: name.to_string(),
                    rationale: format!("Structural match: {}", query_fp),
                    doc: doc_text.to_string(),
                });
            }
        }
        
        Ok(results)
    }

    /// Clear all indexed lemmas (for re-indexing)
    pub fn clear(&self) -> tantivy::Result<()> {
        let mut writer = self.writer.write().unwrap();
        writer.delete_all_documents()?;
        writer.commit()?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct LoogleResult {
    pub name: String,
    pub rationale: String,
    pub doc: String,
}

pub mod indexer;
pub mod applicability;
pub mod code_actions;

pub use indexer::WorkspaceIndexer;
pub use applicability::{ApplicabilityResult, LemmaPayload, check_applicability, to_proposal};
pub use code_actions::generate_loogle_actions;

