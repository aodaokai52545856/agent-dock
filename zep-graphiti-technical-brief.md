# Zep / Graphiti — Temporal Knowledge Graph Agent Memory: Verifiable Technical Brief

**Research snapshot.** Compiled from primary sources only: the Zep paper (arXiv:2501.13956v1, submitted 20 Jan 2025), Graphiti source on `main` (README last substantially changed 2026-08-26), the v3 Zep docs at `help.getzep.com`, and Zep's own blog/research pages. Every factual claim below carries a URL. Numbers in §6 are **vendor self-reported** and are labelled as such.

**Source map**

| ID | Source | URL |
|---|---|---|
| **P** | Zep paper, v1, 20 Jan 2025 | https://arxiv.org/html/2501.13956v1 |
| **C** | Graphiti source, `main` | https://github.com/getzep/graphiti |
| **RD** | Graphiti README (`main`) | https://raw.githubusercontent.com/getzep/graphiti/main/README.md |
| **GD** | Graphiti docs | https://help.getzep.com/graphiti/llms.txt |
| **ZD** | Zep v3 product docs | https://help.getzep.com/llms.txt |
| **B1** | Zep blog, "State of the Art in Agent Memory", 22 Jan 2025 | https://blog.getzep.com/state-of-the-art-agent-memory/ |
| **B2** | Zep blog, "Lies, Damn Lies, & Statistics", 6 May 2025 (upd. 3 Jun 2026) | https://blog.getzep.com/lies-damn-lies-statistics-is-mem0-really-sota-in-agent-memory/ |
| **B3** | Zep blog, "The Retrieval Tradeoff", 9 Dec 2025 (upd. 3 Jun 2026) | https://blog.getzep.com/the-retrieval-tradeoff-what-50-experiments-taught-us-about-context-engineering/ |
| **R** | Zep `/research` benchmark page (current, 2026) | https://www.getzep.com/research/ |
| **M** | Zep `/mem0-alternative` page (current, 2026) | https://www.getzep.com/mem0-alternative/ |
| **I5** | Mem0 rebuttal issue #5 on `getzep/zep-papers` | https://github.com/getzep/zep-papers/issues/5 |

---

## 1. Graph schema / layering

### 1.1 The three tiers (paper, verbatim structure)

The graph is `G = (N, E, φ)` with `φ: E → N × N`, comprising **three hierarchical tiers of subgraphs** (P §2):

| Tier | Nodes | Edges | What is stored |
|---|---|---|---|
| **Episode subgraph `G_e`** | Episodic nodes `n_i ∈ N_e` | `e_i ∈ E_e ⊆ φ*(N_e × N_s)` — connect episodes to their referenced semantic entities | "raw input data in the form of messages, text, or JSON"; "a **non-lossy** data store from which semantic entities and relations are extracted" |
| **Semantic entity subgraph `G_s`** | Entity nodes `n_i ∈ N_s` | `e_i ∈ E_s ⊆ φ*(N_s × N_s)` — relationships between entities | entities "extracted from episodes and **resolved with existing graph entities**"; relationships with facts + four timestamps |
| **Community subgraph `G_c`** | Community nodes `n_i ∈ N_c` | `e_i ∈ E_c ⊆ φ*(N_c × N_s)` — communities → member entities | "**clusters of strongly connected entities**"; "**high-level summarizations** of these clusters" |

Paper's framing: this is claimed to mirror **episodic vs. semantic memory** in psychology (P §2), and the hierarchy "from episodes to facts to entities to communities" is said to extend hierarchical RAG strategies. Community nodes build on GraphRAG (P §2).

### 1.2 Physical schema (code)

Edge types are concrete and Cypher-indexed in the source:

- `(:Episodic)-[:MENTIONS]->(:Entity)` — episodic edges (C `graphiti_core/nodes.py`, `graphiti_core/edges.py`)
- `(:Entity)-[:RELATES_TO]->(:Entity)` — semantic edges
- `(:Community)-[:HAS_MEMBER]->(:Entity)` — community edges
- **Not in the paper (added later):** `(:Saga)-[:HAS_EPISODE]->(:Episodic)` and `(:Episodic)-[:NEXT_EPISODE]->(:Episodic)` — a "saga" is a running, incrementally-summarized episode thread (C `graphiti_core/nodes.py` class `SagaNode`; C `graphiti_core/graphiti.py` `summarize_saga`)

**Node properties** (C `graphiti_core/nodes.py`):

- `EpisodicNode`: `uuid, name, group_id, source` (EpisodeType), `source_description, content, created_at, valid_at, entity_edges[], episode_metadata`
- `EntityNode`: `uuid, name, group_id, labels[], name_embedding, summary, created_at, attributes{}`
- `CommunityNode`: `uuid, name, group_id, name_embedding, summary, created_at`

**Edge properties** (C `graphiti_core/edges.py`, `class EntityEdge`):

`uuid, group_id, source_node_uuid, target_node_uuid, created_at, name, fact, fact_embedding, episodes[], expired_at, valid_at, invalid_at, reference_time, attributes{}`

Note: the paper says Graphiti models "complex multi-entity facts through an implementation of **hyper-edges**" (P §2.2.2); in the code this is realised simply as *the same fact being extractable multiple times between different entity pairs* — i.e. a set of pairwise edges, not a native hyperedge type.

**Fulltext indexes** (C `graphiti_core/graph_queries.py`): `node_name_and_summary` on `Entity(name, summary, group_id)`; `community_name` on `Community(name, group_id)`; `episode_content` on `Episodic(content, source, source_description, group_id)`; `edge_name_and_fact` on `RELATES_TO(name, fact, group_id)`. Range indexes exist on `RELATES_TO(created_at, expired_at, valid_at, invalid_at)`.

`group_id` is the graph partition/namespace (RD "Graph Namespacing"; C `graphiti_core/helpers.py` `validate_group_ids`).

### 1.3 Episode types

- **Paper (P §2.1):** "Episodes can be one of **three core types: message, text, or JSON**." A `message` = "relatively short text … along with the associated actor who produced the utterance", plus a reference timestamp `t_ref`.
- **Code (C `graphiti_core/nodes.py`, `class EpisodeType`):** **four** values — `message`, `json`, `text`, and **`fact_triple`**. `message` content format is `"actor: content"`.
- **Graphiti docs (GD `core-concepts/adding-episodes.md`):** still lists only three — `text`, `message`, `json`. `json` "processed distinctly from the other types".
- **Zep product docs (ZD `/episodes`):** `message` (via `thread.add_messages`), `text` and `json` (via `graph.add`).

⚑ **Discrepancy:** the paper and current docs say three episode types; the shipped enum has a fourth (`fact_triple`). Docs are stale relative to code.

### 1.4 Communities — how they are built and what they are for

