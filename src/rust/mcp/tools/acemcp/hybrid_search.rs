// 混合检索模块
// BM25 关键词检索 + 向量语义检索，通过 RRF（Reciprocal Rank Fusion）融合排名

use std::collections::HashMap;
use anyhow::Result;

use super::local_index::{LocalIndexManager, SearchResult};

// ─── BM25 参数 ───────────────────────────────────────────────────────────────

/// BM25 超参数
const BM25_K1: f32 = 1.5; // 词频饱和系数
const BM25_B: f32 = 0.75; // 文档长度归一化系数

// ─── RRF 参数 ────────────────────────────────────────────────────────────────

/// RRF 平滑常数（通常取 60）
const RRF_K: f32 = 60.0;

// ─── 数据结构 ─────────────────────────────────────────────────────────────────

/// 混合检索结果
#[derive(Debug, Clone)]
pub struct HybridResult {
    pub path: String,
    /// RRF 融合分数（越高越相关）
    pub rrf_score: f32,
    /// BM25 分数（可选，用于调试）
    pub bm25_score: Option<f32>,
    /// 向量相似度分数（可选，用于调试）
    pub vector_score: Option<f32>,
}

/// BM25 文档索引
struct Bm25Index {
    /// 文档词频表：path → (term → tf)
    doc_term_freq: HashMap<String, HashMap<String, u32>>,
    /// 文档词数：path → 词数
    doc_lengths: HashMap<String, usize>,
    /// 逆文档频率：term → idf
    idf: HashMap<String, f32>,
    /// 平均文档长度
    avg_doc_len: f32,
    /// 文档总数
    doc_count: usize,
}

impl Bm25Index {
    /// 从文档集合构建 BM25 索引
    fn build(docs: &[(String, String)]) -> Self {
        let doc_count = docs.len();
        let mut doc_term_freq: HashMap<String, HashMap<String, u32>> = HashMap::new();
        let mut doc_lengths: HashMap<String, usize> = HashMap::new();
        let mut df: HashMap<String, usize> = HashMap::new(); // 文档频率

        for (path, content) in docs {
            let tokens = tokenize(content);
            let len = tokens.len();
            doc_lengths.insert(path.clone(), len);

            let tf_map = doc_term_freq.entry(path.clone()).or_default();
            for token in &tokens {
                *tf_map.entry(token.clone()).or_insert(0) += 1;
            }

            // 统计 DF（每个词在多少文档中出现）
            for term in tf_map.keys() {
                *df.entry(term.clone()).or_insert(0) += 1;
            }
        }

        let total_len: usize = doc_lengths.values().sum();
        let avg_doc_len = if doc_count > 0 {
            total_len as f32 / doc_count as f32
        } else {
            1.0
        };

        // 计算 IDF：ln((N - df + 0.5) / (df + 0.5) + 1)
        let idf: HashMap<String, f32> = df
            .into_iter()
            .map(|(term, df_val)| {
                let n = doc_count as f32;
                let idf_val = ((n - df_val as f32 + 0.5) / (df_val as f32 + 0.5) + 1.0).ln();
                (term, idf_val)
            })
            .collect();

        Self {
            doc_term_freq,
            doc_lengths,
            idf,
            avg_doc_len,
            doc_count,
        }
    }

    /// 计算查询对某文档的 BM25 分数
    fn score(&self, path: &str, query_tokens: &[String]) -> f32 {
        if self.doc_count == 0 {
            return 0.0;
        }

        let tf_map = match self.doc_term_freq.get(path) {
            Some(m) => m,
            None => return 0.0,
        };

        let doc_len = *self.doc_lengths.get(path).unwrap_or(&0) as f32;

        let mut score = 0.0f32;
        for term in query_tokens {
            let idf = *self.idf.get(term).unwrap_or(&0.0);
            let tf = *tf_map.get(term).unwrap_or(&0) as f32;

            // BM25 词频归一化
            let tf_norm = (tf * (BM25_K1 + 1.0))
                / (tf + BM25_K1 * (1.0 - BM25_B + BM25_B * doc_len / self.avg_doc_len));

            score += idf * tf_norm;
        }
        score
    }

    /// 对所有文档打分并排序
    fn search(&self, query: &str, top_k: usize) -> Vec<(String, f32)> {
        let query_tokens = tokenize(query);
        if query_tokens.is_empty() {
            return Vec::new();
        }

        let mut scores: Vec<(String, f32)> = self
            .doc_term_freq
            .keys()
            .map(|path| {
                let s = self.score(path, &query_tokens);
                (path.clone(), s)
            })
            .filter(|(_, s)| *s > 0.0)
            .collect();

        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scores.truncate(top_k);
        scores
    }
}

