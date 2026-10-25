# Cognee · MemoryScope · A-MEM — Citation-Grade Research Brief

**Scope:** memory layering/data model · write path · read path · underlying data structures · benchmark numbers (vendor-self-reported vs peer-reviewed).

**Conventions used here**
- Every factual claim carries its source URL.
- `[DOC]` = prose docs, `[CODE]` = raw source file (authoritative for defaults, not "documented" as such), `[README]` = repo README, `[PAPER]` = arXiv paper, `[VENDOR]` = vendor-published self-reported performance.
- Anything I could not verify is marked **⚠️ UNVERIFIED**.

## Source-access notes (what worked)

| Target | Result |
|---|---|
| `https://docs.cognee.ai/<page>.md` | ✅ Mintlify `.md` suffix returns clean markdown (nav chrome stripped). Index at `https://docs.cognee.ai/llms.txt` |
| `https://raw.githubusercontent.com/topoteretes/cognee/main/README.md` | ✅ |
| `https://raw.githubusercontent.com/agiresearch/A-mem/main/...` | ✅ (also `agentic_memory/memory_system.py` in full) |
| `https://arxiv.org/html/2502.12110v11`, `https://ar5iv.labs.arxiv.org/html/2502.12110` | ✅ |
| `https://raw.githubusercontent.com/agentscope-ai/ReMe/memoryscope_branch/...` | ✅ **this is where MemoryScope now lives** |
| `https://api.github.com/repos/agentscope-ai/ReMe/git/trees/memoryscope_branch?recursive=1` | ✅ full file tree |
| `https://modelscope.github.io/MemoryScope/en/index.html` | ❌ **HTTP 404 — "There isn't a GitHub Pages site here."** The MemoryScope doc site is dead. Repo docs are the only primary source. |
| `https://pypi.org/project/cognee/0.1.27/` | ❌ "Client Challenge" bot wall → used the tagged README instead |
| `https://api.github.com/...` (after ~10 calls) | ❌ HTTP 403 rate-limited |

---

# 0. Corrections to the research brief (read this first)

1. **⚠️ arXiv:2502.07918 is NOT MemoryScope.** That ID is *"Filtered Markovian Projection: Dimensionality Reduction in Filtering for Stochastic Reaction Networks"* (math.NA; Ben Hammouda, Chupin, Münker, Tempone) — https://arxiv.org/abs/2502.07918. The title *"MemoryScope: A Holistic Agent Memory Framework"* could not be verified anywhere. **No arXiv/peer-reviewed paper for MemoryScope was found at all** — the repo ships only a *software* citation (`@software{MemoryScope, author = {Li Yu and Tiancheng Qin and Qingxu Fu and Sen Huang and Xianzhe Xu and Zhaoyang Liu and Boyin Liu}, month = {09}, title = {{MemoryScope}}, url = {https://github.com/modelscope/MemoryScope}, year = {2024}}`) — https://raw.githubusercontent.com/agentscope-ai/ReMe/memoryscope_branch/README.md
2. **`github.com/modelscope/MemoryScope` is now a redirect.** `https://api.github.com/repositories/849121458` (the MemoryScope repo id) resolves to `agentscope-ai/ReMe`, default branch `main`, topics include `memoryscope`. **MemoryScope = the `memoryscope_branch` archive** (v0.1.1.0, released 2024-09-10), and the successor project is **ReMe** — https://api.github.com/repositories/849121458 , https://github.com/agentscope-ai/ReMe/tree/memoryscope_branch
3. **The MemoryScope operator names in the brief do not exist as such.** There is no `ExtractMemory` / `UpdateMemory` / `ConsolidateMemory` / `ReflectMemory` class family. The real architecture is `MemoryOperation` configs → `FrontendOperation` / `BackendOperation` → **WorkerFlow** (a comma/paren workflow DSL) → `*Worker` classes. Two operations *are* named `consolidate_memory` and `reflect_and_reconsolidate`. §2(b) has the exact names.
4. **Cognee's "ECL" branding is historical, not current.** The exact string is in the v0.1.27 tagged README: *"Cognee implements scalable, modular **ECL (Extract, Cognify, Load)** pipelines"* — https://raw.githubusercontent.com/topoteretes/cognee/v0.1.27/README.md. Current docs do not use "ECL": the v1.0 API is `remember` / `recall` / `improve` / `forget`, with `add` / `cognify` / `memify` / `search` retained as legacy lower-level operations — https://docs.cognee.ai/core-concepts/overview.md
5. **A-MEM venue is confirmed, and it is peer-reviewed:** arXiv Comments field reads *"Advances in Neural Information Processing Systems (NeurIPS 2025)"* (added in a later revision; v1 carried no such comment) — https://arxiv.org/abs/2502.12110 . Independent index confirms: https://mlanthology.org/neurips/2025/xu2025neurips-amem/ (OpenReview forum id `FiM0M8gcct`). **⚠️ UNVERIFIED:** the acceptance *type* (poster/spotlight/oral) — OpenReview was unreachable (browser challenge).
6. **A-MEM repo naming is inconsistent between sources.** The arXiv v11 abstract says eval code is at `https://github.com/WujiangXu/A-mem` and the system code at `https://github.com/WujiangXu/A-mem-sys` (page exists) — https://arxiv.org/abs/2502.12110 , https://github.com/WujiangXu/A-mem-sys . But the current production README says: *"If you want to reproduce the results presented in our paper, please refer to: https://github.com/WujiangXu/AgenticMemory"* and gives production code as `agiresearch/A-mem` — https://raw.githubusercontent.com/agiresearch/A-mem/main/README.md . So there are effectively three repo names in circulation for two codebases. `WujiangXu/AgenticMemory` and `WujiangXu/A-mem` appear to be the same repository (GitHub repo id 924725391), but **⚠️ UNVERIFIED by direct API check** (rate-limited).

---

# 1. Cognee

Repo: https://github.com/topoteretes/cognee · docs: https://docs.cognee.ai/ · paper: https://arxiv.org/abs/2505.24478

## (a) Layering / data model

**Three co-equal stores, not tiers.** `[DOC]` https://docs.cognee.ai/core-concepts/architecture.md

> "Cognee uses three complementary storage systems, each playing a different role … **Relational store** — Tracks documents, chunks, and provenance … **Vector store** — Holds embeddings for semantic similarity … **Graph store** — Captures entities and relationships in a knowledge graph"

> "The **relational store** handles document-level metadata and provenance. The **vector store** contains semantic fingerprints of chunks and DataPoints. The **graph store** captures higher-level structure in the form of entities and relationships. There is some overlap: for efficiency, parts of the same information may be indexed in more than one store."

**Memory tiers at the API level (v1.0): permanent vs session.**

| | Session cache (short-term) | Permanent memory (knowledge graph) |
|---|---|---|
| Write | `remember(data, session_id=...)` | `remember(data)` (no `session_id`) |
| What happens | "Raw text is written straight to the cache as a Q&A entry — **no chunking, no entity extraction, no embeddings**" | "the full Add + Cognify pipeline: chunking, entity/relationship extraction, and embeddings, plus an Improve pass" |
| Scope | keyed by `(user_id, session_id)` | a named dataset (default `main_dataset`) |
| Lifetime | "Expires roughly `SESSION_TTL_SECONDS` (**default 7 days** / `604800`) after the session's last write" | durable until `forget()` |

Source: https://docs.cognee.ai/core-concepts/sessions-and-caching.md . Conversation-history window is **a fixed 10 entries, not configurable**; `AUTO_FEEDBACK` (default `true`) adds **one structured-output LLM call per answered turn**; session guidance is grouped into `goals`, `rules`, `preferences`, `lessons_learned`; `SESSION_SEARCH_MODE` is `concurrent` (default) or `sequential` — same page.

**DataPoints — the atomic unit.** `[DOC]` https://docs.cognee.ai/core-concepts/building-blocks/datapoints.md