**Paper (P §2.3), explicit:**
> "While our community detection approach builds upon the technique described in GraphRAG, we employ a **label propagation algorithm** rather than the **Leiden algorithm**. This choice was influenced by label propagation's straightforward dynamic extension…"

Dynamic extension = "the logic of a **single recursive step** in label propagation": a new entity node is assigned to "the community held by the **plurality of its neighbors**", then the community summary is updated. The paper concedes "the resulting communities gradually diverge from those that would be generated by a complete label propagation run. Therefore, **periodic community refreshes remain necessary**."

Community content: "summaries derived through an **iterative map-reduce-style summarization** of member nodes" (per GraphRAG), plus a **name** containing key terms/subjects, which "are embedded and stored to enable cosine similarity searches".

**Code (C `graphiti_core/utils/maintenance/community_operations.py`), exact:**
- `label_propagation(projection)` — comments state the 4 steps: each node starts in its own community; each takes the community of the plurality of its neighbors (weighted by `edge_count`); **ties broken by going to the largest community**; iterate "until no communities change".
- `build_community()` — repeatedly pairs member summaries and asks the LLM to summarise each pair (`summarize_pair`) until one summary remains; then `generate_summary_description()` produces the community **name**; `truncate_at_sentence(..., MAX_SUMMARY_CHARS)`.
- `build_communities()` — `MAX_COMMUNITY_BUILD_CONCURRENCY = 10`.
- `determine_entity_community()` / `update_community()` — the incremental path: if the node has no community, assign it to the **mode** community of surrounding entities; then re-summarise `(entity.summary, community.summary)` and regenerate the name/embedding.
- `Graphiti.build_communities()` (C `graphiti_core/graphiti.py`) calls `remove_communities(...)` first — i.e. **a refresh is a full delete-and-rebuild of communities for the group(s)**; scoped only to the groups being rebuilt.

**Docs (GD `core-concepts/communities.md`):**
> "Communities are determined using the **Leiden algorithm**, which groups strongly connected nodes together."

…and in the same page: "This updating methodology is inspired by the **label propagation algorithm**."

⚑ **Discrepancy (documentation bug):** the docs' "Leiden algorithm" sentence contradicts both the paper and the shipped code (which implements label propagation). Treat the code + paper as authoritative.

**What communities are used for:** one of the three retrieved object types in the retrieval 3-tuple (P §3) — high-level synthesized context; searched by **community name** via cosine similarity and BM25 (GD `core-concepts/communities.md`; C `search_config.py`).

---

## 2. Bi-temporal model

### 2.1 The two timelines and four timestamps

**Paper (P §2.1, §2.2.3):** timeline `T` = "the chronological ordering of events"; timeline `T'` = "the transactional order of Zep's data ingestion". Four timestamps are tracked and **stored on edges**:

- `t'_created`, `t'_expired ∈ T'` — "monitor when facts are created or invalidated **in the system**"
- `t_valid`, `t_invalid ∈ T` — "track the temporal range during which facts **held true**"

**Zep product docs (ZD `/facts`, "The four fact timestamps") — exact semantics, using marriage/divorce:**

| Edge attribute | Meaning | Example given |
|---|---|---|
| `created_at` | when Zep *learned* it | "The time Zep learned that the user got married" |
| `valid_at` | when it became true in the world | "The time the user got married" |
| `invalid_at` | when it stopped being true in the world | "The time the user got divorced" |
| `expired_at` | when Zep *learned* it stopped being true | "The time Zep learned that the user got divorced" |

The docs add the caveat that these are derived bounds and "do not establish that the source content or the derived fact is true". Only `valid_at`/`invalid_at` are rendered into the Context Block, as `FACT (Date range: from - to)`.

### 2.2 Edge invalidation, not deletion

**Paper (P §2.2.3):** "The introduction of new edges can invalidate existing edges in the database. The system employs an **LLM to compare new edges against semantically related existing edges** to identify potential contradictions. When the system identifies **temporally overlapping contradictions**, it invalidates the affected edges by setting their `t_invalid` to the `t_valid` of the invalidating edge. Following the transactional timeline `T'`, **Graphiti consistently prioritizes new information** when determining edge invalidation."

**Code (C `graphiti_core/utils/maintenance/edge_operations.py`) — the exact algorithm:**

`resolve_edge_contradictions(resolved_edge, invalidation_candidates)`:
1. Skip a candidate if `(edge.invalid_at <= resolved_edge.valid_at)` or `(resolved_edge.invalid_at <= edge.valid_at)` — i.e. no temporal overlap.
2. Otherwise, if `edge.valid_at < resolved_edge.valid_at`: set `edge.invalid_at = resolved_edge.valid_at` and, if unset, `edge.expired_at = utc_now()`.

The **reverse** also happens: after resolution, candidates are sorted by `valid_at` and if any candidate's `valid_at > resolved_edge.valid_at`, the **new** edge is expired instead (`resolved_edge.invalid_at = candidate.valid_at; resolved_edge.expired_at = now`).

Contradictions are selected by an LLM prompt (`prompt_library.dedupe_edges.resolve_edge`, response model `EdgeDuplicate`) that returns **two index lists into one contiguous index space**: `duplicate_facts` (indices into `EXISTING FACTS`, i.e. same-endpoint edges) and `contradicted_facts` (indices spanning `EXISTING FACTS` then `FACT INVALIDATION CANDIDATES`, offset by `len(related_edges)`). Both LLM lists are range-validated with warnings on out-of-range indices.

Edges are never hard-deleted by ingestion — the invalidated edge remains in the graph with `invalid_at`/`expired_at` set and is persisted alongside the new edges: `entity_edges = resolved_edges + invalidated_edges` (C `graphiti_core/graphiti.py`, `add_episode`).

### 2.3 Point-in-time querying