// ─── 分词器 ───────────────────────────────────────────────────────────────────

/// 简单分词：小写化 + 按非字母数字分割，过滤长度 < 2 的 token
fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|s| s.len() >= 2)
        .map(|s| s.to_string())
        .collect()
}

// ─── RRF 融合 ─────────────────────────────────────────────────────────────────

/// 将两个排名列表通过 RRF 融合
/// rrf_score(d) = Σ 1 / (k + rank(d))
fn rrf_fuse(
    bm25_ranks: &[(String, f32)],
    vector_ranks: &[SearchResult],
    top_k: usize,
) -> Vec<HybridResult> {
    let mut rrf_scores: HashMap<String, f32> = HashMap::new();
    let mut bm25_map: HashMap<String, f32> = HashMap::new();
    let mut vector_map: HashMap<String, f32> = HashMap::new();

    // BM25 排名贡献
    for (rank, (path, score)) in bm25_ranks.iter().enumerate() {
        let contribution = 1.0 / (RRF_K + rank as f32 + 1.0);
        *rrf_scores.entry(path.clone()).or_insert(0.0) += contribution;
        bm25_map.insert(path.clone(), *score);
    }

    // 向量排名贡献
    for (rank, result) in vector_ranks.iter().enumerate() {
        let contribution = 1.0 / (RRF_K + rank as f32 + 1.0);
        *rrf_scores.entry(result.path.clone()).or_insert(0.0) += contribution;
        vector_map.insert(result.path.clone(), result.score);
    }

    // 构建最终结果
    let mut results: Vec<HybridResult> = rrf_scores
        .into_iter()
        .map(|(path, rrf_score)| HybridResult {
            bm25_score: bm25_map.get(&path).copied(),
            vector_score: vector_map.get(&path).copied(),
            rrf_score,
            path,
        })
        .collect();

    results.sort_by(|a, b| {
        b.rrf_score
            .partial_cmp(&a.rrf_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    results.truncate(top_k);
    results
}

// ─── 混合检索器 ───────────────────────────────────────────────────────────────

/// 混合检索器：BM25 + 向量语义，RRF 融合
pub struct HybridSearcher {
    index_manager: LocalIndexManager,
}

impl HybridSearcher {
    pub fn new(index_manager: LocalIndexManager) -> Self {
        Self { index_manager }
    }

    /// 执行混合检索
    ///
    /// - `query`：查询字符串
    /// - `docs`：文档集合 `(path, content)`，用于构建 BM25 索引
    /// - `top_k`：返回结果数量
    pub async fn search(
        &self,
        query: &str,
        docs: &[(String, String)],
        top_k: usize,
    ) -> Result<Vec<HybridResult>> {
        // 扩大候选集，融合后再截断
        let candidate_k = (top_k * 3).max(20);

        // 并行执行 BM25 和向量检索
        let bm25_index = Bm25Index::build(docs);
        let bm25_results = bm25_index.search(query, candidate_k);

        let vector_results = self.index_manager.search(query, candidate_k).await?;

        // RRF 融合
        let fused = rrf_fuse(&bm25_results, &vector_results, top_k);
        Ok(fused)
    }

    /// 仅执行 BM25 检索（不需要向量索引）
    pub fn search_bm25_only(
        &self,
        query: &str,
        docs: &[(String, String)],
        top_k: usize,
    ) -> Vec<HybridResult> {
        let bm25_index = Bm25Index::build(docs);
        let bm25_results = bm25_index.search(query, top_k);
        bm25_results
            .into_iter()
            .map(|(path, score)| HybridResult {
                path,
                rrf_score: score,
                bm25_score: Some(score),
                vector_score: None,
            })
            .collect()
    }
}

// ─── 单元测试 ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_docs() -> Vec<(String, String)> {
        vec![
            (
                "src/auth.rs".to_string(),
                "fn authenticate user login password token jwt".to_string(),
            ),
            (
                "src/database.rs".to_string(),
                "fn query database sql connection pool".to_string(),
            ),
            (
                "src/api.rs".to_string(),
                "fn handle request response http api endpoint".to_string(),
            ),
        ]
    }

    #[test]
    fn test_tokenize() {
        let tokens = tokenize("Hello, World! foo_bar");
        assert!(tokens.contains(&"hello".to_string()));
        assert!(tokens.contains(&"world".to_string()));
        assert!(tokens.contains(&"foo_bar".to_string()));
    }

    #[test]
    fn test_bm25_search() {
        let docs = make_docs();
        let index = Bm25Index::build(&docs);
        let results = index.search("user login", 3);
        assert!(!results.is_empty());
        // auth.rs 应该排第一（包含 user 和 login）
        assert_eq!(results[0].0, "src/auth.rs");
    }

    #[test]
    fn test_rrf_fuse_empty() {
        let fused = rrf_fuse(&[], &[], 5);
        assert!(fused.is_empty());
    }

    #[test]
    fn test_rrf_fuse_bm25_only() {
        let bm25 = vec![
            ("a.rs".to_string(), 2.5f32),
            ("b.rs".to_string(), 1.0f32),
        ];
        let fused = rrf_fuse(&bm25, &[], 5);
        assert_eq!(fused.len(), 2);
        // a.rs 排名更高，RRF 分数应更大
        assert!(fused[0].rrf_score > fused[1].rrf_score);
        assert_eq!(fused[0].path, "a.rs");
    }

    #[test]
    fn test_bm25_index_empty() {
        let index = Bm25Index::build(&[]);
        let results = index.search("anything", 5);
        assert!(results.is_empty());
    }

    // ─── 边界条件测试 ─────────────────────────────────────────────────────────

    #[test]
    fn test_tokenize_empty_string() {
        // Arrange - 空字符串
        let text = "";

        // Act
        let tokens = tokenize(text);

        // Assert - 空字符串应返回空 token 列表，不 panic
        assert!(tokens.is_empty());
    }

    #[test]
    fn test_tokenize_only_punctuation() {
        // Arrange - 全标点符号，无有效 token
        let text = "!@#$%^&*().,;:";

        // Act
        let tokens = tokenize(text);

        // Assert - 全标点应返回空列表
        assert!(tokens.is_empty());
    }

    #[test]
    fn test_tokenize_single_char_filtered() {
        // Arrange - 单字符 token 应被过滤（长度 < 2）
        let text = "a b c hello";

        // Act
        let tokens = tokenize(text);

        // Assert - 单字符 a/b/c 被过滤，只保留 hello
        assert!(!tokens.contains(&"a".to_string()));
        assert!(!tokens.contains(&"b".to_string()));
        assert!(!tokens.contains(&"c".to_string()));
        assert!(tokens.contains(&"hello".to_string()));
    }

    #[test]
    fn test_bm25_single_document() {
        // Arrange - 单文档集合
        let docs = vec![(
            "only.rs".to_string(),
            "fn authenticate user login".to_string(),
        )];
        let index = Bm25Index::build(&docs);

        // Act
        let results = index.search("user login", 5);

        // Assert - 单文档且包含查询词，应有结果
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, "only.rs");
        assert!(results[0].1 > 0.0, "BM25 分数应为正数");
    }

    #[test]
    fn test_bm25_query_empty_string() {
        // Arrange - 空查询字符串（tokenize 后为空）
        let docs = make_docs();
        let index = Bm25Index::build(&docs);

        // Act
        let results = index.search("", 5);

        // Assert - 空查询应返回空结果，不 panic
        assert!(results.is_empty());
    }

    #[test]
    fn test_bm25_no_matching_term() {
        // Arrange - 查询词在所有文档中均不存在
        let docs = make_docs();
        let index = Bm25Index::build(&docs);

        // Act
        let results = index.search("xyznonexistentterm", 5);

        // Assert - 无匹配词应返回空结果
        assert!(results.is_empty());
    }

    #[test]
    fn test_bm25_top_k_truncation() {
        // Arrange - top_k 小于文档数
        let docs = make_docs(); // 3 个文档
        let index = Bm25Index::build(&docs);

        // Act - 只取 1 个结果
        let results = index.search("fn", 1);

        // Assert - 结果数不超过 top_k
        assert!(results.len() <= 1);
    }

    // ─── 异常路径测试 ─────────────────────────────────────────────────────────

    #[test]
    fn test_rrf_fuse_vector_only() {
        // Arrange - 只有向量结果，无 BM25 结果
        use super::super::local_index::SearchResult;
        let vector = vec![
            SearchResult { path: "x.rs".to_string(), score: 0.9 },
            SearchResult { path: "y.rs".to_string(), score: 0.7 },
        ];

        // Act
        let fused = rrf_fuse(&[], &vector, 5);

        // Assert
        assert_eq!(fused.len(), 2);
        // x.rs 排名更高（rank=0），RRF 分数应更大
        assert!(fused[0].rrf_score > fused[1].rrf_score);
        assert_eq!(fused[0].path, "x.rs");
        // 向量分数应被记录
        assert!(fused[0].vector_score.is_some());
        // BM25 分数应为 None
        assert!(fused[0].bm25_score.is_none());
    }

    #[test]
    fn test_rrf_fuse_both_sources_boost_common_doc() {
        // Arrange - 同一文档同时出现在 BM25 和向量结果中，RRF 分数应叠加
        use super::super::local_index::SearchResult;
        let bm25 = vec![
            ("common.rs".to_string(), 2.0f32),
            ("bm25_only.rs".to_string(), 1.5f32),
        ];
        let vector = vec![
            SearchResult { path: "common.rs".to_string(), score: 0.95 },
            SearchResult { path: "vec_only.rs".to_string(), score: 0.8 },
        ];

        // Act
        let fused = rrf_fuse(&bm25, &vector, 5);

        // Assert - common.rs 同时出现在两个列表，RRF 分数应最高
        let common = fused.iter().find(|r| r.path == "common.rs").unwrap();
        let bm25_only = fused.iter().find(|r| r.path == "bm25_only.rs").unwrap();
        let vec_only = fused.iter().find(|r| r.path == "vec_only.rs").unwrap();

        assert!(common.rrf_score > bm25_only.rrf_score, "双源文档 RRF 分数应高于单源");
        assert!(common.rrf_score > vec_only.rrf_score, "双源文档 RRF 分数应高于单源");

        // common.rs 应同时有 bm25_score 和 vector_score
        assert!(common.bm25_score.is_some());
        assert!(common.vector_score.is_some());
    }

    #[test]
    fn test_rrf_fuse_top_k_limits_output() {
        // Arrange - 结果数超过 top_k
        let bm25: Vec<(String, f32)> = (0..10)
            .map(|i| (format!("file{}.rs", i), 10.0 - i as f32))
            .collect();

        // Act
        let fused = rrf_fuse(&bm25, &[], 3);

        // Assert - 输出不超过 top_k
        assert_eq!(fused.len(), 3);
    }

    #[test]
    fn test_search_bm25_only_returns_correct_fields() {
        // Arrange - 验证 search_bm25_only 返回的字段结构
        use crate::mcp::tools::acemcp::embedding_client::{EmbeddingClient, EmbeddingProvider};
        use crate::mcp::tools::acemcp::local_index::LocalIndexManager;
        use tempfile::TempDir;

        let tmp = TempDir::new().unwrap();
        let embedding_client = EmbeddingClient::new(
            EmbeddingProvider::Ollama,
            "http://localhost:11434".to_string(),
            "".to_string(),
            "nomic-embed-text".to_string(),
        );
        let manager = LocalIndexManager::new(
            tmp.path().join("test.idx"),
            embedding_client,
            tmp.path().to_path_buf(),
        );
        let searcher = HybridSearcher::new(manager);
        let docs = make_docs();

        // Act
        let results = searcher.search_bm25_only("user login", &docs, 5);

        // Assert
        assert!(!results.is_empty());
        // search_bm25_only 的结果：rrf_score == bm25_score，vector_score 为 None
        for r in &results {
            assert!(r.bm25_score.is_some(), "bm25_score 应有值");
            assert!(r.vector_score.is_none(), "vector_score 应为 None");
            assert_eq!(r.rrf_score, r.bm25_score.unwrap(), "rrf_score 应等于 bm25_score");
        }
    }

    #[test]
    fn test_rrf_score_formula() {
        // Arrange - 验证 RRF 公式：rank=0 时 score = 1/(60+1) ≈ 0.01639
        let bm25 = vec![("a.rs".to_string(), 1.0f32)];

        // Act
        let fused = rrf_fuse(&bm25, &[], 5);

        // Assert - RRF_K=60, rank=0: 1/(60+0+1) = 1/61
        let expected = 1.0f32 / 61.0f32;
        assert!((fused[0].rrf_score - expected).abs() < 0.0001,
            "RRF 分数应为 1/(K+rank+1) = {}, 实际: {}", expected, fused[0].rrf_score);
    }
}