> "DataPoints are the smallest building blocks in Cognee. They represent **atomic units of knowledge**"

Base fields (verbatim class): `id: UUID`, `created_at`, `updated_at`, `version: int = 1`, `topological_rank`, `valid_to`, `metadata: MetaData = {"index_fields": []}`, `type: str = "DataPoint"`, `ontology_uri`, `belongs_to_set`.
- The `id` is **"shared across all three stores, linking vector, graph, and relational records for the same DataPoint"**; cross-store retrieval = vector hit → graph node by `id`.
- `metadata` TypedDict keys: `index_fields` (required), `type`, `identity_fields`, `transparent`.
- Each indexed field becomes **its own vector collection named `Class_field`** (e.g. `Entity_name`, `DocumentChunk_text`, `TextSummary_text`).
- Built-in types: `Document`, `DocumentChunk`, `TextSummary` / `CodeSummary` / `GlobalContextSummary`, `Entity`, `EntityType`, `Edge`.
- Dedup identity: `uuid5(NAMESPACE_OID, "ClassName:value1|value2|...")` from `Dedup()` / `identity_fields`; falls back to random UUID4.
- `embed_triplets=True` on `add_data_points()` embeds each `(subject → predicate → object)` as a `Triplet` DataPoint.

**LLM-extracted graph schema.** `[DOC]` https://docs.cognee.ai/core-concepts/main-operations/legacy-operations/cognify.md — node = `id` (chunk-local handle only), `name`, `type` (e.g. `Person`), `description`; edge = `source_node_id`, `target_node_id`, `relationship_name` (free-text verb, e.g. `works_at`). "The identity of the stored node is derived from its `name` (normalized to lowercase, with spaces turned into underscores and apostrophes stripped)". Schema is emergent, not fixed: "The resulting graph schema emerges from the data you ingest."