`SearchFilters` (C `graphiti_core/search/search_filters.py`) exposes four date-filter fields: `valid_at`, `invalid_at`, `created_at`, `expired_at`, each typed `list[list[DateFilter]]` with operators `= <> > < >= <= IS NULL IS NOT NULL`. The **outer list is OR, the inner list is AND** (ZD `/searching-the-graph` "Logic Behavior" — the doc's worked example is `(created >= 2025-07-01 AND created < 2025-08-01) OR (created < 2025-05-01)`). Datetime filters "only apply to edge scope searches" (ZD).

Zep's docs recommend `invalid_at IS NULL` to find currently-valid facts.

⚑ `Graphiti.add_episode_bulk` docstring (C `graphiti_core/graphiti.py`) states "Edge invalidation and date extraction (`valid_at` / `invalid_at`) **are performed** in the bulk path as well", while the Graphiti docs (GD `core-concepts/adding-episodes.md`) warn: "The bulk ingestion pipeline **does not** perform edge invalidation operations." **Sources conflict**; the code path does call `resolve_extracted_edges`, but I did not execute it to settle behaviour empirically.

---

## 3. Write / ingestion path

Exact order from `Graphiti.add_episode` (C `graphiti_core/graphiti.py`) and the maintenance modules:

1. **Retrieve previous-episode context.** `retrieve_episodes(reference_time, last_n=RELEVANT_SCHEMA_LIMIT, ...)` with `RELEVANT_SCHEMA_LIMIT = 10` (C `search_utils.py`). The lower-level default is `EPISODE_WINDOW_LEN = 3` (C `graph_data_operations.py`), but `add_episode` explicitly passes 10. ⚑ **The paper says `n = 4`** ("the system processes both the current message content and the last n messages… For this paper and in Zep's general implementation, n=4", P §2.2.1). Paper (4) vs. code (10) **discrepancy**; the 4 is a paper-era figure.
2. **Create the `EpisodicNode`** with `created_at = utc_now()`, `valid_at = reference_time` (the episode's `t_ref`).
3. **Entity extraction (LLM).** `extract_nodes()` (C `node_operations.py`). Prompt is selected by episode type: `extract_nodes.extract_message` / `extract_text` / `extract_json`. The extraction prompt's Guideline 1 (P App. 6.1.1) is "**ALWAYS extract the speaker/actor as the first node**"; the paper adds that "the speaker is automatically extracted as an entity" and that a **reflection** technique (inspired by Reflexion) is used "to minimize hallucinations and enhance extraction coverage". Empty names are filtered; exact-normalised-name duplicates within one extraction are collapsed (`_collapse_exact_duplicate_extracted_nodes`, keeping the more specific label).
4. **Entity resolution / deduplication** — `resolve_extracted_nodes()`:
   - **Candidate search (semantic):** each extracted name is embedded (`embedder.create_batch`) and passed to `node_similarity_search(..., limit, min_score)` with **`NODE_DEDUP_CANDIDATE_LIMIT = 15`** and **`NODE_DEDUP_COSINE_MIN_SCORE = 0.6`** (both constants in C `node_operations.py`).
     - ⚑ **Paper vs. code:** the paper (P §2.2.1) describes candidate generation as cosine similarity **plus** "a separate full-text search on existing entity names and summaries". The current implementation's `_semantic_candidate_search` docstring says "Run **direct cosine similarity search** per extracted node **without reranking**" — no BM25 leg. **Discrepancy.**
   - **Deterministic resolution first** (C `utils/maintenance/dedup_helpers.py`, `_resolve_with_similarity`):
     - **Exact normalised-name match — always attempted**, regardless of length/entropy. 1 hit → resolve; >1 hit (ambiguous) → escalate to LLM.
     - **Entropy gate** on the fuzzy path only: `_NAME_ENTROPY_THRESHOLD = 1.5`, `_MIN_NAME_LENGTH = 6`, `_MIN_TOKEN_COUNT = 2` (Shannon entropy over characters; short/repetitive names are "unreliable for fuzzy matching" and go to the LLM).
     - **Fuzzy match:** 3-gram shingles → MinHash with `_MINHASH_PERMUTATIONS = 32` → LSH bands of `_MINHASH_BAND_SIZE = 4` → Jaccard similarity, accepted if `>= _FUZZY_JACCARD_THRESHOLD = 0.9`.
     - A resolved node inherits a more specific label than bare `Entity` via `_promote_resolved_node`.
   - **LLM escalation** for everything unresolved: prompt `dedupe_nodes.nodes` with numbered extracted nodes and numbered existing candidates; the model returns a `duplicate_candidate_id` per node (`-1` = not a duplicate). Malformed/duplicate/out-of-range IDs are defensively ignored with warnings.
5. **Edge extraction (LLM).** `extract_edges()` (C `edge_operations.py`), prompt `extract_edges.edge`, `response_model=ExtractedEdges`, **`extract_edges_max_tokens = 16384`**. Guideline (P App. 6.1.3): extract facts **only between the provided entities**; `relation_type` "should be a concise, all-caps description" (e.g. `LOVES`, `IS_FRIENDS_WITH`, `WORKS_FOR`). Returned entity names are validated against the node list; self-edges are dropped.
6. **Fact embeddings** — `create_entity_edge_embeddings(embedder, extracted_edges)`.
7. **Edge deduplication candidates restricted to the same entity pair.** `EntityEdge.get_between_nodes(driver, source_uuid, target_uuid)` for each new edge, then `search(..., config=EDGE_HYBRID_SEARCH_RRF, search_filter=SearchFilters(edge_uuids=[...]))`. This matches the paper exactly: "The hybrid search for relevant edges is **constrained to edges existing between the same entity pairs** as the proposed new edge… [which] significantly reduces the computational complexity" (P §2.2.2).
8. **Invalidation candidates** — a second, **unconstrained** `search(..., EDGE_HYBRID_SEARCH_RRF, SearchFilters())`; candidates that also appear in the duplicate set are removed (kept only as duplicates).
9. **Edge resolution (LLM).** `resolve_extracted_edge()`:
   - **Fast path:** if fact text and endpoints already exist verbatim (normalised), reuse the existing edge and append the episode UUID — no LLM call.
   - Otherwise prompt `dedupe_edges.resolve_edge` returns `duplicate_facts` + `contradicted_facts` (see §2.2). The paper's Fact Resolution prompt (P App. 6.1.4) states: "The facts do not need to be completely identical to be duplicates, they just need to express the same information."
10. **Temporal extraction (LLM).** `_extract_edge_timestamps()` — small model, prompt `extract_edges.extract_timestamps`, context `{fact, reference_time}`. Skipped if `valid_at`/`invalid_at` are already set (the separate-extraction path sets them in the extraction prompt). P App. 6.1.5 rules: ISO-8601 with `Z`; "If the fact is written in the present tense, use the Reference Timestamp for the `valid_at` date"; "Do not infer dates from related events"; date-only → `00:00:00`; year-only → Jan 1.
11. **Invalidation** — `resolve_edge_contradictions()` (see §2.2), plus possible expiry of the new edge.
12. **Node attributes + summaries (LLM).** `extract_attributes_from_nodes()`: per-entity attribute extraction (small model, only when a typed model applies, `merge_mode='overlay'`), then batched summarisation in "flights" of **`MAX_NODES = 30`**, truncated at `MAX_SUMMARY_CHARS`; short summaries get edge facts appended directly **without** an LLM call. Node name embeddings created at the end.
13. **Persist.** `_process_episode_data()` saves the episode, the episodic `MENTIONS` edges (respecting the per-node `node_episode_index_map`), and `resolved_edges + invalidated_edges`.
14. **Optional community update.** `update_communities: bool = False` — **off by default**. When on, `update_community()` is invoked per node (mode-of-neighbours assignment + re-summarisation).

**Why Cypher, not an LLM:** "the system incorporates the data into the knowledge graph using **predefined Cypher queries**. We chose this approach over LLM-generated database queries to ensure consistent schema formats and reduce the potential for hallucinations" (P §2.2.1).

**Concurrency:** `SEMAPHORE_LIMIT` env var, **default `10`** (RD "Default to Low Concurrency"); the MCP server README gives per-tier tuning guidance (e.g. OpenAI Tier 3 500 RPM → `SEMAPHORE_LIMIT=10–15`).

**Embeddings:** the paper's experiments use **BGE-m3**, embedding entity names into a **1024-dimensional** space (P §2.2.1, §4.1). The code default matches: `EMBEDDING_DIM = int(os.getenv('EMBEDDING_DIM', 1024))` (C `graphiti_core/embedder/client.py`).

---

## 4. Retrieval path

### 4.1 The three search methods — confirmed

P §3.1: Zep implements exactly three search functions —
- **`φ_cos`** — cosine semantic similarity search
- **`φ_bm25`** — "**Okapi BM25** full-text search"
- **`φ_bfs`** — breadth-first search

"The first two functions utilize **Neo4j's implementation of Lucene**." Search fields differ per object type: for `E_s` the **`fact`** field; for `N_s` the **entity name**; for `N_c` the **community name**. BFS is described as accepting nodes as parameters, "particularly valuable when using **recent episodes as seeds**".

Code confirms the enums (C `graphiti_core/search/search_config.py`):
```python
class EdgeSearchMethod(Enum):   cosine_similarity / bm25 / bfs = 'breadth_first_search'
class NodeSearchMethod(Enum):   cosine_similarity / bm25 / bfs = 'breadth_first_search'
class EpisodeSearchMethod(Enum): bm25                      # only BM25 — no vector search
class CommunitySearchMethod(Enum): cosine_similarity / bm25
```

### 4.2 How they are combined — **all of RRF, MMR and cross-encoder reranking exist**; which one runs depends on the recipe

The paper (P §3.2): "Zep supports existing reranking approaches such as **Reciprocal Rank Fusion (RRF)** and **Maximal Marginal Relevance (MMR)**. Additionally, Zep implements a graph-based **episode-mentions reranker**… The system also includes a **node distance reranker**… The system's most sophisticated reranking capability employs **cross-encoders**… though this approach incurs the highest computational cost."

Code reranker enum values (C `search_config.py`): `rrf = 'reciprocal_rank_fusion'`, `node_distance`, `episode_mentions`, `mmr`, `cross_encoder`. `CommunityReranker` has **no** `node_distance`/`episode_mentions`; `EpisodeReranker` has only `rrf` and `cross_encoder`.

**Pipelines actually executed** (C `graphiti_core/search/search.py`):

- Each configured search method is run **in parallel** (`semaphore_gather`), each fetching **`2 * limit`** candidates.
- `rrf` reranker → `rrf([[uuids per method]], min_score=reranker_min_score)`; **episode_mentions** also seeds with RRF, then `episode_mentions_reranker` sorts by `count(*)` of `(:Episodic)-[:MENTIONS]->(n)` descending.
- `mmr` reranker → loads embeddings, then `maximal_marginal_relevance(query_vector, candidates, mmr_lambda, ...)`: `mmr = mmr_lambda * dot(q, d) + (mmr_lambda - 1) * max_sim(d, others)`.
- `cross_encoder` reranker → **RRF first as a recall stage**, then truncate to `2*limit`, then `cross_encoder.rank(query, passages)`.
- `node_distance` reranker → **RRF first**, then `node_distance_reranker(...)`; raises `SearchRerankerError` if `center_node_uuid` is None.
- **BFS auto-seeding:** if `bfs` is in `search_methods` and `bfs_origin_node_uuids` is None, the code derives origin node UUIDs from the results of the other methods and runs BFS again.

**RRF formula in code** (C `search_utils.py`): `scores[uuid] += 1 / (i + rank_const)` with `rank_const=1` default ⇒ `1/(rank+1)`. (The docs describe it more loosely as "converted to reciprocal scores (**1/rank**) and summed" — GD `working-with-data/searching.md`.)

**Cross-encoder implementation** (C `graphiti_core/cross_encoder/openai_reranker_client.py`): `OpenAIRerankerClient` with `DEFAULT_MODEL = 'gpt-4.1-nano'`. It asks the model to answer `"True"`/`"False"` for relevance with `temperature=0`, `max_tokens=1`, `logit_bias={'6432': 1, '7983': 1}`, `logprobs=True, top_logprobs=2`, and converts the top logprob into a score. Alternatives per GD: `GeminiRerankerClient` (default `gemini-2.5-flash-lite`) and `BGERerankerClient` (`BAAI/bge-reranker-v2-m3`). ⚑ Note this is an **LLM-as-classifier**, not a true cross-encoder architecture — despite the paper describing cross-encoders as "LLMs that generate relevance scores by evaluating nodes and edges against queries using **cross-attention**".

### 4.3 Exact parameter names and defaults

**graphiti-core (`SearchConfig`), C `search_config.py` + `search_utils.py`:**

| Parameter | Scope | Default |
|---|---|---|
| `search_methods` | per-scope (`edge_config`, `node_config`, `episode_config`, `community_config`) | required (no default) |
| `reranker` | per-scope | `rrf` (`EdgeReranker.rrf` etc.) |
| `sim_min_score` | per-scope | **`DEFAULT_MIN_SCORE = 0.6`** |
| `mmr_lambda` | per-scope | **`DEFAULT_MMR_LAMBDA = 0.5`** (the `COMBINED_HYBRID_SEARCH_MMR` recipe overrides it to **`1`**) |
| `bfs_max_depth` | per-scope | **`MAX_SEARCH_DEPTH = 3`** |
| `limit` | `SearchConfig` | **`DEFAULT_SEARCH_LIMIT = 10`** |
| `reranker_min_score` | `SearchConfig` | `0` |

Other constants (C `search_utils.py`): `RELEVANT_SCHEMA_LIMIT = 10`, `MAX_QUERY_LENGTH = 128` (fulltext queries longer than this return no query).

**Recipe constants (the "search type" names you asked about)** — module-level `SearchConfig` objects in C `search_config_recipes.py`; the docs list them in GD `working-with-data/searching.md`:

`COMBINED_HYBRID_SEARCH_RRF`, `COMBINED_HYBRID_SEARCH_MMR`, `COMBINED_HYBRID_SEARCH_CROSS_ENCODER`, `EDGE_HYBRID_SEARCH_RRF`, `EDGE_HYBRID_SEARCH_MMR`, `EDGE_HYBRID_SEARCH_NODE_DISTANCE`, `EDGE_HYBRID_SEARCH_EPISODE_MENTIONS`, `EDGE_HYBRID_SEARCH_CROSS_ENCODER`, `NODE_HYBRID_SEARCH_RRF`, `NODE_HYBRID_SEARCH_MMR`, `NODE_HYBRID_SEARCH_NODE_DISTANCE`, `NODE_HYBRID_SEARCH_EPISODE_MENTIONS`, `NODE_HYBRID_SEARCH_CROSS_ENCODER`, `COMMUNITY_HYBRID_SEARCH_RRF`, `COMMUNITY_HYBRID_SEARCH_MMR`, `COMMUNITY_HYBRID_SEARCH_CROSS_ENCODER`.

⚠ **Important correction to the premise:** these are **Python constants, not a `search_type` string parameter.** There is no `search_type` argument anywhere in `graphiti-core`'s public API, and none in the Graphiti MCP server's tool list (C `mcp_server/README.md`) or Zep's hosted `graph.search` (ZD `/searching-the-graph`). The hosted API instead parameterises by **`scope`** + **`reranker`**. Also note: only 3 of the recipes use BFS (`COMBINED_HYBRID_SEARCH_CROSS_ENCODER`, `EDGE_HYBRID_SEARCH_CROSS_ENCODER`, `NODE_HYBRID_SEARCH_CROSS_ENCODER`); the RRF defaults do **not** include BFS. The GD table lists **16** recipes while the prose says "**The 15 recipes** are the following" — a documentation miscount.

**Entry-point defaults in `graphiti-core`:**

| Call | Recipe used | Source |
|---|---|---|
| `graphiti.search(query)` | `EDGE_HYBRID_SEARCH_RRF` (bm25 + cosine on edges, RRF; **no BFS, no LLM rerank**) | C `graphiti.py` |
| `graphiti.search(query, center_node_uuid=...)` | `EDGE_HYBRID_SEARCH_NODE_DISTANCE` | C `graphiti.py` |
| `graphiti.search_(query)` (advanced) | `COMBINED_HYBRID_SEARCH_CROSS_ENCODER` (bm25+cosine+**bfs** on edges & nodes, cross-encoder rerank) | C `graphiti.py` |
| Ingestion dedup + invalidation candidate search | `EDGE_HYBRID_SEARCH_RRF` | C `edge_operations.py` |

**Zep hosted API — documented parameters and defaults** (ZD `/searching-the-graph`, "Configurable Parameters"):

| Parameter | Type | Default | Notes |
|---|---|---|---|
| `scope` | string | **`"edges"`** | `"auto"`, `"edges"`, `"nodes"`, `"episodes"`, `"observations"`, `"thread_summaries"` |
| `reranker` | string | **`"rrf"`** | `"rrf"`, `"mmr"`, `"node_distance"`, `"episode_mentions"`, `"cross_encoder"` |
| `limit` | integer | **`10`** | |
| `max_characters` | integer | **`2500`** | cap `50000`; only for `scope="auto"` |
| `return_raw_results` | boolean | **`false`** | only for `scope="auto"` |
| `mmr_lambda` | float | *(not stated)* | 0.0–1.0; required when `reranker="mmr"` |
| `center_node_uuid` | string | *(not stated)* | required when `reranker="node_distance"` |
| `bfs_origin_node_uuids` | array | *(not stated)* | "Up to **five** node or episode UUIDs that seed breadth-first searches" |
| `search_filters` | object | *(not stated)* | `node_labels`, `edge_types`, `exclude_node_labels`, `exclude_edge_types`, `property_filters`, `episode_metadata_filters`, `connected_node_uuids`, `source_node_uuids`, `target_node_uuids`, `episode_uuids`, and timestamps `created_at` / `expired_at` / `invalid_at` / `valid_at` (timestamps: edge scope only) |
| `query` | string | — | **max 400 characters** on the public API |

⚑ **`bfs_max_depth` is not a documented public parameter of Zep's hosted API** — it exists only in `graphiti-core` (default `3`). I could not find a published value or override for BFS depth on the hosted service. Similarly, `sim_min_score`/`reranker_min_score` are not exposed in the hosted parameter table.

**Zep `auto` search** (ZD): single call, retrieves across all scopes in parallel, applies "its own internal **cross-scope rerank**", and packs into a `max_characters` budget. "When `scope="auto"`, the `reranker` parameter is **ignored entirely**." It also resolves relative time phrases ("yesterday", "last week") into an automatic temporal filter using "a machine learning model"; when applied, the returned context block states the restriction and includes the UTC interval and timezone.

---

## 5. Incremental updates, latency, roadmap

### 5.1 Incremental-update claims (verbatim)

- RD: the framework "supports **incremental data updates**, efficient retrieval, and precise historical queries **without requiring complete graph recomputation**".
- RD "Why Graphiti" → **Incremental Graph Construction:** "New data integrates immediately **without batch recomputation**. The graph evolves in real-time as episodes are ingested."
- RD "Why Graphiti" → **Temporal Fact Management:** "When information changes, old facts are **invalidated — not deleted**. Query what's true now, or what was true at any point in time."
- RD "Graphiti vs. GraphRAG": Graphiti = "Continuous, incremental updates", "Explicit **bi-temporal** tracking with automatic fact invalidation", "Automatic fact invalidation with temporal history preserved", query latency "Typically **sub-second**".
- P §2.3 (the *only* place the paper makes an incremental claim): the label-propagation dynamic extension "enables the system to maintain accurate community representations for longer periods as new data enters the graph, **delaying the need for complete community refreshes**… provides a practical heuristic that **significantly reduces latency and LLM inference costs**."
- P §2.2.3: "This comprehensive approach enables the **dynamic addition of data** to Graphiti as conversations evolve, while maintaining both current relationship states and historical records."

⚑ **Scope caveat:** the paper does **not** claim "no re-indexing" anywhere. It explicitly says "**periodic community refreshes remain necessary**" (P §2.3), and the code's `build_communities()` deletes and rebuilds all communities for the target groups. So: **episode ingestion is incremental (no global recompute, no vector re-index of existing edges)**; **community detection is not** — it is a periodic full rebuild of that tier. Incremental community handling happens only per-node during ingestion and only when `update_communities=True` (off by default).

### 5.2 Latency numbers (all vendor self-reported)

| Claim | Value | Source / date |
|---|---|---|
| LongMemEval median latency, Zep + gpt-4o | **2.58 s** (IQR 0.684 s) vs full-context **28.9 s** (IQR 6.01 s) | P Table 2; B1 |
| LongMemEval median latency, Zep + gpt-4o-mini | **3.20 s** (IQR 1.31 s) vs full-context **31.3 s** (IQR 8.76 s) | P Table 2; B1 |
| Aggregate latency reduction | "approximately **90%**" / "latency reductions of an order of magnitude" | P §4.3.2; B1 |
| Avg context tokens | **1.6k** (Zep) vs **115k** (full-context) — "less than 2% of the baseline tokens" | P Table 2; B1 |
| LoCoMo p95 *search* latency | **0.632 s** (Zep, concurrent search) vs 0.778 s reported for Zep by Mem0 (sequential) vs 0.657 s Mem0 graph | B2, 6 May 2025 |
| Product claim | "Pre-configured, production-ready retrieval with **sub-200ms performance at scale**" | RD "Zep vs Graphiti" (present in both the current README and the pre-2026-08-26 version) |
| LoCoMo config sweep, p50 retrieval | 149 ms (5/2), 161 (10/2), 199 (15/5), 241 (20/20), 189 (30/30) — "**p50 retrieval stayed under 200ms across every configuration**" | B3, 9 Dec 2025 |
| Auto search on LoCoMo | accuracy 86.5%, latency **115 / 173 ms** p50/p95, median context 2,680 tokens (−53%) | R (current) |
| Current loftiest latency claims | LongMemEval retrieval latency **104 / 162 ms** p50/p95; LoCoMo **87 / 155 ms** p50/p95 | R (current, 2026) |

### 5.3 Roadmap / stated future work (P §5)

- Integrating other GraphRAG approaches into the Zep paradigm.
- Fine-tuned models for Graphiti's entity/edge extraction prompts ("may enhance knowledge extraction, particularly for complex conversations").
- Domain-specific **ontologies** ("Graph ontologies, foundational in pre-LLM knowledge graph work, warrant further exploration within the Graphiti framework").
- Calls for better memory benchmarks, especially business/enterprise tasks; notes "**no existing benchmarks adequately assess Zep's capability to process and synthesize conversation history with structured business data**".
- The paper also flags a *known regression*: single-session-assistant questions dropped 17.7% (gpt-4o) and 9.06% (gpt-4o-mini) versus the full-context baseline, "areas of current research and engineering work".

---

## 6. Benchmark numbers (⚠ ALL VENDOR SELF-REPORTED)

### 6.1 Correction to the brief's premise: the Graphiti README has **no** benchmark table

The brief asked for "the Graphiti GitHub README benchmark table". **It does not exist — now, or (as far as this research could establish) ever.**

- Current `main` README contains no numeric benchmark table; its only performance statement is the qualitative Zep-vs-Graphiti row "Retrieval & performance | Pre-configured, production-ready retrieval with sub-200ms performance at scale | Custom implementation required" (RD).
- The pre-2026-08-26 README (commit `993e081a6d7948a0d8851c12a5fbdbeb49fed862`, parent of the "Reframe README as a temporal knowledge graph framework (#1801)" commit dated 2026-08-26) also has no benchmark table: https://raw.githubusercontent.com/getzep/graphiti/993e081a6d7948a0d8851c12a5fbdbeb49fed862/README.md
- Parallel verification found no numeric benchmark table at tags `v0.5.0`, `v0.10.0`, `v0.13.0`, `v0.17.0` nor at commits `b59d4ba0` / `526dcad7`.
- The README points instead to a blog post for the benchmark claims: "Using Graphiti, we've demonstrated Zep is the State of the Art in Agent Memory" → https://blog.getzep.com/state-of-the-art-agent-memory/

**Consequence:** there is **no paper-vs-README numeric discrepancy** to report for DMR/LongMemEval, because the README publishes no numbers. The paper's numbers are reproduced verbatim on the Zep blog, which is a *vendor-consistent* echo, not an independent replication.

### 6.2 DMR (Deep Memory Retrieval) — paper Table 1 = blog; no numeric conflict

Benchmark: 500 multi-session conversations, 5 sessions each, up to 12 messages/session (60 messages total) — a 500-conversation subset of Multi-Session Chat (P §4.2). Zep retrieved the top 10 nodes+edges and used an LLM judge.

| Memory | Model | Score |
|---|---|---|
| Recursive Summarization† | gpt-4-turbo | 35.3% |
| Conversation Summaries | gpt-4-turbo | 78.6% |
| **MemGPT†** | gpt-4-turbo | **93.4%** |
| Full-conversation | gpt-4-turbo | 94.4% |
| **Zep** | gpt-4-turbo | **94.8%** |
| Conversation Summaries | gpt-4o-mini | 88.0% |
| Full-conversation | gpt-4o-mini | 98.0% |
| **Zep** | gpt-4o-mini | **98.2%** |

† results reported in the MemGPT paper. Source: P Table 1 (https://arxiv.org/html/2501.13956v1), reproduced on B1. Headline claim: "94.8% vs 93.4%" (P Abstract). Zep's own caveat, quoted: "these results must be contextualized: each conversation contains only 60 messages, easily fitting within current LLM context windows" (P §4.2).

**Margins (the key caveat for a technical report):** Zep's edge over MemGPT is **+1.4 pp** (94.8 vs 93.4). Its edge over the *trivial* full-conversation baseline is **+0.4 pp** on gpt-4-turbo (94.8 vs 94.4) and **+0.2 pp** on gpt-4o-mini (98.2 vs 98.0). Zep itself calls the benchmark "inadequate" (P §4.2) — the full-context baseline is essentially at parity.

### 6.3 LongMemEval (LME) — paper Table 2 = blog; no numeric conflict

Dataset: LongMemEval_S, ~115,000 tokens per problem (the larger `_M` variant at 1.5M tokens was not used, due to gpt-4o's 128k window — B1). Six question types. Experiments run Dec 2024 – Jan 2025 from "a consumer laptop from a residential location in Boston, MA, connecting to Zep's service hosted in AWS us-west-2" (P §4.3). Answer evaluation by GPT-4o with the LongMemEval paper's question-specific prompts. Zep retrieved the **20** most relevant edges + entity nodes (P §4).

| Memory | Model | Score | Latency (median) | Latency IQR | Avg Context Tokens |
|---|---|---|---|---|---|
| Full-context | gpt-4o-mini | 55.4% | 31.3 s | 8.76 s | 115k |
| **Zep** | gpt-4o-mini | **63.8%** | **3.20 s** | 1.31 s | **1.6k** |
| Full-context | gpt-4o | 60.2% | 28.9 s | 6.01 s | 115k |
| **Zep** | gpt-4o | **71.2%** | **2.58 s** | 0.684 s | **1.6k** |

Source: P Table 2 (https://arxiv.org/html/2501.13956v1); identical figures on B1. Headline claims: "accuracy improvements of up to **18.5%**" and "reducing response latency by **90%**" (P Abstract). Absolute deltas: **+15.2 pp** (gpt-4o-mini) and **+18.5 pp** (gpt-4o).

**Per-question-type breakdown** (P Table 3; identical on B1):

| Question Type | Model | Full-context | Zep | Delta |
|---|---|---|---|---|
| single-session-preference | gpt-4o-mini | 30.0% | 53.3% | 77.7%↑ |
| single-session-assistant | gpt-4o-mini | 81.8% | 75.0% | 9.06%↓ |
| temporal-reasoning | gpt-4o-mini | 36.5% | 54.1% | 48.2%↑ |
| multi-session | gpt-4o-mini | 40.6% | 47.4% | 16.7%↑ |
| knowledge-update | gpt-4o-mini | 76.9% | 74.4% | 3.36%↓ |
| single-session-user | gpt-4o-mini | 81.4% | 92.9% | 14.1%↑ |
| single-session-preference | gpt-4o | 20.0% | 56.7% | 184%↑ |
| single-session-assistant | gpt-4o | 94.6% | 80.4% | 17.7%↓ |
| temporal-reasoning | gpt-4o | 45.1% | 62.4% | 38.4%↑ |
| multi-session | gpt-4o | 44.3% | 57.9% | 30.7%↑ |
| knowledge-update | gpt-4o | 78.2% | 83.3% | 6.52%↑ |
| single-session-user | gpt-4o | 81.4% | 92.9% | 14.1%↑ |

**Models used:** graph construction = `gpt-4o-mini-2024-07-18`; chat agent = `gpt-4o-mini-2024-07-18` and `gpt-4o-2024-11-20`; DMR comparison also run with `gpt-4-turbo-2024-04-09`; reranking **and** embedding = BAAI **BGE-m3** (P §4.1). ⚑ Note the paper says BGE-m3 was used "for both reranking and embedding tasks" — consistent with the code's 1024-dim default — but the shipped default in `graphiti-core` today is `OpenAIRerankerClient` (gpt-4.1-nano), so the paper's configuration is **not** the library default.

**Zep could not benchmark MemGPT on LongMemEval:** "the current MemGPT framework does not support direct ingestion of existing message histories… we were unable to achieve successful question responses using this approach" (P §4.3.1). So the LongMemEval comparison is Zep vs. **full-context baseline only** — there is **no** Zep-vs-MemGPT LongMemEval number.

### 6.4 LOCOMO — the contested one (report both, and the third number)

LOCOMO is **not in the arXiv paper** (the paper covers only DMR and LongMemEval). All LOCOMO numbers come from Zep's blog/pages and a competitor's rebuttal.

| Claim | Number | Who / when | URL |
|---|---|---|---|
| Original Zep claim | **84%** | Zep blog, ~Apr–May 2025 (now corrected in place) | B2 |
| Mem0's re-evaluation | **58.44% ± 0.20** (10 runs) — "artificially inflated the reported accuracy by approximately **25.56 percentage points**" | Deshraj, co-founder/CTO, Mem0 — issue opened 2025-05-08 | I5 |
| Zep's corrected number, after acknowledging the error | "**75.14% +/- 0.17** (over 10 runs), with Zep outperforming Mem0 by ~10%" | Daniel Chalef, Zep — 2025-05-12 | I5 comment 2873485203; B2 correction note |
| Mem0's own baseline for Zep | 65.99% (as published in the Mem0 paper, arXiv:2504.19413) | Mem0 | I5 / B2 |
| Mem0 Graph (best Mem0 config) | ~68% J-score | per B2 | B2 |
| Full-context baseline on LoCoMo | ~73% J-score — i.e. **a full-context baseline beat Mem0's best** | per B2 | B2 |
| LoCoMo re-run sweep (gpt-4o-mini agent + grader, 10 runs/config) | 30/30 **80.32 ± 0.43**; 20/20 **80.06 ± 0.33**; 15/5 **77.06 ± 0.41**; 10/2 **73.72 ± 0.41**; 5/2 **69.62 ± 0.47** | Zep, 9 Dec 2025 | B3 |
| Current Zep page | LoCoMo **94.7%** (1,459/1,540), retrieval latency 87/155 ms p50/p95, median context 5,760 tokens | Zep `/research`, 2026 | R |
| Current head-to-head | Zep **94.7%** vs **mem0 91.6%**; retrieval 35× faster at p50 (87 ms vs 3,060 ms); context 17% smaller (5,760 vs 6,956 tokens). LongMemEval: Zep 90.2% vs mem0 90.4% (top_50), 24× faster (104 ms vs 2,470 ms), 35% smaller context. "Mem0 baseline numbers published by mem0 at github.com/mem0ai/memory-benchmarks." | Zep `/mem0-alternative`, 2026 | M |

⚑ **These LOCOMO numbers are not comparable across rows.** The Dec-2025 sweep used `gpt-4o-mini` as both agent and grader (B3). The current `/research` page used **reader `gpt-5.4` (reasoning = medium), judge `gpt-5.4` with chain-of-thought grading**, multi-scope depth "20 edges, 10 nodes, 10 episodes, 5 thread summaries, 5 observations, cross-encoder reranking", auto search at `max_characters=10000` (R methodology section). The 2025 dispute ran on the Mem0 evaluation harness with gpt-4o-class models and a different system prompt. Any 84 → 75.14 → 80.32 → 94.7 progression is a **methodology churn**, not a monotone improvement.

⚑ **Zep's own position on LOCOMO:** "we're **not huge fans of this benchmark**… It has known issues with question quality and answer ambiguity" (B3), and B2 enumerates the flaws: average conversation only 16k–26k tokens (fits in context), **no knowledge-update questions**, Category 5 unusable due to missing ground truth, multimodal errors, incorrect speaker attribution, underspecified questions.

⚑ **Typo in a primary source:** B2's update banner reads "**December 9, 2015**: We've published new LoCoMo scores for Zep: 80% at <200ms latency." The post itself is dated **09 Dec 2025** — the 2015 is a typo.

⚑ **No Graphiti-README LOCOMO number exists** to compare against, so the "README vs paper" discrepancy the brief anticipated cannot arise from the README.

⚑ **Also note the Mem0 dispute is unresolved in the record:** Zep closed issue #5 on 2025-05-19 "due to inactivity" after asking Mem0 to share their resulting datasets; Mem0's 58.44% figure was never reconciled with Zep's 75.14% figure in that thread (I5).

---

## 7. Claimed advantages over pure vector / chunk memory

From RD "Why Graphiti" and RD "Graphiti vs. GraphRAG" (verbatim claims), with the mechanism each maps to:

| Claimed advantage | Exact claim (README) | Mechanism in paper/code |
|---|---|---|
| **Temporal reasoning** | "Facts have validity windows. When information changes, old facts are invalidated — not deleted. Query what's true now, or what was true at any point in time." Reads to "Typically **sub-second latency**" vs GraphRAG's "Seconds to tens of seconds". | Bi-temporal `T`/`T'`; four timestamps on `EntityEdge`; `SearchFilters.valid_at/invalid_at/created_at/expired_at` (P §2.2.3; C `search_filters.py`) |
| **Contradiction handling** | "Automatic fact invalidation with temporal history preserved" (vs GraphRAG's "LLM-driven summarization judgments"). Zep docs: "when Kendra initially loves Adidas shoes but later is angry that the shoes broke… Zep attempts to **invalidate** the fact that Kendra loves Adidas shoes and creates two new facts" | LLM `dedupe_edges.resolve_edge` → `contradicted_facts` → `resolve_edge_contradictions` sets `invalid_at` to the new edge's `valid_at` (P §2.2.3; C `edge_operations.py`; ZD `/facts`) |
| **No re-indexing / incremental** | "New data integrates immediately without batch recomputation." "supports incremental data updates… without requiring complete graph recomputation." "Continuous, incremental updates" vs GraphRAG's "Batch-oriented processing". | Episode-scoped ingestion; edge dedup search constrained to same entity pair; label-propagation single-step dynamic community extension. ⚑ **Caveat:** the community tier is *not* incrementally maintained in general — `build_communities()` deletes and rebuilds, and the paper says periodic refreshes "remain necessary" (P §2.3) |
| **Explainability / provenance** | "Every entity and relationship traces back to the episodes (raw data) that produced it. **Full lineage** from derived fact to source." Episodic subgraph is "**non-lossy**". Retrieval "without reliance on **LLM summarization**". | `(:Episodic)-[:MENTIONS]->(:Entity)` edges + `EntityEdge.episodes[]` back-pointers = "bidirectional indices"; paper: "semantic artifacts can be traced to their sources for citation or quotation" (P §2.1) |
| **Hybrid retrieval** | "Combines semantic embeddings, keyword (BM25), and graph traversal for low-latency, high-precision queries" | φ_cos + φ_bm25 + φ_bfs (P §3.1) |
| **Adaptive ontology** | "Prescribed & Learned Ontology: Define entity and edge types upfront via Pydantic models (prescribed), or let structure emerge from your data (learned)." "Custom Entity Types: No (GraphRAG) / Yes (Graphiti)" | `entity_types`, `edge_types`, `edge_type_map` parameters (C `graphiti.py`) |

⚑ One README/overview claim needs qualification: Graphiti's overview docs say hybrid search works "with **no LLM-in-the-loop reranking**" (GD `getting-started/overview.md`). That is true of the default `graphiti.search()` path (RRF over BM25+cosine), but false for `graphiti.search_()`'s default (`COMBINED_HYBRID_SEARCH_CROSS_ENCODER`) and for Zep's hosted `reranker="cross_encoder"`, which both call an LLM classifier. Note also that **ingestion** is heavily LLM-dependent either way: entity extraction, entity resolution, edge extraction/dedup, timestamp extraction, and summarisation all invoke an LLM.

---

## 8. Everything I could NOT verify, or where sources conflict

**Conflicts (all verified against ≥2 primary sources):**

1. **Community algorithm: Leiden vs. label propagation.** Docs say Leiden; paper and code say label propagation. → GD `core-concepts/communities.md` vs P §2.3 and C `community_operations.py` `label_propagation()`. **Code + paper win.**
2. **Bulk-ingestion edge invalidation.** Code docstring says it happens; docs warn it does not. → C `graphiti.py` `add_episode_bulk` vs GD `core-concepts/adding-episodes.md`. **Unresolved.**
3. **Entity-resolution candidate generation.** Paper describes cosine **+ full-text** candidate retrieval; current code's `_semantic_candidate_search` is cosine-only. → P §2.2.1 vs C `node_operations.py`.
4. **Context window `n`.** Paper: `n = 4` previous messages. Code: `add_episode` passes `last_n = RELEVANT_SCHEMA_LIMIT = 10`; `EPISODE_WINDOW_LEN = 3`. → P §2.2.1 vs C `graphiti.py`, `graph_data_operations.py`, `search_utils.py`. **Three different numbers.**
5. **Episode types.** Paper and docs: 3 (`message`, `text`, `json`). Code: 4 (adds `fact_triple`).
6. **"15 recipes".** Docs prose says 15; the doc's own table lists 16; the code defines 16.
7. **LOCOMO accuracy.** 84% (Zep original) vs 58.44% ± 0.20 (Mem0) vs 75.14% ± 0.17 (Zep corrected) vs 80.32% ± 0.43 (Zep, Dec 2025, gpt-4o-mini) vs 94.7% (Zep, 2026, gpt-5.4). **All vendor/competitor self-reported, different harnesses, not comparable.**
8. **`t_invalid` initial value / exact invalidation semantics.** The paper says `t_invalid` is set to the invalidating edge's `t_valid`; the code additionally sets `expired_at = now` and can also expire the *new* edge when a candidate has a later `valid_at`. The code is the more complete description.

**Not found / not verifiable from primary sources:**

- **No `search_type` parameter exists.** The name `search_type` does not appear in `graphiti-core`, the Graphiti MCP server tool list, or Zep's `graph.search` docs. The nearest real names are the `*_HYBRID_SEARCH_*` Python constants (recipes) and Zep's `scope` / `reranker` enums. A `search_type` argument existed in **Zep v2's memory API** (pre-v3) — I did not fetch the v2 reference to confirm exact values, so treat any v2 `search_type` list as **unverified**.
- **BFS depth on the hosted API.** `bfs_max_depth` (default 3) is only a `graphiti-core` config field; Zep's public search docs do not document a BFS-depth parameter.
- **No published `sim_min_score` / `reranker_min_score` override on the hosted API.** Only `graphiti-core` (`0.6` and `0` respectively).
- **Graphiti README benchmark table: does not exist** (verified on `main` and at commit `993e081a`, plus tags `v0.5.0`, `v0.10.0`, `v0.13.0`, `v0.17.0` per parallel verification). If a benchmark table is genuinely remembered, it is most likely being conflated with the Zep blog post B1 or with a third-party write-up.
- **Observation tier.** Zep's v3 product docs add an `observations` context type and a `thread_summaries` scope (ZD `/concepts`, ZD `/searching-the-graph`) that have **no counterpart in the Graphiti OSS code or in the paper**. Their internal construction is undocumented publicly; I did not verify it.
- **The hosted service (`Konig` Context Graph Engine) internals.** Zep describes "Hot graphs are held in memory as adjacency lists and CSR matrices, with vector and BM25 indexes alongside" and "millions of context graphs" (R; RD). This is **marketing copy with no reproducible specification**. The "sub-200ms at scale" claim likewise.
- **`Zep` per-benchmark version numbers.** Neither the paper nor the blog pins a Graphiti/Zep release version to the DMR/LongMemEval numbers. The paper gives model versions and a date range (Dec 2024 – Jan 2025) but no software version. The current `/research` numbers give a model pair (`gpt-5.4` reader/judge) but no SDK version.
- **Embedding model in production.** Paper uses BGE-m3 (1024-dim); library default is `EMBEDDING_DIM=1024` but the default embedder client is `OpenAIEmbedder`. Which is used by the hosted service at the time of each benchmark is **not stated**.