**Ontology support — yes, optional RDF/OWL.** `[DOC]` https://docs.cognee.ai/core-concepts/further-concepts/ontologies.md
- Supplied by `ONTOLOGY_FILE_PATH` env var **or** `config={"ontology_config": {"ontology_resolver": RDFLibOntologyResolver(ontology_file=...)}}`; parsed with RDFLib. `cognify()` has **no** `ontology_file_path` parameter.
- Grounding modes: `ONTOLOGY_MODE=annotate` (default; unmatched entities kept verbatim with `ontology_valid = False`) or `strict` (drops nodes matching neither `type`→class nor `name`→individual; raises `EmptyOntologyInStrictModeError` over an empty ontology).
- Matching is name-based fuzzy matching at a **`0.8` similarity cutoff** (`FuzzyMatchingStrategy`, `MATCHING_STRATEGY=fuzzy`), keyed on the **IRI local name** (not `rdfs:label`).
- Only `rdf:type owl:Class`, typed individuals, `rdfs:subClassOf`, `rdf:type owl:ObjectProperty` are read. **"No axioms are enforced. `rdfs:domain` / `rdfs:range`, `owl:equivalentClass`, `owl:sameAs`, `owl:Restriction`, and cardinality … are never consulted — no reasoner runs."**
- Matched IRIs are preserved in `DataPoint.ontology_uri`; `ingest_rdf()` imports RDF T-Box/A-Box; `serialize_memory_graph()` / `query_memory_graph_sparql()` export the graph back to RDF.
- A **custom graph model** (`graph_model=` a `DataPoint` subclass) acts *before* extraction (it is the LLM's structured-output schema); an ontology acts *after*. "**the two do not stack within a single `cognify()` / `remember()` call**".

## (b) Write path

**v1.0 single call:** `remember()` = `Add` → `Cognify` → `Improve` "under the hood" — https://docs.cognee.ai/core-concepts/main-operations/remember.md . Permanent path: "**Ingest** → **Build the graph** → **Enrich**"; session path: "**Store in session cache** → **Return immediately** → **Optional background bridge**".

**Legacy ECL:** *"Cognee implements scalable, modular ECL (Extract, Cognify, Load) pipelines"* — https://raw.githubusercontent.com/topoteretes/cognee/v0.1.27/README.md

**Is "cognify" an LLM-based graph-construction step? Yes.** `[DOC]` https://docs.cognee.ai/core-concepts/main-operations/legacy-operations/cognify.md — six ordered Tasks:

1. **Classify documents** — wrap each file as a `Document` (+ metadata, node sets)
2. **Check permissions** — write access to the dataset
3. **Extract chunks** — `DocumentChunk`s via a chunker; updates token counts in the relational DB
4. **Extract graph** — "Calls the LLM to extract entities and relationships. Deduplicates nodes and edges, commits to the graph DB"
5. **Summarize text** — per-chunk `TextSummary` DataPoints
6. **Add data points** — "Converts summaries and other DataPoints into graph + vector nodes. Embeds them in the vector store, persists in the graph DB"

Plus two opt-in tasks: **7. Record provenance** (`PROVENANCE_TRACKING=true`; append-only `provenance_entries` table, SHA-256 chained, `verify_chain()`), **8. Detect contradictions** (`CONTRADICTION_DETECTION=true`; writes `contradicts` edges with `first_fact`, `second_fact`, `reason`, `confidence`; `CONTRADICTION_CONFIDENCE_THRESHOLD` default `0.5`, `CONTRADICTION_MAX_FACTS` default `500`).

**Trigger & cost:** `cognify()` is called by the app (or inside `remember()`); it is **not** automatic. "The default cognify pipeline makes **2 LLM calls per chunk**" (graph extraction + summarization); `chunks = ceil(document_tokens / chunk_size)`, `total_llm_calls = chunks × 2`, `chunk_size = min(embedding_model_max_tokens, llm_max_tokens ÷ 2)` → typically **1024–8192 tokens**. Batching: `data_per_batch` default **20**; `chunks_per_batch` default **2000** (temporal Cognify `10`; `remember` HTTP form `36`). Incremental: content-hash dedup in `add()` + `pipeline_status == COMPLETED` skip in `cognify()`; failure control `RAISE_INCREMENTAL_LOADING_ERRORS` default `true`. **LLM-free mode:** `extractor="gliner_demo"` / `GRAPH_EXTRACTOR=auto` (auto picks GLiNER when no usable LLM key) — same URL.

**"Memify" / memory-evolution step — yes, both exist; "memify" is now the legacy name.**
- `[DOC]` **Improve** (current): "`improve()` always runs the same **nine stages in a fixed order: `feedback_weights`, `persist_session_qa`, `persist_agent_traces`, `extract_agent_context`, `distill_sessions`, `update_user_preferences`, `build_truth_subspace`, `triplet_enrichment`, `global_context_index`**." Stages report `skipped` (e.g. `no_session_ids`, `opt_in_disabled`) or `already_completed`. Session distillation uses "**a curator [that] proposes durable lessons, a writer/rejecter [that] checks them against existing lessons and graph entities**"; accepted lessons are cognified and tagged `session_learnings`. Feedback weights update `feedback_weight` on graph nodes/edges used in retrieval. `PERSONALIZATION_ENABLED` is **off by default** (preference subgraph writes nothing when off). Failure in any stage except **Persist session Q&A** does not stop the run — https://docs.cognee.ai/core-concepts/main-operations/improve.md
- `[DOC]` **Memify** (legacy): two stages — *Extraction* + *Enrichment*. Default pipeline = `get_triplet_datapoints` → `index_data_points` (indexes into the `Triplet_text` collection, enabling `SearchType.TRIPLET_COMPLETION`). Other built-ins: coding rules (`extract_subgraph_chunks` → `add_rule_associations`; `Rule` nodes + `rule_associated_from` edges under node set `coding_agent_rules`), session persistence (`extract_user_sessions` → `cognify_session`; node set `user_sessions_from_cache`), entity consolidation (`get_entities_with_neighborhood` → `generate_consolidated_entities` → `generate_type_descriptions` → `add_data_points`), cross-connect entities (`get_entity_nodes` → `cross_connect_entities`; defaults `similarity_threshold 0.5`, `overlap_threshold 0.2`, `max_new_edges_per_node 5`, `confidence_threshold 0.7`, `dry_run=False`; written edges carry `inferred: True`, `confidence`, `feedback_weight: 0.2`). Pipeline cache: `cognify()` `True` (skips processed data) vs `memify()` `False` (always re-processes) — https://docs.cognee.ai/core-concepts/main-operations/legacy-operations/memify.md

**Three data-flow pipelines** (session learning / self-improvement / ingestion), all built from the same Task+DataPoint structure — https://docs.cognee.ai/core-concepts/data-flows.md

## (c) Read path

`[DOC]` https://docs.cognee.ai/core-concepts/main-operations/recall.md

1. **Check session scope** — with `session_id` and no `datasets`/`query_type`, "Cognee first searches the session cache directly. Session search is **keyword-based** over stored question, context, and answer fields."
2. **Choose the retrieval strategy** — auto-routing is on by default and is **"rule-based, not LLM-based"**: exactly two ordered regex rules — `quoted_phrase` → `CHUNKS_LEXICAL`, `coding_rules_intent` → `CODING_RULES`; **anything else → `HYBRID_COMPLETION`**. No usable LLM configured → resolves to `CHUNKS`. `auto_route=False` → `HYBRID_COMPLETION`. `FEELING_LUCKY` is the only mode with an LLM selector (falls back to `RAG_COMPLETION`). A routed type that is rejected/empty is retried once as `HYBRID_COMPLETION`.
3. **Run graph retrieval** — "for permanent memory, `recall()` runs graph retrieval — **not plain embedding similarity**"; `HYBRID_COMPLETION` "already searches document chunks, summaries, and the entity neighbourhood in one LLM call".

Retrieval triggers: an explicit `recall()`/`search()` call. Results are tagged with `source` (`"graph"`, `"session"`, `"trace"`, `"session_context"`); graph results are `ResponseGraphEntry` (`text`, `kind`, `search_type`, `dataset_id`, `dataset_name`, `metadata`, `raw`, `structured`); session results are `ResponseQAEntry`. `top_k` default **15**. `only_context=True` returns the assembled prompt instead of an answer and skips the session QA write. `CYPHER` is **never** auto-routed (needs explicit `query_type` + `ALLOW_CYPHER_QUERY=true`) because routing on text "would let a request body such as `MATCH (n) DETACH DELETE n` mutate the graph".

Manual-only search types (not reachable by the router): `GRAPH_COMPLETION_COT`, `GRAPH_COMPLETION_CONTEXT_EXTENSION`, `TEMPORAL`, `GRAPH_SUMMARY_COMPLETION`, `CYPHER`, `GRAPH_COMPLETION`, `SUMMARIES`, `CHUNKS`, `RAG_COMPLETION`, `TRIPLET_COMPLETION`, `GRAPH_COMPLETION_DECOMPOSITION`, `NATURAL_LANGUAGE`, `AGENTIC_COMPLETION`. (Same page.)

Recall **writes** too: with session-enabled graph retrieval it appends a QA entry (`question`, `context`, `answer`) to the session cache.

## (d) Data structures — graph + vector + relational (all three)

`[DOC]` https://docs.cognee.ai/setup-configuration/graph-stores.md , https://docs.cognee.ai/setup-configuration/vector-stores.md

| Layer | Default | Production options | Community |
|---|---|---|---|
| **Graph** | **Kuzu** — "Local file-based graph database (default)"; embedded Ladybug/Kuzu engine run in a dedicated worker process; `KUZU_BUFFER_POOL_SIZE` default `34359738368` (32 GB); `KUZU_MAX_DB_SIZE` 32 GB; **"not for large-scale production"**: "one writer, on one machine, with no replication or failover" | **Neo4j** (self-hosted/Docker; "Recommended"), Neo4j Aura, Neo4j Desktop (dev only), **Postgres** (`postgres_demo` — "**demo feature**", production adapter is a **licensed product**), Amazon Neptune, Neptune Analytics | Memgraph |
| **Vector** | **LanceDB** — "Built-in, default … file-based"; default path `<SYSTEM_ROOT_DIRECTORY>/databases/cognee.lancedb` | **PGVector** (one table per indexed field, `{DataPointType}_{field}`, columns `id` / `payload` / `vector`), Turso (libSQL), Neptune Analytics, ChromaDB | Qdrant, Redis, FalkorDB, Pinecone, Turbopuffer, Milvus, Weaviate |
| **Relational** | SQLite (default); Postgres supported | — | — |

Also: **Kuzu-remote** (HTTP in front of the same single-writer file store), **FalkorDB** as a hybrid graph+vector community adapter. Single-process constraint: "The embedded file-based defaults (Kuzu graph, SQLite, LanceDB) must not be shared across processes" — https://docs.cognee.ai/core-concepts/main-operations/remember.md . `EMBEDDING_DIMENSIONS` must match the collection width or you get `EmbeddingDimensionMismatchError` (HTTP 409) — vector-stores page.

## (e) Benchmarks — vendor-self-reported (no peer-reviewed Cognee-system numbers found)

**BEAM evaluation — vendor-self-reported.** `[README]` https://raw.githubusercontent.com/topoteretes/cognee/main/README.md

> "The BEAM evaluation (cognee/eval_framework/beam/REPORT.md) measures conversational memory using synthetic long-context conversations and an LLM judge. The reported runs use Cognee's memory components with **benchmark-specific data formatting, prompts, and retrieval configuration**."

| BEAM context | Reported score (0–1) | Scope (verbatim) |
|---|---|---|
| 100K tokens | **0.79** | "Fixed hybrid retrieval; four evaluation rounds over 20 questions from one held-out conversation." |
| 10M tokens | **0.67** | "Exploratory result; question-type routing selected and scored on the same question set, averaged over five rounds." |

The README itself warns: "The two settings use different conversations, ingestion models, and retrieval-selection procedures. Read the methodology … before comparing these scores with other systems. The report also documents the remaining reproduction gap for the distributed 10M ingestion."

**Research paper (Cognee authors):** *"Optimizing the Interface Between Knowledge Graphs and LLMs for Complex Reasoning"*, Markovic, Obradovic, Hajdu, Pavlovic, 2025 — https://arxiv.org/abs/2505.24478 (cited in the README's BibTeX).

**BEAM — verified in detail.** The report exists and is not a 404: https://raw.githubusercontent.com/topoteretes/cognee/main/cognee/eval_framework/beam/REPORT.md . Unrounded artifacts (exact, not rounded): 100K mean **`0.794375`**, `run_std 0.004571`, `kendall_tau 0.514639` — https://raw.githubusercontent.com/topoteretes/cognee/main/cognee/eval_framework/beam/report_artifacts/100k_fixed/hybrid_completion_20_20_qa_v1_cross_run_summary.json ; 10M mean **`0.667379`**, `run_std 0.019695`, `kendall_tau 0.088426` — https://raw.githubusercontent.com/topoteretes/cognee/main/cognee/eval_framework/beam/report_artifacts/10m_routed/routed_by_question_type_cross_run_summary.json . No other context sizes are reported.

- **Sample shape:** 20 probing questions per conversation — "10 types; 2 questions per type"; 100K = 4 QA+scoring rounds, 10M = 5 rounds; repetition "does not add new question samples" (REPORT.md). Independently corroborated: the HuggingFace BEAM `100K` split has exactly 20 examples — https://huggingface.co/api/datasets/Mohammadta/BEAM
- **Judge:** *"Answers were evaluated with BEAM's upstream rubric-based LLM procedure, ported into Cognee's evaluation framework without changing its scoring rules. BEAM scores each rubric criterion at 0, 0.5, or 1 and averages the criteria into a question score."* Appendix A models: embeddings `openai/text-embedding-3-large`; ingestion `openai/gpt-5` (100K) / `openai/gpt-5-mini` (10M); answer generation `openai/gpt-5`; **rubric judge `openai/gpt-5` in a separate call** (REPORT.md).
- **Per-ability, 100K** (Appendix E; 2 questions each, "read directionally"): information_extraction 0.979, knowledge_update 1.000, preference_following 1.000, instruction_following 1.000, contradiction_resolution 0.875, summarization 0.802, event_ordering 0.788, multi_session_reasoning 0.500, temporal_reasoning 0.500, abstention 0.500.
- **The report's own caveats, verbatim:** *"These scores are not a controlled comparison across scales: the conversations, ingestion models, and retrieval-selection procedures differ."* · *"Because selection and reporting use the same question set, this is an in-sample exploratory result."* · *"we did not run controlled ablations. The scores therefore describe the combined pipeline, not the contribution of each component."* · *"A fully public benchmark limits evidence of generalization… not equivalent to a hidden test set."* · *"The 10M ingestion ran in Cognee distributed mode on Modal. The public code includes the same preprocessing representation and the local path above, but not the BEAM-specific distributed orchestration used for that run."* (all REPORT.md)

**Head-to-head numbers — every one is Cognee-run.** 24-question HotPotQA subset, 45 repeated runs per system, "same DeepEval scoring", competitors on published defaults, Cognee tuned — https://www.cognee.ai/knowledge-graph-memory-benchmarks

| System | Human-like Correctness | DeepEval Correctness | F1 |
|---|---|---|---|
| **Cognee** (`GRAPH_COMPLETION_COT`, tuned) | **0.93** | **0.85** | **0.84** |
| Graphiti (LangChain + Neo4j, default) | 0.88 | 0.74 | 0.70 |
| LightRAG (default) | 0.96 | 0.67 | 0.09 |
| Mem0 (OpenAI memory QA, default) | 0.72 | 0.54 | 0.12 |

Cognee's own disclosure, verbatim: *"cognee was running tuned and Graphiti wasn't. We didn't run a tuning sweep on the competitors"*; *"**This is cognee's benchmark, not an independent study.** We built the harness and we published the numbers. That is useful but not neutral."* The harness is archived: *"45 evaluation cycles on 24 HotPotQA questions"* — https://raw.githubusercontent.com/topoteretes/cognee/main/evals/old/hotpot_qa_24_2025/README.md , and *"The previous 24-question HotpotQA evaluation suite is retained for historical reference… archived and is no longer the current Cognee benchmark"* — https://raw.githubusercontent.com/topoteretes/cognee/main/evals/README.md

**BEAM comparison chart** (https://www.cognee.ai/research-and-evaluation-results): Cognee `0.79 / 0.67`, Hindsight `0.73 / 0.64`, Mem0 `N/A / 0.49`, Honcho `0.63 / 0.41`, RAG (llama-4) `0.32 / 0.25`. No metric definition, sample size, or per-system methodology is published for this chart; **no Graphiti and no Zep**. Two cross-checks that undermine it: (i) Cognee's `0.49` for Mem0 matches Mem0's *average score* 0.486, not Mem0's self-published BEAM-10M pass rate 50.5% — https://raw.githubusercontent.com/mem0ai/memory-benchmarks/main/README.md ; (ii) Hindsight's own benchmark network publishes BEAM **100K 86.2% / 500K 80.1% / 1M 79.1% / 10M 64.1%** — https://agentmemorybenchmark.ai/ — so Cognee's headline *"Previous SOTA 0.73"* matches no Hindsight 100K figure (0.73 is closest to Hindsight's 1M = 0.739). **⚠️ UNVERIFIED: the "0.73 @ 100K previous SOTA" attribution** (https://www.cognee.ai/research-and-evaluation-results). Hindsight's own paper reports no BEAM numbers at all: https://arxiv.org/abs/2512.12818 . Also, internal inconsistency: the research page renders the CoT comparison as 0.738 → 0.925 (human-like), 0.569 → 0.846 (DeepEval correctness), 0.203 → 0.841 (F1), 0.04 → 0.687 (EM) versus the blog's rounded 0.93 / 0.85 / 0.84. **⚠️ UNVERIFIED: any Cognee-vs-Zep head-to-head numeric comparison** — https://www.cognee.ai/cognee-vs-zep publishes no Zep figures.

**Paper numbers (arXiv:2505.24478), vendor-self-reported and preprint-only.** Table 2 (training) — HotPotQA Correctness 0.476 → **0.815**, EM 0.042 → **0.667**, F1 0.169 → **0.840**; Musique 0.414 → 0.674 / 0.000 → 0.500 / 0.145 → 0.654; TwoWikiMultiHop 0.348 → 0.582 / 0.000 → 0.458 / 0.148 → 0.625. Table 3 (hold-out) — HotPotQA EM **0.583**, Correctness **0.715**, F1 **0.819**; Musique 0.375 / 0.596 / 0.581; TwoWikiMultiHop 0.417 / 0.482 / 0.704 — https://arxiv.org/html/2505.24478v1 . Method: 9 experiments, *"24 training instances and 12 test instances per dataset"*, 50 TPE trials; *"The correctness score is computed using DeepEval, an LLM-based grading tool"*; the baseline is *"the default configuration used prior to optimization"* — i.e. **Cognee's own pre-optimization defaults, not an external RAG baseline**. Peer-review status: **none** — arXiv comments read *"This is a preliminary version. A revised and expanded version is in preparation"* (https://arxiv.org/abs/2505.24478); OpenAlex records `"type":"preprint"`, `"is_accepted":false`, `"is_published":false` (https://api.openalex.org/works/doi:10.48550/arxiv.2505.24478); no Crossref record. The blog calls it "one peer-reviewable paper", not peer-reviewed.

**Built-in eval framework defaults** (`[DOC]` https://docs.cognee.ai/integrations/eval-framework.md): metric keys `EM`, `f1`, `correctness`, `contextual_relevancy`, `context_coverage`, `beam_rubric`, `kendall_tau`; engines `DeepEval`, `DirectLLM`, `BeamEval`; benchmarks `Dummy` (default), `HotPotQA`, `Musique`, `TwoWikiMultiHop`, `BEAM` ("Preliminary support"); `DEEPEVAL_MODEL=gpt-4o-mini`; `QA_ENGINE=cognee_graph_completion`. **RAGAS is not used** — no `ragas` module or `faithfulness`/`answer_relevancy` strings in the framework. **⚠️ UNVERIFIED:** the metric names `deep_eval_g_answer_relevancy`, `deepeval_faithfulness`, `hotpot_qa` do not exist in Cognee's current docs or `eval_config.py`.

**⚠️ Flag (net):** no peer-reviewed publication reports Cognee's own retrieval accuracy. The BEAM 0.79 / 0.67 figures are genuine and reproducible from Cognee's own artifacts, but they are **vendor-run, gpt-5-LLM-judged, 20 questions per scale, with the 10M result declared exploratory and in-sample by Cognee itself**. The earlier statement in `research/03-memory-frameworks.md` that "本报告未找到 Cognee 官方公布的与 Mem0/Zep 同口径 benchmark 数字" needs correcting in one direction and confirming in another: Cognee **does** now publish BEAM numbers plus a HotPotQA head-to-head against Mem0/Graphiti/LightRAG, but **there is still no same-harness comparison with Zep**, and Cognee's own blog concedes the comparison is neither neutral nor tuning-symmetric.

---

# 2. MemoryScope

**Canonical location:** `https://github.com/agentscope-ai/ReMe/tree/memoryscope_branch` (the `modelscope/MemoryScope` repo redirects here). Version badge: `v0.1.1.0`, news entry "**[2024-09-10]** We release MemoryScope v0.1.1.0 now, which is also available in PyPI" — all in https://raw.githubusercontent.com/agentscope-ai/ReMe/memoryscope_branch/README.md

## (a) Layering / data model — the actual tier names

**The persistent tiers are an enum, not prose labels.** `[CODE]` https://raw.githubusercontent.com/agentscope-ai/ReMe/memoryscope_branch/memoryscope/enumeration/memory_type_enum.py

```python
class MemoryTypeEnum(str, Enum):
    CONVERSATION = "conversation"
    OBSERVATION = "observation"
    INSIGHT = "insight"
    OBS_CUSTOMIZED = "obs_customized"
```

So: **`conversation`** (raw source), **`observation`** (extracted user facts), **`insight`** (aggregated high-level), **`obs_customized`** (customized observations). The README states the hierarchy: *"The memory pieces stored in the system are in a hierarchical structure, with **insights** being the high level information from the aggregation of similarly-themed **observations**."* — [README] same URL.

**"Working memory" is not a DB tier** — it is the in-process rolling dialogue window. The config labels two frontend operations explicitly: `read_message` → description **"read short memory"**, `retrieve_memory` → description **"retrieve long-term memory"** — `[CODE]` https://raw.githubusercontent.com/agentscope-ai/ReMe/memoryscope_branch/memoryscope/core/config/demo_config.yaml . `ReadMessageWorker` "Fetches unmemorized chat messages" and returns the last `contextual_msg_max_count` batches from the in-memory `self._chat_messages` list — `[CODE]` .../core/worker/frontend/read_message_worker.py , .../core/service/base_memory_service.py

**The stored record — `MemoryNode`.** `[CODE]` https://raw.githubusercontent.com/agentscope-ai/ReMe/memoryscope_branch/memoryscope/scheme/memory_node.py
Fields: `memory_id` (uuid4 hex), `user_name`, `target_name`, `meta_data: Dict[str,str]`, `content`, `key`, `key_vector`, `value`, `score_recall`, `score_rank`, `score_rerank`, `memory_type`, `action_status` (`none`/`new`/`content_modified`/`modified`/`deleted`), `store_status` (`valid`/`expired`), `vector`, `timestamp`, `dt`, `obs_reflected` (0/1), `obs_updated` (0/1).
- An **observation's `meta_data` carries the source conversation verbatim** under key `conversation`, plus `TIME_INFER`, `keywords`, date-part dict, and `event_*` date parts — `[CODE]` .../core/worker/backend/get_observation_worker.py
- `key` / `value`: observations use `content`; insights use `key` (the theme) + `value` (the insight text) — `[CODE]` .../core/worker/backend/update_insight_worker.py

## (b) Write path — two background operations, decoupled from the request path

The README's own framing: *"Backend operations (Memory Consolidation, Reflection and Re-consolidation) are **decoupled from the frontend operation** (Memory Retrieval) … the system's response time (RT) for the user depends solely on the frontend operation, which is only **~500ms**."* — [README]

Exact operation workflows and intervals (`[CODE]` demo_config.yaml):

```yaml
consolidate_memory:
  class: core.operation.consolidate_memory_op
  workflow: info_filter,[get_observation|get_observation_with_time|load_today_memory],contra_repeat,store_memory
  interval_time: 1

reflect_and_reconsolidate:
  class: core.operation.backend_operation
  workflow: load_obs_and_insight,get_reflection_subject,update_insight,long_contra_repeat,store_memory
  interval_time: 15
```

`BackendOperation` runs `_loop_operation` in a thread-pool thread, sleeping `interval_time` **seconds** between runs, guarded by `_operation_status_run` to avoid overlap — `[CODE]` .../core/operation/backend_operation.py

**Trigger for consolidation** (`[CODE]` .../core/operation/consolidate_memory_op.py): take chat-message batches whose first message is **not** `memorized` and which contain a message authored by `target_name`; skip if `len(chat_messages) < contextual_msg_min_count`; after the run, set `msg.memorized = True` under `message_lock`.

**Pipeline steps** (all `[CODE]`, paths under `memoryscope/core/worker/backend/`):
1. **`info_filter_worker`** — LLM filter of which query info is worth memorizing.
2. **`get_observation_worker`** — LLM extracts observations, parsed as 4-tuples `[index, time_inference, observation_content, keywords]`; entries equal to the localized `none`/`repeated` words are dropped; the source message content is stored under meta key `conversation`; `get_observation_with_time_worker` is the time-aware variant.
3. **`contra_repeat_worker`** — de-duplication/contradiction pass: `retrieve_top_k` default **30**, `contra_repeat_max_count` default **50**, gated by `enable_today_contra_repeat`; the LLM returns `[index, flag]` pairs where a non-`none` flag (`contradictory`/`contained`) sets `store_status = expired`.
4. **`update_memory_worker`** (`method: from_memory_key`, `memory_key: all`) — `MemoryManager._update_memories` dispatches by `action_status`: **new** → `batch_insert`; **content_modified** → `batch_update(update_embedding=True)`; **modified** → `batch_update(update_embedding=False)`; **delete** → `batch_delete` — `[CODE]` .../core/worker/memory_manager.py

**Reflection / re-consolidation** (`reflect_and_reconsolidate`):
1. **`load_memory_worker`** — `retrieve_not_reflected_top_k: 100`, `retrieve_not_updated_top_k: 100`, `retrieve_insight_top_k: 100`
2. **`get_reflection_subject_worker`** — `reflect_obs_cnt_threshold: 6`
3. **`update_insight_worker`** — for each insight, filter observations by relevance to `insight.key` (cosine on `key_vector` when the ranker is off, else `rank_model`), threshold `update_insight_threshold: 0.01`, keep the top `update_insight_max_count: 5`, then an LLM rewrites the insight `value` and `content` (sets `action_status = content_modified`, refreshes `timestamp`/`dt`); afterwards marks observations `obs_updated = 1`
4. **`long_contra_repeat_worker`** — `long_contra_repeat_threshold: 0.5`
5. **`store_memory`**

**Worker/operator architecture — the honest answer to the brief's guess.** There is **no** background "database server" with `ExtractMemory`/`UpdateMemory`/`ConsolidateMemory`/`ReflectMemory` operators. The real stack is:
`MemoryScopeService` (holds `memory_operations` config dict) → per-operation `FrontendOperation` / `BackendOperation` (the latter adds the timed thread loop) → `run_workflow` → ordered `*Worker` classes → `MemoryManager` (in-memory `id→MemoryNode` and `key→[ids]` registries that flush to the store) — `[CODE]` .../core/service/base_memory_service.py , .../core/operation/frontend_operation.py , .../core/operation/memory_manager.py
- The README claims *"over 20 workers"*; the archived branch contains **16 concrete worker modules** (9 backend: `contra_repeat`, `get_observation`, `get_observation_with_time`, `get_reflection_subject`, `info_filter`, `load_memory`, `long_contra_repeat`, `update_insight`, `update_memory`; 7 frontend: `extract_time`, `fuse_rerank`, `print_memory`, `read_message`, `retrieve_memory`, `semantic_rank`, `set_query`) plus base/dummy classes — from the tree listing at `https://api.github.com/repos/agentscope-ai/ReMe/git/trees/memoryscope_branch?recursive=1`. **Flag: README overstates the count by this file inventory.**

## (c) Read path

Workflow (demo_config.yaml): `retrieve_memory: set_query,[extract_time|retrieve_obs_ins,semantic_rank],fuse_rerank`

1. **`set_query_worker`** sets the query (+ timestamp).
2. **`extract_time_worker`** — LLM time-reference extraction, so *"if the query involves reference to time"* the matching-time memories are retrieved (README claim).
3. **`retrieve_memory_worker`** — `[CODE]` .../core/worker/frontend/retrieve_memory_worker.py : three concurrent retrieval tasks against the vector store, each filtered by `user_name`, `target_name`, `store_status`:
   - observations + `obs_customized` (`valid`), `retrieve_obs_top_k: 100`
   - insights (`valid`), `retrieve_ins_top_k: 100`
   - expired observations (only for the listing/`retrieve_expired_top_k` paths)
   Results are merged and **sorted by `score_recall` descending**.
4. **`semantic_rank_worker`** — `rank_model` (default `gte-rerank`, `top_n: 500`), enabled by `enable_ranker` (default **false** in demo config).
5. **`fuse_rerank_worker`** — `fuse_score_threshold: 0.01`, `fuse_time_ratio: 2.0`, and a per-type weight map: **`{conversation: 0.5, observation: 1, obs_customized: 1.2, insight: 2.0}`** (insights are boosted 4× relative to conversation).

**Trigger:** a user query hitting the frontend operation (synchronous, on the request path). Cache/refresh: `list_memory` uses `refresh_time=5`; `output_memory_max_count: 20`; `enable_ranker: false`; `enable_today_contra_repeat: true`; `enable_long_contra_repeat: false` (all demo_config.yaml).

## (d) Data structures — vector DB only (Elasticsearch); no graph DB

- **Store:** `memory_store: class: core.storage.llama_index_es_memory_store`, `index_name: memory_index`, `es_url: http://localhost:9200`, **`retrieve_mode: dense`** — demo_config.yaml. README: *"MemoryScope is equipped with a vector database (default is **ElasticSearch**) to store all memory fragments recorded in the system."*
- `[CODE]` .../core/storage/llama_index_es_memory_store.py — built on LlamaIndex `VectorStoreIndex` + a custom `SyncElasticsearchStore` with `ESCombinedRetrieveStrategy(retrieve_mode=..., hybrid_alpha=...)` (so dense is the default; hybrid is configurable). Each `MemoryNode` becomes a `TextNode(id_=memory_id, text=content, embedding=vector, text_template="{content}", metadata=<all other fields>)`; `key_vector` is pickled into metadata. Retrieval uses `similarity_top_k=top_k, sparse_top_k=top_k` with an ES filter.
- **Models (demo):** generation `gpt-4o` (`max_tokens: 2000`, `temperature: 0.01`), embedding `text-embedding-3-small`, ranker `gte-rerank` — demo_config.yaml.
- **No knowledge graph, no graph database, no relational store** in this codebase. The only "structure" beyond the vector index is the `key`/`value`/`memory_type` fields and the `obs_reflected`/`obs_updated` flags.

## (e) Benchmarks — none published for MemoryScope

**⚠️ UNVERIFIED / absent:** the archived MemoryScope branch publishes **no** benchmark numbers, no evaluation harness, and no dataset results. Searches for a MemoryScope paper returned nothing (see §0.1); the README's only citation is the software citation. The old docs site is 404, so there is no benchmark page to check.

**Successor (ReMe) numbers — vendor-published, and clearly labelled as such by the vendor:** `[README]` https://raw.githubusercontent.com/agentscope-ai/ReMe/main/README.md — *"The figures below are the published reference runs in this repository; model, prompt, dataset, and judging details are documented with each benchmark."*
| Benchmark | Setting | Sample size | Agentic score |
|---|---|---|---|
| LongMemEval cleaned-s | Overall | 500 questions | **89.4%** |
| BEAM | 100K context | 20 cases / 400 questions | 66.1% |
| BEAM | 1M context | 35 cases / 700 questions | 65.0% |
Plus: "**0.580 PROC score across five user personas**" in the repository's π-Bench evaluation, "2.4% above NanoBot under the same test-model configuration."

**Peer-reviewed successor work:** Cao, Deng, Yu, Zhou, Liu, Ding, Zhao — *"Remember Me, Refine Me: A Dynamic Procedural Memory Framework for Experience-Driven Agent Evolution"*, **Findings of ACL 2026**, pp. 16803–16822, DOI `10.18653/v1/2026.findings-acl.829` — https://aclanthology.org/2026.findings-acl.829/ . Abstract: *"Experiments on BFCL-V3 and AppWorld demonstrate that ReMe establishes a new state-of-the-art in agent memory system… Qwen3-8B equipped with ReMe outperforms larger, memoryless Qwen3-14B."* (Note: **Li Yu** appears in both the MemoryScope software citation and this paper.) There is also an arXiv preprint for the tool-memory method: https://arxiv.org/abs/2608.03403 (per the ReMe README). **These results are for ReMe, not for the archived MemoryScope code — do not attribute them to MemoryScope.**

---

# 3. A-MEM (Agentic Memory)

Paper: https://arxiv.org/abs/2502.12110 (v1 2025-02-17 → **v11 2025-10-08**; Comments: *"Advances in Neural Information Processing Systems (NeurIPS 2025)"*) · code: https://github.com/agiresearch/A-mem (production) and `WujiangXu/AgenticMemory` ≡ `WujiangXu/A-mem` (benchmark) · system repo per the paper's abstract: https://github.com/WujiangXu/A-mem-sys

## (a) Layering / data model — no tiers; a flat, self-linking Zettelkasten network

`[PAPER]` §3.1, Eq. 1 — each note is `m_i = {c_i, t_i, K_i, G_i, X_i, e_i, L_i}`:
`c_i` = original interaction content · `t_i` = timestamp · `K_i` = LLM-generated keywords · `G_i` = LLM-generated tags · `X_i` = LLM-generated contextual description · `e_i` = dense embedding · `L_i` = the set of linked memories.

`[CODE]` https://raw.githubusercontent.com/agiresearch/A-mem/main/agentic_memory/memory_system.py — `class MemoryNote` fields: `content`, `id` (uuid4), `keywords` (list), `links` (list, default `[]`), `retrieval_count`, `timestamp` + `last_accessed` (`"%Y%m%d%H%M"`), `context` (default **`"General"`**), `evolution_history` (list), `category` (default **`"Uncategorized"`**), `tags` (list). The `analyze_content` LLM prompt only generates `keywords` / `context` / `tags` — `category` is caller-supplied.

The paper's "box" concept (Fig. 2 caption): *"related memories become interconnected through their similar contextual descriptions, analogous to the Zettelkasten method. However, our approach allows individual memories to exist simultaneously within **multiple different boxes**."*

## (b) Write path — LLM note construction, then one fused link-generation + evolution LLM call

1. **Note construction** (`[PAPER]` §3.1, Eq. 2–3): `K_i, G_i, X_i ← LLM(c_i ‖ t_i ‖ P_s1)`, then `e_i = f_enc[concat(c_i, K_i, G_i, X_i)]` — the embedding covers content **plus** the LLM-generated keywords, tags and context.
2. **`add_note(content, time=None, **kwargs)`** (`[CODE]`): builds the `MemoryNote`, calls `process_memory(note)` (link generation + evolution, below), stores it in the in-process `self.memories` dict, and writes it to **ChromaDB** (`ChromaRetriever(collection_name="memories")`) together with the full metadata dict (`id, content, keywords, links, retrieval_count, timestamp, last_accessed, context, evolution_history, category, tags`).
3. **Consolidation:** `evo_threshold: int = 100` — every 100 *evolved* notes, `consolidate_memories()` **resets the ChromaDB collection and re-adds every document**. `[CODE]`, same file.
4. **Trigger:** an explicit `add_note()` call (agent adds a memory/experience). There is no automatic ingestion of raw turns inside the library.

## (c) Read path

- `[PAPER]` §3.4, Eq. 8–10: `e_q = f_enc(q)`; cosine similarity `s_{q,i}` against every note; `M_retrieved = {m_i | rank(s_{q,i}) ≤ k}`; the retrieved notes are injected as context. Trigger: **"In each interaction, our A-Mem performs context-aware memory retrieval"** — i.e. per interaction, not per query type.
- `[PAPER]` §4.2: *"we primarily employ **k = 10** for top-k memory selection to maintain computational efficiency, while adjusting this parameter for specific categories"*; embedding model **`all-minilm-l6-v2`**.
- `[PAPER]` Appendix A.5 / Table 8 — the **actual per-category k values used for the headline GPT results are much larger than 10**: GPT-4o-mini **40, 40, 50, 50, 40** and GPT-4o **40, 40, 50, 50, 40** (Multi Hop, Temporal, Open Domain, Single Hop, Adversarial); Qwen2.5-1.5b all 10; Qwen2.5-3b 10/10/50/10/10; Llama-3.2-1b all 10; Llama-3.2-3b 10/20/10/10/10. Paper text: *"For models that have already achieved state-of-the-art (SOTA) performance with k=10, we maintain this value without further tuning."* **⚠️ Implication to flag: "k=10" is the default, not the setting behind the best GPT numbers.** The eval repo agrees: `--retrieve_k` default 10 with the note "Tune this per model for best results" and a `run_k_sweep.sh`.
- `[CODE]` `search_agentic(query, k=5)`: ChromaDB vector search for top-`k`, then **link expansion** — for each hit, its `links` neighbours are appended with `is_neighbor: True`, capped at `k` results total. Returns `id, content, context, keywords, tags, timestamp, category, score`. The framework figure states it explicitly: *"When related memory is retrieved, similar memories that are linked within the same box are also automatically accessed."*
- Note the gap: paper's benchmark uses `k=10` (per §4.2) while the library default is `k=5` in `search_agentic`, `search`, and `_search`.

## (d) Data structures — vector store + in-memory dict; **no graph database**

- **ChromaDB** is the only persistent store, `collection_name="memories"`, embedding model `all-MiniLM-L6-v2` (README: *"Intelligent indexing and linking of memories via ChromaDB"* — https://raw.githubusercontent.com/agiresearch/A-mem/main/README.md).
- The "network" is **the per-note `links` list stored as ChromaDB metadata**; traversal happens in Python against `self.memories`. There is no graph DB, no relational store, and no separate tier store — which is precisely the paper's criticism of prior work (*"Mem0, following the principles of RAG, incorporates graph databases… their reliance on predefined schemas and relationships fundamentally limits their adaptability"*, §1).
- `rank_bm25` / `BM25Okapi` and `cosine_similarity` are imported in `memory_system.py`, but the retrieval paths used by `search`/`search_agentic` are ChromaDB-only in this snapshot (`_search` calls the same Chroma retriever twice and dedups — the docstring's "hybrid retrieval" claim is **not** what the code does). **⚠️ Flag as a code/docstring discrepancy.**

## (e) Link generation and memory evolution — the key novel mechanism, precisely

**Link generation** (`[PAPER]` §3.2, Eq. 4–6):
1. similarity `s_{n,j} = (e_n · e_j) / (‖e_n‖‖e_j‖)` for every existing note;
2. `M_near^n = {m_j | rank(s_{n,j}) ≤ k}` (top-k embedding neighbours = an efficiency filter, *"without exhaustive comparison"*);
3. `L_i ← LLM(m_n ‖ M_near^n ‖ P_s2)` — **the LLM, not the embedding, decides which links are real**: *"The LLM-driven analysis allows for nuanced understanding of relationships that goes beyond simple similarity metrics. The language model can identify subtle patterns, causal relationships, and conceptual connections that might not be apparent from embedding similarity alone."*

**Memory evolution** (`[PAPER]` §3.3, Eq. 7) — the mechanism that distinguishes A-MEM:
> `m_j* ← LLM(m_n ‖ M_near^n \ {m_j} ‖ m_j ‖ P_s3)`
> "For each memory `m_j` in the nearest neighbor set `M_near^n`, the system determines **whether to update its context, keywords, and tags**. … The evolved memory `m_j*` then **replaces the original memory `m_j`** in the memory set `M`. This evolutionary approach enables continuous updates and new connections… As the system processes more memories over time, it develops increasingly sophisticated knowledge structures, discovering higher-order patterns and concepts across multiple memories."

So: **yes, entirely LLM-driven**, and the write is **bidirectional** — inserting note `n` rewrites the `context` and `tags` of its existing neighbours.

**Implementation detail (important, and stricter/looser than the paper):** in `memory_system.py` link generation and evolution are **one fused LLM call** driven by `_evolution_system_prompt` (k=5 neighbours via `find_related_memories(note.content, k=5)`), returning JSON with `should_evolve`, `actions ∈ ["strengthen", "update_neighbor"]`, `suggested_connections`, `tags_to_update`, `new_context_neighborhood`, `new_tags_neighborhood`. The prompt requires *"the length of new_tags_neighborhood must equal the number of input neighbors, and the length of new_context_neighborhood must equal the number of input neighbors"*. Execution (`[CODE]`):
- `strengthen` → `note.links.extend(suggested_connections)`, `note.tags = tags_to_update` (the **new** note's tags);
- `update_neighbor` → for each neighbour `i`: `notetmp.tags = new_tags_neighborhood[i]`, `notetmp.context = new_context_neighborhood[i]` — i.e. **only `context` and `tags` are rewritten; `content` is never touched, and `keywords` are not updated by the evolution path** (the paper says "context, keywords, and tags").
- Observations to flag: (i) `evolution_history` exists as a field but is **never appended to** in this snapshot; (ii) evolved neighbours are written back to `self.memories` but their **ChromaDB metadata is not re-indexed at evolution time** — the updated `context`/`tags` only reach the index on the periodic `consolidate_memories()` or an explicit `update()`; (iii) the neighbour-index bookkeeping mixes result-list indices with global memory list indices (`indices[i]` is a rank position, used to index `noteslist`), which looks fragile. **These are code-level observations from the current `main`, not documented behaviour.**

## (f) Benchmarks — **paper-reported** (NeurIPS 2025; the strongest evidence tier of the three systems)

**Datasets** (`[PAPER]` §4.1): LoCoMo — *"averaging 9K tokens spanning up to 35 sessions"*, five question types (single-hop, multi-hop, temporal, open-domain, adversarial), *"**In total, LoCoMo contains 7,512 question-answer pairs**"* (no per-category breakdown is given — **⚠️ UNVERIFIED**). DialSim — *"covering 1,300 sessions spanning five years, containing approximately 350,000 tokens, and including more than 1,000 questions per session"*.

**Table 1 (LoCoMo), F1 / BLEU-1 %.** Column order as printed in v11: **Multi Hop | Temporal | Open Domain | Single Hop | Adversarial**, then an "Average" block that contains **only Ranking and Token Length — there is NO average F1/BLEU column** (**⚠️ flag**: this is a common misreading; v1 prints the same values under a differently-ordered header).

| Model | Method | Multi Hop | Temporal | Open Domain | Single Hop | Adversarial | Avg Rank | Avg Tokens |
|---|---|---|---|---|---|---|---|---|
| GPT-4o-mini | LoCoMo | 25.02/19.75 | 18.41/14.77 | 12.04/11.16 | 40.36/29.05 | 69.23/68.75 | 2.4 | 16,910 |
| GPT-4o-mini | MemGPT | 26.65/17.72 | 25.52/19.44 | 9.15/7.44 | 41.04/34.34 | 43.29/42.73 | 2.4 | 16,977 |
| GPT-4o-mini | MemoryBank | 5.00/4.77 | 9.68/6.99 | 5.56/5.94 | 6.61/5.16 | 7.36/6.48 | 4.8 | 432 |
| GPT-4o-mini | ReadAgent | 9.15/6.48 | 12.60/8.87 | 5.31/5.12 | 9.67/7.66 | 9.81/9.02 | 4.2 | 643 |
| GPT-4o-mini | **A-Mem** | **27.02/20.09** | **45.85/36.67** | 12.14/12.00 | **44.65/37.06** | 50.03/49.47 | **1.2** | **2,520** |
| GPT-4o | LoCoMo | 28.00/18.47 | 9.09/5.78 | 16.47/14.80 | 61.56/54.19 | 52.61/51.13 | 2.0 | 16,910 |
| GPT-4o | MemGPT | 30.36/22.83 | 17.29/13.18 | 12.24/11.87 | 60.16/53.35 | 34.96/34.25 | 2.4 | 16,987 |
| GPT-4o | **A-Mem** | **32.86/23.76** | **39.41/31.23** | **17.10/15.84** | 48.43/42.97 | 36.35/35.53 | **1.6** | **1,216** |
| Qwen2.5-1.5b | **A-Mem** | 18.23/11.94 | 24.32/19.74 | 16.48/14.31 | 23.63/19.23 | 46.00/43.26 | **1.0** | 1,300 |
| Qwen2.5-3b | **A-Mem** | 12.57/9.01 | 27.59/25.07 | 7.12/7.28 | 17.23/13.12 | 27.91/25.15 | **1.0** | 1,137 |
| Llama 3.2 1b | **A-Mem** | 19.06/11.71 | 17.80/10.28 | 17.55/14.67 | 28.51/24.13 | 58.81/54.28 | **1.0** | 1,376 |
| Llama 3.2 3b | **A-Mem** | 17.44/11.74 | 26.38/19.50 | 12.53/11.83 | 28.14/23.87 | 42.04/40.60 | **1.0** | 1,126 |

(All six headline models' full baseline rows were extracted from https://arxiv.org/html/2502.12110v11 . The eval repo's category-id mapping is 1 Multi-hop, 2 Temporal, 3 Open-domain, 4 Single-hop, 5 Adversarial, dataset file `data/locomo10.json` — https://raw.githubusercontent.com/WujiangXu/AgenticMemory/main/README.md .)

**Appendix A.3 Table 7 — three further models** (LoCoMo / MemGPT / A-Mem, F1/BLEU-1; these appear only in the appendix although the abstract says "six foundation models"):

| Model | LoCoMo | MemGPT | A-Mem |
|---|---|---|---|
| DeepSeek-R1-32B | 8.58/6.48 \| 4.79/4.35 \| 12.96/12.52 \| 10.72/8.20 \| 21.40/20.23 | 8.28/6.25 \| 5.45/4.97 \| 10.97/9.09 \| 11.34/9.03 \| 30.77/29.23 | **15.02/10.64 \| 14.64/11.01 \| 14.81/12.82 \| 15.37/12.30 \| 27.92/27.19** |
| Claude 3.0 Haiku | 4.56/3.33 \| 0.82/0.59 \| 2.86/3.22 \| 3.56/3.24 \| 3.46/3.42 | 7.65/6.36 \| 1.65/1.26 \| 7.41/6.64 \| 8.60/7.29 \| 7.66/7.37 | **19.28/14.69 \| 16.65/12.23 \| 11.85/9.61 \| 34.72/30.05 \| 35.99/34.87** |
| Claude 3.5 Haiku | 11.34/8.21 \| 3.29/2.69 \| 3.79/3.58 \| 14.01/12.57 \| 7.37/7.12 | 8.27/6.55 \| 3.99/2.76 \| 4.71/4.48 \| 16.52/14.89 \| 5.64/5.45 | **29.70/23.19 \| 31.54/27.53 \| 11.42/9.47 \| 42.60/37.41 \| 13.65/12.71** |

(⚠️ Appendix A.3's body text contains the typo "Qwen2.5-**15b**" for what must be 1.5b.)

**DialSim** (Table 2): A-Mem **3.45 F1 / 3.37 BLEU-1 / 3.54 ROUGE-L / 3.60 ROUGE-2 / 2.05 METEOR / 19.51 SBERT** vs LoCoMo 2.55/3.13/2.75/0.90/1.64/15.76 and MemGPT 1.18/1.07/0.96/0.42/0.95/8.54. §4.3: *"achieving an F1 score of 3.45 (a 35% improvement over LoCoMo's 2.55 and 192% higher than MemGPT's 1.18)"*.

**Cost/latency** (§4.3): *"approximately 1,200 tokens per memory operation, achieving an 85-93% reduction … (LoCoMo and MemGPT with 16,900 tokens)"*; *"less than $0.0003"*; *"Processing times average 5.4 seconds using GPT-4o-mini and only 1.1 seconds with locally-hosted Llama 3.2 1B"*.

**Ablation** (§4.4, Table 3, GPT-4o-mini, F1/BLEU-1) — quantifies link generation (LG) and memory evolution (ME):

| Method | Multi Hop | Temporal | Open Domain | Single Hop | Adversarial |
|---|---|---|---|---|---|
| w/o LG & ME | 9.65/7.09 | 24.55/19.48 | 7.77/6.70 | 13.28/10.30 | 15.32/18.02 |
| w/o ME (LG only) | 21.35/15.13 | 31.24/27.31 | 10.13/10.85 | 39.17/34.70 | 44.16/45.33 |
| A-Mem (full) | 27.02/20.09 | 45.85/36.67 | 12.14/12.00 | 44.65/37.06 | 50.03/49.47 |

**Scaling** (Table 4, retrieval latency): A-Mem 0.31 ± 0.30 µs @1k memories → 3.70 ± 0.74 µs @1M; MemoryBank 0.24 → 1.91 µs; ReadAgent 43.62 → 120,069.68 µs.

**Peer-review status: peer-reviewed** — NeurIPS 2025 per the arXiv Comments field (https://arxiv.org/abs/2502.12110) and https://mlanthology.org/neurips/2025/xu2025neurips-amem/ . **⚠️ UNVERIFIED:** acceptance type; and note the abstract's own claim of *"six foundation models"* is a **paper-reported** summary whose supported numbers live partly in appendix Table 7.

**Caveats found in the artifact itself (worth a footnote in the report):** (a) no average F1/BLEU column exists in Table 1; (b) v1 and v11 print the same values under different category orderings; (c) the Llama-3.2-3b ReadAgent row in Table 1 duplicates the Qwen2.5-3b ReadAgent metrics exactly (only token length differs) — looks like a duplicated row; (d) Appendix A.3 says "Qwen2.5-15b" (typo).

---

# 4. Cross-system summary

| | **Cognee** | **MemoryScope** | **A-MEM** |
|---|---|---|---|
| Organization | Three-store pipeline, no context layer | Frontend/backend-decoupled tiers: `conversation → observation → insight` (+ `obs_customized`) | Flat, self-linking Zettelkasten network; no tiers |
| Write trigger | App calls `remember()` / `cognify()`; background `improve()` for session bridging | Timed backend ops (`consolidate_memory` every 1s-loop tick; `reflect_and_reconsolidate` every 15) + message `memorized` flag | App calls `add_note()`; every 100 evolutions → ChromaDB reindex |
| Read trigger | Explicit `recall()`; rule-based 2-rule router → `HYBRID_COMPLETION` default | User query → frontend `retrieve_memory` workflow (reported ~500 ms) | Every interaction; cosine top-k + link expansion |
| Structures | Graph (**Kuzu** default → Neo4j/Neptune; Postgres demo) + vector (**LanceDB** default → PGVector/…) + relational (SQLite/Postgres) | **Elasticsearch** vector only (`retrieve_mode: dense`) | **ChromaDB** vector only + in-process dict |
| Evolution | `improve()` 9 stages + `memify` enrichment; contradictions as `contradicts` edges | Periodic contradiction/repetition expiry + LLM insight rewriting | **LLM rewrites neighbours' `context`/`tags` on insert** (bidirectional) |
| Strongest evidence | Vendor-run BEAM 0.79 / 0.67 (README, self-caveated) | **No published benchmarks** | **NeurIPS 2025 + LoCoMo/DialSim + ablation** |
