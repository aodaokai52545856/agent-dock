# Zep / Graphiti — Citation-Grade Research Report

**Research date:** current session. All quotes verbatim from the cited URL. Where a fact comes from **source code** rather than prose docs, it is labelled `[CODE]` with the raw GitHub URL — these are authoritative for defaults but are *not* "documented" in the sense of the docs site.

## Access notes (what worked)

| Target | Result |
|---|---|
| `https://help.getzep.com/llms.txt` | ✅ HTTP 200 — full doc index |
| `https://help.getzep.com/<page>.md` | ✅ works; Mintlify `.md` suffix returns clean markdown |
| `https://help.getzep.com/graphiti/llms.txt` | ✅ section index (22 Graphiti pages) |
| `https://help.getzep.com/v2/llms.txt` | ✅ v2 index |
| `https://blog.getzep.com/sitemap-posts.xml`, `https://www.getzep.com/sitemap.xml` | ✅ XML |
| `https://raw.githubusercontent.com/getzep/graphiti/main/<path>` | ✅ full source |
| `https://docs.getzep.com/llms.txt` | ❌ cross-origin redirect → use `help.getzep.com` |
| `pwsh Invoke-WebRequest` to help.getzep.com | ❌ `The SSL connection could not be established` — all retrieval done via the `web_fetch` tool instead |

> IMPORTANT: `https://help.getzep.com/graphiti/graphiti/search.md` (the URL shape suggested in the brief) is **404 / not in the index**. The live Graphiti search page is `https://help.getzep.com/graphiti/working-with-data/searching.md`.

---

# A) SEARCH CONFIGURATION

## A1. Documented `search_type` / search recipe names

**Source (docs):** https://help.getzep.com/graphiti/working-with-data/searching.md

The page states verbatim:

> "Graphiti also provides a low-level search method that is more configurable than the out-of-the-box search. This search method can be called using `graphiti.search_()` and passing in an additional config parameter of type `SearchConfig`. `SearchConfig` contains four fields: one for the limit, and three more configs for each of edges, nodes, and communities. The `graphiti.search_()` method returns a `SearchResults` object containing a list of nodes, edges, and communities."

> "The `graphiti.search_()` method is configurable and can require more setup than `graphiti.search()`. As such, we also have a `search_config_recipes.py` file that contains a few prebuilt `SearchConfig` recipes for common use cases."

> "**The 15 recipes are the following:**"

⚠️ **The docs say "15 recipes" but the table immediately below lists 16 rows.** The code file also defines 16 module-level constants. Flagging as a documentation defect.

Exact names + verbatim descriptions from the docs table:

| Search Type | Description (verbatim) |
|---|---|
| `COMBINED_HYBRID_SEARCH_RRF` | "Performs a hybrid search with RRF reranking over edges, nodes, and communities." |
| `COMBINED_HYBRID_SEARCH_MMR` | "Performs a hybrid search with MMR reranking over edges, nodes, and communities." |
| `COMBINED_HYBRID_SEARCH_CROSS_ENCODER` | "Performs a full-text search, similarity search, and BFS with cross_encoder reranking over edges, nodes, and communities." |
| `EDGE_HYBRID_SEARCH_RRF` | "Performs a hybrid search over edges with RRF reranking." |
| `EDGE_HYBRID_SEARCH_MMR` | "Performs a hybrid search over edges with MMR reranking." |
| `EDGE_HYBRID_SEARCH_NODE_DISTANCE` | "Performs a hybrid search over edges with node distance reranking." |
| `EDGE_HYBRID_SEARCH_EPISODE_MENTIONS` | "Performs a hybrid search over edges with episode mention reranking." |
| `EDGE_HYBRID_SEARCH_CROSS_ENCODER` | "Performs a hybrid search over edges with cross encoder reranking." |
| `NODE_HYBRID_SEARCH_RRF` | "Performs a hybrid search over nodes with RRF reranking." |
| `NODE_HYBRID_SEARCH_MMR` | "Performs a hybrid search over nodes with MMR reranking." |
| `NODE_HYBRID_SEARCH_NODE_DISTANCE` | "Performs a hybrid search over nodes with node distance reranking." |
| `NODE_HYBRID_SEARCH_EPISODE_MENTIONS` | "Performs a hybrid search over nodes with episode mentions reranking." |
| `NODE_HYBRID_SEARCH_CROSS_ENCODER` | "Performs a hybrid search over nodes with cross encoder reranking." |
| `COMMUNITY_HYBRID_SEARCH_RRF` | "Performs a hybrid search over communities with RRF reranking." |
| `COMMUNITY_HYBRID_SEARCH_MMR` | "Performs a hybrid search over communities with MMR reranking." |
| `COMMUNITY_HYBRID_SEARCH_CROSS_ENCODER` | "Performs a hybrid search over communities with cross encoder reranking." |

**Note:** all six names in the brief that use `_EPISODE_MENTIONS` / `_NODE_DISTANCE` **exist and are spelled exactly as guessed**. There is **no `EPISODE_HYBRID_SEARCH_*` recipe** in either the docs table or the code.

### Recipe-level overrides that differ from field defaults `[CODE]`

**Source:** https://raw.githubusercontent.com/getzep/graphiti/main/graphiti_core/search/search_config_recipes.py

- `COMBINED_HYBRID_SEARCH_MMR` sets **`mmr_lambda=1`** on its edge, node, and community configs (i.e. not the 0.5 field default).
- `EDGE_HYBRID_SEARCH_CROSS_ENCODER` carries **`limit=10`**.
- `NODE_HYBRID_SEARCH_CROSS_ENCODER` carries **`limit=10`**.
- `COMMUNITY_HYBRID_SEARCH_CROSS_ENCODER` carries **`limit=3`**.
- All other recipes set only `search_methods` and `reranker`, inheriting field defaults.
- `COMBINED_HYBRID_SEARCH_*` recipes also define an `episode_config` (e.g. `EpisodeSearchConfig(search_methods=[EpisodeSearchMethod.bm25], reranker=EpisodeReranker.rrf)`) which the docs table does not mention for those recipes.
- The three `*_CROSS_ENCODER` recipes use `search_methods=[bm25, cosine_similarity, bfs]`; the RRF/MMR recipes use only `[bm25, cosine_similarity]`.

## A2. Exact reranker names

**Source (docs):** https://help.getzep.com/graphiti/working-with-data/searching.md — section "Supported Reranking Approaches"

> "**Reciprocal Rank Fusion (RRF)** enhances search by combining results from different algorithms, like BM25 and semantic search. Each algorithm's results are ranked, converted to reciprocal scores (1/rank), and summed. This aggregated score determines the final ranking, leveraging the strengths of each method for more accurate retrieval."

> "**Maximal Marginal Relevance (MMR)** is a search strategy that balances relevance and diversity in results. It selects results that are both relevant to the query and diverse from already chosen ones, reducing redundancy and covering different query aspects. MMR returns varied search results by iteratively choosing results that maximize relevance while minimizing similarity to those already selected."

> "A **Cross-Encoder** is a model that jointly encodes a query and a result, scoring their relevance by considering their combined context. This approach often yields more accurate results compared to methods that encode query and a text separately."

> "Graphiti supports three cross encoders:
> * `OpenAIRerankerClient` (**the default**) - Uses an OpenAI model to classify relevance and the resulting `logprobs` are used to rerank results.
> * `GeminiRerankerClient` - Uses Google's Gemini models to classify relevance.
> * `BGERerankerClient` - Uses the `BAAI/bge-reranker-v2-m3` model and requires `sentence_transformers` be installed."

**Enum values `[CODE]`** — https://raw.githubusercontent.com/getzep/graphiti/main/graphiti_core/search/search_config.py

The docs page does **not** give the string values. The code does:

```python
class EdgeReranker(Enum):
    rrf = 'reciprocal_rank_fusion'
    node_distance = 'node_distance'
    episode_mentions = 'episode_mentions'
    mmr = 'mmr'
    cross_encoder = 'cross_encoder'
```

`NodeReranker` is identical (same 5 members). `EpisodeReranker` has only `rrf` + `cross_encoder`. `CommunityReranker` has only `rrf`, `mmr`, `cross_encoder`.

So the **wire/string** name for RRF is `reciprocal_rank_fusion`, **not** `rrf`. (`rrf` is the Python attribute name and the *Zep platform* API value.)

### RRF formula and `rank_const` `[CODE]`

**Source:** https://raw.githubusercontent.com/getzep/graphiti/main/graphiti_core/search/search_utils.py

```python
# takes in a list of rankings of uuids
def rrf(
    results: list[list[str]], rank_const=1, min_score: float = 0
) -> tuple[list[str], list[float]]:
    scores: dict[str, float] = defaultdict(float)
    for result in results:
        for i, uuid in enumerate(result):
            scores[uuid] += 1 / (i + rank_const)
```

- `rank_const` **default = 1** (positional, no type annotation).
- Formula: `score(uuid) += 1 / (i + rank_const)` where `i` is the **0-based** index.
- With the default `rank_const=1`, rank-0 gets `1/(0+1) = 1.0`.
- `rrf`'s own `min_score` default is `0`.
- `episode_mentions_reranker` calls `rrf(node_uuids)` with no `rank_const` argument, i.e. the default 1, as its "preliminary ranker".

**Was `rank_const` documented anywhere?** ❌ **Not stated on any prose docs page.** It appears only in source code. No page states the RRF formula algebraically other than the docs' prose "(1/rank)".

### `mmr` formula `[CODE]`

```python
def maximal_marginal_relevance(
    query_vector: list[float],
    candidates: dict[str, list[float]],
    mmr_lambda: float = DEFAULT_MMR_LAMBDA,
    min_score: float = -2.0,
) -> tuple[list[str], list[float]]:
    ...
        mmr = mmr_lambda * np.dot(query_array, candidate_arrays[uuid]) + (mmr_lambda - 1) * max_sim
```

Note the implemented coefficient is `(mmr_lambda - 1)`, not the textbook `(1 - mmr_lambda)`. Worth verifying against intent before quoting this as "standard MMR". `min_score` default here is `-2.0` (not 0).

### `node_distance` reranker `[CODE]`

```python
async def node_distance_reranker(
    driver: GraphDriver,
    node_uuids: list[str],
    center_node_uuid: str,
    min_score: float = 0,
) -> tuple[list[str], list[float]]:
```

- Connected neighbours get `score = 1`; unreachable nodes get `score = float('inf')`; the centre node itself is re-inserted with `scores[center_node_uuid] = 0.1`.
- Return filter is `if (1 / scores[uuid]) >= min_score`.
- **`center_node_uuid` is the parameter name** (not `focal_node_uuid`). The docs example uses `center_node_uuid=jane_node_uuid`. However the prose in `searching.md` calls it a "focal node": *"Extends Hybrid Search above by prioritizing results based on proximity to a specified node in the graph."* and the sample code comment says `focal_node_uuid = "123e4567-e89b-12d3-a456-426614174000"` before passing it as `center_node_uuid=`. So both names appear on that page; only `center_node_uuid` is a real kwarg.

### `episode_mentions` reranker `[CODE]`

```python
async def episode_mentions_reranker(
    driver: GraphDriver, node_uuids: list[list[str]], min_score: float = 0
) -> tuple[list[str], list[float]]:
```

Counts `MATCH (episode:Episodic)-[r:MENTIONS]->(n:Entity {uuid: node_uuid}) RETURN count(*) AS score`; nodes with no `MENTIONS` edges get score `0`; sorted descending.

## A3. Documented / coded defaults

### Graphiti `SearchConfig` / `*SearchConfig` `[CODE]`

**Source:** https://raw.githubusercontent.com/getzep/graphiti/main/graphiti_core/search/search_config.py

```python
DEFAULT_SEARCH_LIMIT = 10

class EdgeSearchConfig(BaseModel):
    search_methods: list[EdgeSearchMethod]
    reranker: EdgeReranker = Field(default=EdgeReranker.rrf)
    sim_min_score: float = Field(default=DEFAULT_MIN_SCORE)
    mmr_lambda: float = Field(default=DEFAULT_MMR_LAMBDA)
    bfs_max_depth: int = Field(default=MAX_SEARCH_DEPTH)

class SearchConfig(BaseModel):
    edge_config: EdgeSearchConfig | None = Field(default=None)
    node_config: NodeSearchConfig | None = Field(default=None)
    episode_config: EpisodeSearchConfig | None = Field(default=None)
    community_config: CommunitySearchConfig | None = Field(default=None)
    limit: int = Field(default=DEFAULT_SEARCH_LIMIT)
    reranker_min_score: float = Field(default=0)
```

`NodeSearchConfig`, `EpisodeSearchConfig`, `CommunitySearchConfig` have the **same five fields with the same defaults**.

**Source for the constants:** https://raw.githubusercontent.com/getzep/graphiti/main/graphiti_core/search/search_utils.py

```python
RELEVANT_SCHEMA_LIMIT = 10
DEFAULT_MIN_SCORE = 0.6
DEFAULT_MMR_LAMBDA = 0.5
MAX_SEARCH_DEPTH = 3
MAX_QUERY_LENGTH = 128
```

| Parameter | Default | Verdict on the brief's guess |
|---|---|---|
| `limit` | `10` (`DEFAULT_SEARCH_LIMIT`) | ✅ confirmed (code only; not on `searching.md`) |
| `sim_min_score` | `0.6` (`DEFAULT_MIN_SCORE`) | ✅ confirmed (code only) |
| `mmr_lambda` | `0.5` (`DEFAULT_MMR_LAMBDA`) | ✅ confirmed (code only) |
| `bfs_max_depth` | `3` (`MAX_SEARCH_DEPTH`) | ✅ confirmed (code only) |
| `reranker_min_score` | `0` | ✅ confirmed (code only) |
| `RELEVANT_SCHEMA_LIMIT` | `10` | used as the internal default limit in `edge_fulltext_search`, `edge_similarity_search`, `node_fulltext_search`, `node_similarity_search`, `get_edge_invalidation_candidates`, `get_episodes_by_mentions` |
| `MAX_QUERY_LENGTH` | `128` | fulltext query token cap |

**None of these six defaults is stated on `searching.md`.** The page states the *existence* of the limit field but not its value. On the **Zep platform** side, by contrast, the defaults *are* tabulated (see A5).

### Graphiti `Graphiti.search()` defaults `[CODE]`

**Source:** https://raw.githubusercontent.com/getzep/graphiti/main/graphiti_core/graphiti.py

```python
async def search(
    self,
    query: str,
    center_node_uuid: str | None = None,
    group_ids: list[str] | None = None,
    num_results=DEFAULT_SEARCH_LIMIT,
    search_filter: SearchFilters | None = None,
    driver: GraphDriver | None = None,
) -> list[EntityEdge]:
```

Docstring verbatim: *"num_results : int, optional — The maximum number of results to return. **Defaults to 10.**"*

Recipe selection (verbatim code):
```python
search_config = (
    EDGE_HYBRID_SEARCH_RRF if center_node_uuid is None else EDGE_HYBRID_SEARCH_NODE_DISTANCE
)
search_config.limit = num_results
```

`search_()` default config (verbatim): `config: SearchConfig = COMBINED_HYBRID_SEARCH_CROSS_ENCODER`.

## A4. `SearchFilters` fields

### Graphiti `SearchFilters` — the complete field list `[CODE]`

**Source:** https://raw.githubusercontent.com/getzep/graphiti/main/graphiti_core/search/search_filters.py

```python
class SearchFilters(BaseModel):
    node_labels: list[str] | None = Field(
        default=None, description='List of node labels to filter on'
    )
    edge_types: list[str] | None = Field(
        default=None, description='List of edge types to filter on'
    )
    valid_at: list[list[DateFilter]] | None = Field(default=None)
    invalid_at: list[list[DateFilter]] | None = Field(default=None)
    created_at: list[list[DateFilter]] | None = Field(default=None)
    expired_at: list[list[DateFilter]] | None = Field(default=None)
    edge_uuids: list[str] | None = Field(default=None)
    property_filters: list[PropertyFilter] | None = Field(default=None)
```

⚠️ **There is NO `node_uuids` field** in `SearchFilters` on current `main`. The brief's guessed field list is partially wrong: `node_labels` ✅, `edge_types` ✅, `valid_at`/`invalid_at` ✅, `created_at` ✅, `edge_uuids` ✅ — but `node_uuids` ❌ does not exist. There is also an `expired_at` field the brief did not list.

Supporting types (verbatim):

```python
class ComparisonOperator(Enum):
    equals = '='
    not_equals = '<>'
    greater_than = '>'
    less_than = '<'
    greater_than_equal = '>='
    less_than_equal = '<='
    is_null = 'IS NULL'
    is_not_null = 'IS NOT NULL'

class DateFilter(BaseModel):
    date: datetime | None = Field(default=None, description='A datetime to filter on')
    comparison_operator: ComparisonOperator = Field(
        description='Comparison operator for date filter'
    )

class PropertyFilter(BaseModel):
    property_name: str = Field(description='Property name')
    property_value: str | int | float | None = Field(...)
    comparison_operator: ComparisonOperator = Field(...)
```

Semantics implemented in code: for each of `valid_at`/`invalid_at`/`created_at`/`expired_at`, the **outer list is OR**, the **inner list is AND** — the constructor emits `(a AND b) OR (c)`.

### Temporal filter syntax — the `[["2024-01-01", "2024-06-01"]]` form

⚠️ **The bare-string-pair form `[["2024-01-01", "2024-06-01"]]` is NOT documented on any page I found, and is not the Graphiti Python type.** In Graphiti Python the type is `list[list[DateFilter]]`, i.e. objects carrying an explicit `comparison_operator` and a `date`.

The **Zep REST API** reference documents the shape as a 2-D array of `DateFilter` objects. Verbatim from https://help.getzep.com/sdk-reference/graph/search.md:

> "`created_at` (list of list of graphiti.DateFilter, optional) — 2D array of date filters for the created_at field. The outer array elements are combined with OR logic. The inner array elements are combined with AND logic. Example: `[[{">", date1}, {"<", date2}], [{"=", date3}]]` This translates to: `(created_at > date1 AND created_at < date2) OR (created_at = date3)`"

Identical wording (substituting the field name) is given for `valid_at`, `invalid_at`, and `expired_at`. `graphiti.DateFilter`:

> "`comparison_operator` (enum, required) — Allowed values: `=`, `<>`, `>`, `<`, `>=`, `<=`, `IS NULL`, `is_null`, `IS NOT NULL`, `CONTAINS`
> `date` (string, optional) — Date to filter on. Required for non-null operators (`=`, `<>`, `>`, `<`, `>=`, `<=`). Should be omitted for IS NULL (or is_null) and IS NOT NULL operators."

**Zep platform docs** (https://help.getzep.com/searching-the-graph.md) give the full worked example and this verbatim statement:

> "**Date Format**: All dates must be in ISO 8601 format with timezone (e.g., \"2025-07-01T20:57:56Z\")"

> "**Logic Behavior:**
> * **Outer array/list**: Uses OR logic - any condition graph can match
> * **Inner array/list**: Uses AND logic - all conditions within a graph must match"

> "> **Edge Scope Only**
> > Datetime filtering only applies to edge scope searches. When using `scope=\"nodes\"` or `scope=\"episodes\"`, datetime filter values are ignored and have no effect on search results."

Four timestamps documented on that page, verbatim: `created_at` "The time when Zep learned the fact was true"; `valid_at` "The real world time that the fact started being true"; `invalid_at` "The real world time that the fact stopped being true"; `expired_at` "The time that Zep learned that the fact was false".

### Zep platform `search_filters` — full field list

**Source:** https://help.getzep.com/searching-the-graph.md (parameter table row) — verbatim:

> "Filter by entity types (`node_labels`), edge types (`edge_types`), exclude entity types (`exclude_node_labels`), exclude edge types (`exclude_edge_types`), custom properties (`property_filters`), episode metadata (`episode_metadata_filters`), connected nodes (`connected_node_uuids`, `source_node_uuids`, `target_node_uuids`), source episodes (`episode_uuids`), or timestamps (`created_at`, `expired_at`, `invalid_at`, `valid_at`§)"

**Source (REST type dump):** https://help.getzep.com/sdk-reference/graph/search.md gives the authoritative field list for `graphiti.SearchFilters`: `connected_node_uuids`, `created_at`, `edge_types`, `edge_uuids`, `episode_metadata_filters` (marked "[Experimental] ... This feature is experimental and may change in future releases."), `episode_uuids`, `exclude_edge_types`, `exclude_node_labels`, `expired_at`, `invalid_at`, `node_labels`, `property_filters`, `source_node_uuids`, `target_node_uuids`, `valid_at`.

Documented caps (verbatim): `connected_node_uuids` "Max 256 entries"; `edge_uuids` "Max 256 to align with graph-service filter limits."; `episode_uuids` "Max 256 entries."; `source_node_uuids` / `target_node_uuids` "Max 256 entries."

The Zep API `node_uuids`-equivalent fields are `connected_node_uuids` / `source_node_uuids` / `target_node_uuids` / `episode_uuids` — again, **no field literally named `node_uuids`**.

## A5. BFS as a search method

### Documented in Graphiti

**Source:** https://help.getzep.com/graphiti/working-with-data/searching.md — "BFS" appears only inside the recipe descriptions and in the `*_CROSS_ENCODER` recipe descriptions ("Performs a full-text search, similarity search, and BFS with cross_encoder reranking"). There is **no dedicated BFS section and no `bfs_origin_node_uuids` / `bfs_max_depth` mention on that page.**

The Graphiti code enum is where BFS is named as a method `[CODE]` (https://raw.githubusercontent.com/getzep/graphiti/main/graphiti_core/search/search_config.py):

```python
class EdgeSearchMethod(Enum):
    cosine_similarity = 'cosine_similarity'
    bm25 = 'bm25'
    bfs = 'breadth_first_search'
```

`NodeSearchMethod` is identical. `CommunitySearchMethod` has only `cosine_similarity` + `bm25` (**no BFS**). `EpisodeSearchMethod` has only `bm25`.

So: **BFS's string value is `breadth_first_search`**, and it is available on **edge and node** configs only.

`bfs_max_depth` usage `[CODE]` (https://raw.githubusercontent.com/getzep/graphiti/main/graphiti_core/search/search_utils.py):

```python
async def edge_bfs_search(
    driver: GraphDriver,
    bfs_origin_node_uuids: list[str] | None,
    bfs_max_depth: int,
    search_filter: SearchFilters,
    group_ids: list[str] | None = None,
    limit: int = RELEVANT_SCHEMA_LIMIT,
) -> list[EntityEdge]:
```
with `if bfs_origin_node_uuids is None or len(bfs_origin_node_uuids) == 0: return []` and the Cypher pattern `MATCH path = (origin {uuid: origin_uuid})-[:RELATES_TO|MENTIONS*1..{bfs_max_depth}]->(:Entity)`. Origin nodes may be `Entity` **or** `Episodic`.

`Graphiti.search_()` signature `[CODE]` (https://raw.githubusercontent.com/getzep/graphiti/main/graphiti_core/graphiti.py):

```python
async def search_(
    self,
    query: str,
    config: SearchConfig = COMBINED_HYBRID_SEARCH_CROSS_ENCODER,
    group_ids: list[str] | None = None,
    center_node_uuid: str | None = None,
    bfs_origin_node_uuids: list[str] | None = None,
    search_filter: SearchFilters | None = None,
    driver: GraphDriver | None = None,
) -> SearchResults:
```

### Documented for the Zep platform

**Source:** https://help.getzep.com/searching-the-graph.md — dedicated section "## Breadth-first search (BFS)", verbatim:

> "The `bfs_origin_node_uuids` parameter starts breadth-first searches from specified nodes or episodes. **You can provide up to five UUIDs.**
> **When to use**: Use BFS when you want to find information that's contextually connected to specific starting points in your graph, such as recent episodes or important entities."

Overview bullet, verbatim: "**Breadth-first search** (optional): Biases results toward information connected to specified starting nodes, useful for contextual relevance".

`center_node_uuid` in the Zep parameter table, verbatim: "Center node for distance-based reranking" — default `-`, marked "‡Required when using `node_distance` reranker".

**`bfs_max_depth` is NOT a documented Zep platform parameter** — it does not appear in the "Configurable Parameters" table, which lists exactly: `graph_id`, `user_id`, `query`, `scope`, `reranker`, `limit`, `max_characters`, `return_raw_results`, `mmr_lambda`, `center_node_uuid`, `search_filters`, `bfs_origin_node_uuids`.

Zep `reranker` documented values, verbatim: `"rrf"`, `"mmr"`, `"node_distance"`, `"episode_mentions"`, or `"cross_encoder"`; default `"rrf"`. Zep `limit` default = `10`. Zep `scope` default = `"edges"`. Zep `mmr_lambda` default is listed as `-` (no default shown in the table) with the doc note "Required when using `mmr` reranker".

Zep `episode_mentions` semantics, verbatim (line 809/811 of the page):

> "`episode_mentions` reranks edge candidates by how many of the episodes listed in `search_filters.episode_uuids` mention them, from most to least mentioned. Node results are filtered by mention but not reranked by this reranker."
> "**Required parameter**: `search_filters.episode_uuids` - the episode UUIDs to count mentions against. Without it, `episode_mentions` has no effect and results are ranked as if no reranker were specified."

Zep `node_distance`, verbatim: "`node_distance` reranks search results based on graph proximity, prioritizing results that are closer (fewer hops) to a specified center node."

Zep RRF score semantics, verbatim: "RRF scores combine semantic similarity and keyword matching by summing reciprocal ranks (1/rank) from both search methods... Scores don't follow a fixed 0-1 scale".

Zep MMR `mmr_lambda`, verbatim: "**Required parameter**: `mmr_lambda` (0.0-1.0) - Controls the balance between relevance (1.0) and diversity (0.0). **A value of 0.5 provides balanced results.**"

---

# B) EPISODE / INGESTION

## B1. Episode types

### Documented

**Source:** https://help.getzep.com/graphiti/core-concepts/adding-episodes.md — verbatim:

> "Supported episode types:
> * `text`: Unstructured text data
> * `message`: Conversational messages of the format `speaker: message...`
> * `json`: Structured data, processed distinctly from the other types"

> "Episodes represent a single data ingestion event. An `episode` is itself a node, and any nodes identified while ingesting the episode are related to the episode via `MENTIONS` edges."

### `fact_triple` — ⚠️ EXISTS but is NOT on that docs page

The `adding-episodes.md` page lists **only three** types. However the enum has four `[CODE]`:

**Source:** https://raw.githubusercontent.com/getzep/graphiti/main/graphiti_core/nodes.py

```python
class EpisodeType(Enum):
    """
    ...
    Attributes:
    -----------
    message : str
    json : str
    text : str
    """
    message = 'message'
    json = 'json'
    text = 'text'
    fact_triple = 'fact_triple'

    @staticmethod
    def from_str(episode_type: str):
        ...
        if episode_type == 'fact_triple':
            return EpisodeType.fact_triple
```

Note the docstring still lists only three attributes. And the **Zep REST API accepts it**: https://help.getzep.com/sdk-reference/graph/search.md, under `apidata.GraphEpisode`:

> "`source` (enum, optional) — Allowed values: `text`, `json`, `message`, `fact_triple`"

There is a separate **"Adding Fact Triples"** docs page (https://help.getzep.com/graphiti/working-with-data/adding-fact-triples.md) describing `graphiti.add_triplet(source_node, edge, target_node)` — that is a *write API*, documented as: "A 'fact triple' consists of two nodes and an edge between them, where the edge typically contains some fact." It is the closest documented analogue but is not documented as an episode type.

The Zep paper also says three, verbatim: "Episodes can be one of three core types: message, text, or JSON." — https://arxiv.org/html/2501.13956v1 §2.1.

**Verdict: `fact_triple` is a real, live value in code and in the Zep REST enum, but is NOT documented as an episode type in any prose docs page found.**

## B2. `add_episode` API signature

### Documented (docs example)

**Source:** https://help.getzep.com/graphiti/core-concepts/adding-episodes.md

```python
await graphiti.add_episode(
    name="tech_innovation_article",
    episode_body=(...),
    source=EpisodeType.text,
    # A description of the source (e.g., "podcast", "news article")
    source_description="Technology magazine article",
    # The timestamp for when this episode occurred or was created
    reference_time=datetime(2023, 11, 15, 9, 30),
)
```

`add_episode_bulk` usage documented too, with the warning verbatim:

> "Use `add_episode_bulk` only for populating empty graphs or when edge invalidation is not required. The bulk ingestion pipeline does not perform edge invalidation operations."

Also verbatim: "> For large data imports, consider using the `add_episode_bulk` API to add multiple episodes in one call."

### Authoritative full signature `[CODE]`

**Source:** https://raw.githubusercontent.com/getzep/graphiti/main/graphiti_core/graphiti.py

```python
async def add_episode(
    self,
    name: str,
    episode_body: str,
    source_description: str,
    reference_time: datetime,
    source: EpisodeType = EpisodeType.message,
    group_id: str | None = None,
    uuid: str | None = None,
    update_communities: bool = False,
    entity_types: dict[str, type[BaseModel]] | None = None,
    excluded_entity_types: list[str] | None = None,
    previous_episode_uuids: list[str] | None = None,
    edge_types: dict[str, type[BaseModel]] | None = None,
    edge_type_map: dict[tuple[str, str], list[str]] | None = None,
    custom_extraction_instructions: str | None = None,
    saga: str | SagaNode | None = None,
    saga_previous_episode_uuid: str | None = None,
) -> AddEpisodeResults:
```

**Notable: `source` defaults to `EpisodeType.message`, not `text`.** Returns `AddEpisodeResults(episode, episodic_edges, nodes, edges, communities, community_edges)`.

### Documented pipeline steps

The `adding-episodes.md` page does **not** enumerate pipeline steps. The only pipeline description on it is the `MENTIONS`-edge sentence quoted above. Pipeline steps are only described at the code level. `add_episode_bulk`'s docstring `[CODE]` states verbatim:

> "This method performs several steps including:
> - Saving all episodes to the database
> - Retrieving previous episode context for each new episode
> - Extracting nodes and edges from all episodes
> - Generating embeddings for nodes and edges
> - Deduplicating nodes and edges
> - Saving nodes, episodic edges, and entity edges to the knowledge graph"

**Paper description of the construction pipeline** (https://arxiv.org/html/2501.13956v1) §2.2.1, verbatim:

> "During ingestion, the system processes both the current message content and the last n messages to provide context for named entity recognition. For this paper and in Zep's general implementation, **n=4**, providing two complete conversation turns for context evaluation."

> "After extraction, the system embeds each entity name into a **1024-dimensional** vector space. This embedding enables the retrieval of similar nodes through cosine similarity search across existing graph entity nodes. The system also performs a separate full-text search on existing entity names and summaries to identify additional candidate nodes."

§2.2.2, verbatim: "The system performs edge deduplication through a process similar to entity resolution. The hybrid search for relevant edges is constrained to edges existing between the same entity pairs as the proposed new edge."

§2.2.3, verbatim: "The system tracks four timestamps: t′created and t′expired ∈ T′ monitor when facts are created or invalidated in the system, while tvalid and tinvalid ∈ T track the temporal range during which facts held true."

## B3. Entity/edge deduplication, "MIN_SIMILARITY", entity resolution

### "MIN_SIMILARITY"

❌ **Not found.** No constant or docs mention of `MIN_SIMILARITY` anywhere on current `main` or on any docs page. The closest live constants are below.

### Actual dedup constants `[CODE]`

**Source:** https://raw.githubusercontent.com/getzep/graphiti/main/graphiti_core/utils/maintenance/node_operations.py

```python
# Maximum number of nodes to summarize in a single LLM call
MAX_NODES = 30
NODE_DEDUP_CANDIDATE_LIMIT = 15
NODE_DEDUP_COSINE_MIN_SCORE = 0.6
```

**Source:** https://raw.githubusercontent.com/getzep/graphiti/main/graphiti_core/utils/maintenance/dedup_helpers.py

```python
_NAME_ENTROPY_THRESHOLD = 1.5
_MIN_NAME_LENGTH = 6
_MIN_TOKEN_COUNT = 2
_FUZZY_JACCARD_THRESHOLD = 0.9
_MINHASH_PERMUTATIONS = 32
_MINHASH_BAND_SIZE = 4
```

Also from `text_utils` (imported by `node_operations`): `MAX_SUMMARY_CHARS` is used with `truncate_at_sentence` and compared as `len(summary_with_edges) <= MAX_SUMMARY_CHARS * 2` (value not retrieved in this pass).

### The resolution algorithm `[CODE]` — three-stage cascade

`resolve_extracted_nodes` docstring, verbatim: *"Resolve nodes with semantic retrieval first, then deterministic and LLM dedup."*

1. **Semantic candidate retrieval** — `_semantic_candidate_search` runs "direct cosine similarity search per extracted node without reranking" using `node_similarity_search(..., NODE_DEDUP_CANDIDATE_LIMIT, NODE_DEDUP_COSINE_MIN_SCORE)` i.e. limit **15**, min cosine **0.6**.
2. **Deterministic resolution** (`_resolve_with_similarity`) — docstring verbatim: *"Attempt deterministic resolution using exact name hits and fuzzy MinHash comparisons. Exact normalized-name matching runs first for *all* names regardless of length or entropy. The entropy gate only guards the fuzzy (MinHash/LSH) path where short or low-entropy names produce unreliable shingle sets."* Exact normalized-name hit → resolve immediately; >1 exact match → escalate to LLM; low-entropy name → escalate to LLM; otherwise MinHash/LSH + Jaccard ≥ `0.9` → resolve.
3. **LLM dedup** (`_resolve_with_llm`) — prompt `dedupe_nodes.nodes`, response model `NodeResolutions`, with defensive guards for missing/duplicate/out-of-range IDs.

### Documented statements about dedup

**Source:** https://help.getzep.com/graphiti/working-with-data/adding-fact-triples.md — verbatim:

> "When you add a fact triple, Graphiti will attempt to deduplicate your passed in nodes and edge with the already existing nodes and edges in the graph. If there are no duplicates, it will add them as new nodes and edges."

**Source (README):** https://raw.githubusercontent.com/getzep/graphiti/main/README.md — verbatim:

> "Graphiti depends on structured (JSON) output for entity/edge extraction and deduplication, and works best with models and providers that reliably honor it (OpenAI, Anthropic, Gemini)."

**Source (Zep blog):** https://blog.getzep.com/scaling-agent-memory-zep-30x/ — verbatim:

> "**Enhanced deduplication reliability.** We improved entity and relationship deduplication using traditional NLP and algorithmic approaches. Fewer duplicate nodes, better graph quality."

and

> "We replaced expensive LLM calls with Shannon Entropy for information density scoring, TF-IDF for content deduplication, and LSH (Locality-Sensitive Hashing) for fast similarity matching."

**Paper** §2.2.1, verbatim: "Following initial entity extraction, we employ a reflection technique inspired by reflexion to minimize hallucinations and enhance extraction coverage."

## B4. Community detection / `build_communities` / label propagation

### ⚠️ DIRECT CONTRADICTION between docs and code

**Docs say Leiden.** https://help.getzep.com/graphiti/core-concepts/communities.md — verbatim:

> "Communities are determined using the **Leiden algorithm**, which groups strongly connected nodes together."

**Paper says label propagation, explicitly not Leiden.** https://arxiv.org/html/2501.13956v1 §2.3 — verbatim:

> "While our community detection approach builds upon the technique described in GraphRAG, **we employ a label propagation algorithm rather than the Leiden algorithm.** This choice was influenced by label propagation's straightforward dynamic extension, which enables the system to maintain accurate community representations for longer periods as new data enters the graph, delaying the need for complete community refreshes."

**Code says label propagation.** https://raw.githubusercontent.com/getzep/graphiti/main/graphiti_core/utils/maintenance/community_operations.py — verbatim:

```python
        cluster_uuids = label_propagation(projection)
...
def label_propagation(projection: dict[str, list[Neighbor]]) -> list[list[str]]:
    # Implement the label propagation community detection algorithm.
    # 1. Start with each node being assigned its own community
    # 2. Each node will take on the community of the plurality of its neighbors
    # 3. Ties are broken by going to the largest community
    # 4. Continue until no communities change during propagation
```

**Conclusion: the docs page is wrong; the paper and code agree on label propagation.** If you are citing "Leiden", cite the docs page and expect it to be contested; the implementation is label propagation.

### Other documented community facts

**Source:** https://help.getzep.com/graphiti/core-concepts/communities.md — verbatim:

> "In Graphiti, communities (represented as `CommunityNode` objects) represent groups of related entity nodes. Communities can be generated using the `build_communities` method on the graphiti class."
> "```python\nawait graphiti.build_communities()\n```"
> "Communities contain a summary field that collates the summaries held on each of its member entities."
> "Once communities are built, they can also be updated with new episodes by passing in `update_communities=True` to the `add_episode` method. If a new node is added to the graph, we will determine which community it should be added to based on the most represented community of the new node's surrounding nodes. This updating methodology is inspired by the label propagation algorithm for determining communities. However, we still recommend periodically rebuilding communities to ensure optimal grouping. **Whenever the `build_communities` method is called it will remove any existing communities before creating new ones.**"

**Signature `[CODE]`** (https://raw.githubusercontent.com/getzep/graphiti/main/graphiti_core/graphiti.py):
```python
async def build_communities(
    self, group_ids: list[str] | None = None, driver: GraphDriver | None = None
) -> tuple[list[CommunityNode], list[CommunityEdge]]:
```
Docstring verbatim: *"group_ids : list[str] | None — Optional. Create communities only for the listed group_ids. If blank the entire graph will be used."* Body calls `remove_communities(driver, group_ids=group_ids)` first, matching the docs' "remove any existing communities".

**Extra constant not in docs `[CODE]`:** `MAX_COMMUNITY_BUILD_CONCURRENCY = 10` in `community_operations.py`.

**Paper** §2.3, verbatim: "When the system adds a new entity node ni ∈ Ns to the graph, it surveys the communities of neighboring nodes. The system assigns the new node to the community held by the plurality of its neighbors, then updates the community summary and graph accordingly." and "periodic community refreshes remain necessary."

---

# C) LATENCY / PERFORMANCE / SCALABILITY CLAIMS

## C1. "sub-200ms"

**Source:** https://raw.githubusercontent.com/getzep/graphiti/main/README.md — the Zep vs Graphiti table, verbatim:

> "| **Retrieval & performance** | Pre-configured, production-ready retrieval with **sub-200ms performance at scale** | Custom implementation required; performance depends on your setup |"

Same README, verbatim: "Zep manages context graphs at scale, providing governed, **low-latency** context retrieval and assembly for production deployments."

**Source:** https://www.getzep.com/mem0-alternative/ — verbatim:

> "### Sub-200ms retrieval at scale
> Hot graphs held in memory, with vector and BM25 indexes alongside. One call, one ranked answer."

and

> "Retrieval that holds at **sub-200ms** across millions of subjects"

**Source:** https://blog.getzep.com/lies-damn-lies-statistics-is-mem0-really-sota-in-agent-memory/ — callout, verbatim (typo in original):

> "💡 **December 9, 2015:** We've published new LoCoMo scores for Zep: **80% at <200ms latency**. [Learn More]"

**Source:** https://blog.getzep.com/the-retrieval-tradeoff-what-50-experiments-taught-us-about-context-engineering/ — verbatim:

> "**Latency wasn't the binding constraint: p50 retrieval stayed under 200ms across every configuration.**"
> "We've optimized Zep for low-latency use cases such as video and voice agents. Latency stays reasonable across the entire range. Even at 30/30, retrieval p50 is under 200ms."

Their latency table, verbatim:

| Config | Accuracy (mean ± std) | Retrieval p50 (ms) |
|---|---|---|
| 30/30 | 80.32 ± 0.43 | 189 |
| 20/20 | 80.06 ± 0.33 | 241 |
| 15/5 | 77.06 ± 0.41 | 199 |
| 10/2 | 73.72 ± 0.41 | 161 |
| 5/2 | 69.62 ± 0.47 | 149 |

## C2. Incremental updates — "no full re-indexing" / "without full graph recomputation"

**Source:** https://raw.githubusercontent.com/getzep/graphiti/main/README.md — verbatim:

> "The framework supports incremental data updates, efficient retrieval, and precise historical queries **without requiring complete graph recomputation**, making it suitable for developing interactive, context-aware AI applications."

and, verbatim:

> "- **Incremental Graph Construction:** New data integrates immediately without batch recomputation. The graph evolves in real-time as episodes are ingested."

and, in the Graphiti vs GraphRAG table:

> "| **Data Handling** | Batch-oriented processing | Continuous, incremental updates |"

**Source:** https://help.getzep.com/graphiti/getting-started/overview.md — verbatim: "| **Data Handling** | Batch-oriented processing | Continuous, incremental updates |"

**Paper** §2.3, verbatim (the closest thing to "no full re-indexing" in the paper):

> "This choice was influenced by label propagation's straightforward dynamic extension, which enables the system to maintain accurate community representations for longer periods as new data enters the graph, **delaying the need for complete community refreshes.**"

and

> "While this dynamic updating enables efficient community extension as data flows into the system, the resulting communities gradually diverge from those that would be generated by a complete label propagation run. **Therefore, periodic community refreshes remain necessary.**"

⚠️ **Important scoping caveat for citation:** the strongest "no full recomputation" phrasing is a **README/marketing claim**; the **paper explicitly says full community refreshes remain necessary periodically**. Do not attribute "no full re-indexing" to the paper.

The exact string **"no full re-indexing" was NOT found** on any Zep/Graphiti page I fetched. The paper abstract's phrase is different (see C4).

**Source:** https://blog.getzep.com/scaling-agent-memory-zep-30x/ — verbatim, relevant to incremental scaling:

> "We realized we'd been treating LLMs as the solution to every problem, even when simpler approaches would work better. We replaced expensive LLM calls with Shannon Entropy for information density scoring, TF-IDF for content deduplication, and LSH (Locality-Sensitive Hashing) for fast similarity matching. We compressed context before LLM calls by stripping low-signal content and optimized prompts to reduce token overhead. This cut LLM token usage by 50% while keeping costs within our prepurchased inference capacity."

## C3. Scalability — millions of context graphs

**Source:** https://help.getzep.com/context-lake.md — verbatim:

> "The service **manages millions of Context Graphs on Konig**, Zep's graph database service. This count describes the empirical scale of the Zep service. **The count is not a per-account capacity limit or an availability guarantee.**"

(The self-limiting second sentence is important if you quote this.)

**Source:** https://raw.githubusercontent.com/getzep/graphiti/main/README.md — verbatim:

> "Under the hood, Zep is powered by a proprietary graph database — the Context Graph Engine — **built for millions of context graphs with low-latency retrieval**, so production deployments don't require a separate third-party graph database."

> "| **Context graphs** | Manages vast numbers of per-user/entity context graphs with governance | ... |"

> "- **Scalability:** Efficiently manages large datasets with parallel processing, pluggable graph backends, suitable for enterprise workloads."

**Source:** https://www.getzep.com/mem0-alternative/ — verbatim:

> "Zep is built on the Context Lake — **millions of context graphs per deployment**, served back to agents in milliseconds. In production at Fortune 500 deployments."

**Source:** https://blog.getzep.com/scaling-agent-memory-zep-30x/ — verbatim numbers:

> "This summer, our largest enterprise customers landed all at once. **Service usage increased 30x in two weeks.** We went from handling thousands of hourly requests to millions overnight."
> "Context retrieval latency **spiked from a P95 of 200ms to over 2 seconds**. Episode processing crawled to **60 seconds**. LLM costs exploded to **3-5x our provisioned capacity**"
> "Six weeks later... **Graph search now returns results in 150ms (P95), down from 600ms. Context retrieval, which is a retrieval pipeline with many search components, dropped to 200ms (P95). And episode processing latency improved 92%, from around to 4 seconds.** And we cut LLM token usage in half while maintaining **accuracy above 80% on the LongMemEval benchmark, up 10% since our early 2025 paper.**"
> "The result: **65ms P75 search latency** at massive scale, a fraction of the cost, and far lower operational overhead."
> "We restored **99.95%+ uptime under 30x load**"
> "We considered OpenSearch but estimated it would cost over **$50K per month** in infrastructure."

⚠️ Note the source text "episode processing latency improved 92%, from around to 4 seconds" is **garbled in the original** (a number is missing after "from around"). Quote as-is or avoid.

## C4. Peer-reviewed / paper numbers (arXiv 2501.13956)

**Source:** https://arxiv.org/abs/2501.13956 (abstract), verbatim:

> "In the DMR benchmark, which the MemGPT team established as their primary evaluation metric, Zep demonstrates superior performance (**94.8% vs 93.4%**). Beyond DMR, Zep's capabilities are further validated through the more challenging LongMemEval benchmark... In this evaluation, Zep achieves substantial results with **accuracy improvements of up to 18.5% while simultaneously reducing response latency by 90%** compared to baseline implementations."

**Table 1, Deep Memory Retrieval** (https://arxiv.org/html/2501.13956v1), verbatim:

| Memory | Model | Score |
|---|---|---|
| Recursive Summarization† | gpt-4-turbo | 35.3% |
| Conversation Summaries | gpt-4-turbo | 78.6% |
| MemGPT† | gpt-4-turbo | 93.4% |
| Full-conversation | gpt-4-turbo | 94.4% |
| Zep | gpt-4-turbo | 94.8% |
| Conversation Summaries | gpt-4o-mini | 88.0% |
| Full-conversation | gpt-4o-mini | 98.0% |
| Zep | gpt-4o-mini | 98.2% |

**Table 2, LongMemEval-s**, verbatim:

| Memory | Model | Score | Latency | Latency IQR | Avg Context Tokens |
|---|---|---|---|---|---|
| Full-context | gpt-4o-mini | 55.4% | 31.3 s | 8.76 s | 115k |
| Zep | gpt-4o-mini | 63.8% | 3.20 s | 1.31 s | 1.6k |
| Full-context | gpt-4o | 60.2% | 28.9 s | 6.01 s | 115k |
| Zep | gpt-4o | 71.2% | 2.58 s | 0.684 s | 1.6k |

Text verbatim: "Using gpt-4o-mini, Zep achieved a **15.2% accuracy improvement** over the baseline, while gpt-4o showed an **18.5% improvement**."

**Table 3, question-type breakdown** (both tables in paper and https://blog.getzep.com/state-of-the-art-agent-memory/), verbatim deltas for gpt-4o: single-session-preference `20.0% → 56.7%` (**184%↑**); single-session-assistant `94.6% → 80.4%` (**17.7%↓**); temporal-reasoning `45.1% → 62.4%` (**38.4%↑**); multi-session `44.3% → 57.9%` (**30.7%↑**); knowledge-update `78.2% → 83.3%` (**6.52%↑**); single-session-user `81.4% → 92.9%` (**14.1%↑**).

Paper methodology caveat, verbatim: "We performed testing using a consumer laptop from a residential location in Boston, MA, connecting to Zep's service hosted in AWS us-west-2. **This distributed architecture introduced additional network latency when evaluating Zep's performance**, though this latency was not present in our baseline evaluations."

**Blog companion** https://blog.getzep.com/state-of-the-art-agent-memory/ — verbatim: "Setting a new standard for agent memory with **up to 100% accuracy gains and 90% lower latency**." and "Evaluation with Zep utilized, on average, **less than 2% of the baseline tokens** while offering latency reductions of an order of magnitude."

## C5. Current published benchmark page (2026)

**Source:** https://www.getzep.com/research/ — verbatim:

> "LoCoMo — **94.7%** accuracy / Retrieval latency **155 ms** / Context size **5,760 tokens**"
> "LongMemEval — **90.2%** accuracy / Retrieval latency **162 ms** / Context size **4,408 tokens**"
> "LongMemEval ... Accuracy **90.2%** / **451 / 500 correct** / Retrieval latency **104/162ms** p50 / p95 / Median context **4,408 tokens** per question, end-to-end"
> "LoCoMo ... Accuracy **94.7%** / **1,459 / 1,540 correct** / Retrieval latency **87/155ms** p50 / p95 / Median context **5,760 tokens** per question, end-to-end"

Accuracy by question type (LongMemEval), verbatim: Single-session assistant 96.4%; Single-session user 94.3%; Knowledge update 93.6%; Temporal reasoning 90.2%; Single-session preference 90.0%; Multi-session 83.5%.
By category (LoCoMo), verbatim: Single-hop 96.4% (646/670); Temporal 95.6% (311/325); Multi-hop 94.0% (304/323); Open-domain 79.2% (175/221).

Auto search, verbatim: "Accuracy **86.5%** single API call, no tuning / Retrieval latency **115/173ms** p50 / p95 / Median context **2,680 tokens** ↓ 53% smaller".

**Methodology block, verbatim:** "Reader: `gpt-5.4` (reasoning = medium). Judge: `gpt-5.4` with chain-of-thought grading. Multi-scope retrieval depth: **20 edges, 10 nodes, 10 episodes, 5 thread summaries, 5 observations**, cross-encoder reranking. Auto search at `max_characters=10000`. Run on 1,540 LoCoMo questions and 500 LongMemEval questions. **0 failed tests on either benchmark.**"

Architecture claim, verbatim: "Hot graphs are held in memory as adjacency lists and CSR matrices, with vector and BM25 indexes alongside. One query returns one ranked answer across every retrieval signal." → https://www.getzep.com/research/

## C6. Product/roadmap positioning claims

**Source:** https://help.getzep.com/graphiti/getting-started/overview.md — verbatim:

> "- **Hybrid Search:** Combines vector similarity, BM25 full-text, and graph traversal into a single ranked answer, **with no LLM-in-the-loop reranking.** Results can be reranked by distance from a central node e.g. \"Kendra\"."

⚠️ Note this sits in tension with the `cross_encoder` reranker options (OpenAI/Gemini/BGE LLM-based rerankers documented on `searching.md`) and with `COMBINED_HYBRID_SEARCH_CROSS_ENCODER` being the `search_()` default. Worth flagging as another docs inconsistency.

**Source:** https://raw.githubusercontent.com/getzep/graphiti/main/README.md — verbatim:

> "Graphiti's ingestion pipelines are designed for high concurrency. By default, concurrency is set low to avoid LLM Provider 429 Rate Limit Errors... Concurrency controlled by the `SEMAPHORE_LIMIT` environment variable. By default, `SEMAPHORE_LIMIT` is set to **`10`** concurrent operations"

README GraphRAG comparison table, verbatim: "| **Query Latency** | Seconds to tens of seconds | **Typically sub-second latency** |"

README, verbatim on model defaults: "`OpenAIGenericClient` ... is optimized for local models with a higher default max token limit (**16K vs 8K**)". Gemini reranker default model, verbatim: "The Gemini reranker uses the `gemini-2.5-flash-lite` model by default".

Requirements, verbatim: "Neo4j 5.26 / FalkorDB 1.1.2 / Amazon Neptune Database Cluster or Neptune Analytics Graph + Amazon OpenSearch Serverless collection (serves as the full text search backend) / Kuzu 0.11.2 (**deprecated**)"; "**Kuzu is deprecated** and will be removed in a future release — the upstream Kuzu project is no longer maintained."

---

# D) ZEP vs MEM0 / COMPETITOR COMPARISONS PUBLISHED BY ZEP

Zep runs a dedicated "alternative" page family. Full list from https://www.getzep.com/sitemap.xml:
`/mem0-alternative/`, `/letta-alternative/`, `/cognee-alternative/`, `/supermemory-alternative/`, `/hydradb-alternative/`, `/aws-agentcore-memory-alternative/`, `/vertex-ai-memory-bank-alternative/`, `/vectorize-hindsight-alternative/`; plus the docs `https://help.getzep.com/mem0-to-zep` and `https://help.getzep.com/v2/mem0-to-zep`.

## D1. https://www.getzep.com/mem0-alternative/ — headline comparison

Verbatim framing: "Zep and mem0 run on the same long-running memory benchmarks: **LoCoMo (1,540 questions) and LongMemEval (500 questions). Same reader prompts. Same judge.** The retrieval stack is what varies."

**LoCoMo block, verbatim:**
> "1,540 questions · long-running memory
> Accuracy **94.7%** Zep accuracy (Zep 94.7% / mem0 91.6%)
> Retrieval **35×faster** at p50 · **87 ms vs 3,060 ms** (Zep 87 ms / mem0 3,060 ms)
> Context **17% smaller** · **5,760 vs 6,956 tokens** (Zep 5,760 tok / mem0 6,956 tok)"
> "**Zep wins every category:** multi-hop, temporal, open-domain, single-hop."

**LongMemEval block, verbatim:**
> "500 questions · top-k retrieval
> Accuracy **90.2%** Zep accuracy (Zep 90.2% / mem0 **90.4% top_50**)
> Retrieval **24×faster** at p50 · **104 ms vs 2,470 ms** (Zep 104 ms / mem0 2,470 ms)
> Context **35% smaller** · **4,408 vs 6,787 tokens** (Zep 4,408 tok / mem0 6,787 tok)"
> "**Same accuracy at comparable retrieval depth, on a third less context and an order of magnitude faster.**"

Baseline provenance, verbatim: "Mem0 baseline numbers published by mem0 at github.com/mem0ai/memory-benchmarks."

Other verbatim claims on that page: "Retrieval that holds at sub-200ms across millions of subjects"; "**Zep is built on the Context Lake — millions of context graphs per deployment**, served back to agents in milliseconds. In production at Fortune 500 deployments."

Their "when to choose" split, verbatim:
> "**Choose Zep when you need:** Facts that change over time, with point-in-time queries / Business data integrated alongside chat — CRM, support, billing, events / Custom entities and relationships specific to your domain / Retrieval that holds at sub-200ms across millions of subjects / Enterprise deployment with SOC 2 Type II, HIPAA, and BYOC"
> "**Stay with mem0 when:** Basic chat-history memory is enough / You don't need temporal validity or business data integration / You're prototyping and quick setup matters more than production characteristics"

## D2. https://blog.getzep.com/lies-damn-lies-statistics-is-mem0-really-sota-in-agent-memory/ (Zep's rebuttal to Mem0's LoCoMo paper)

Verbatim: "Mem0 recently published research claiming to be the State-of-the-Art in Agent Memory, besting Zep. In reality, **Zep outperforms Mem0 by 10%** on their chosen benchmark."

Verbatim correction notice: "**Correction:** In an earlier version of this article, we erred in how we calculated Zep's LoCoMo score. We've updated the article to reflect Zep's corrected result is **75.14% +/- 0.17**, with Zep outperforming Mem0 by 10%."

Verbatim: "Our evaluation shows Zep achieving an **75.14% +/- 0.17** J score, significantly outperforming Mem0's best configuration (_Mem0 Graph_) by approximately **10% relative improvement**. This starkly contrasts with the **65.99% score reported for Zep in the Mem0 paper**".

Search latency, verbatim: "Zep, when configured correctly for concurrent searches, achieves a **p95 search latency of 0.632 seconds**. This is faster than the **0.778 seconds** reported by Mem0 for Zep (likely inflated due to their sequential search implementation) and slightly faster than **Mem0's graph search latency (0.657s)**." and "While Mem0's base configuration shows a lower search latency (**0.200s**), it's important to note this isn't an apples-to-apples comparison; the base Mem0 uses a simpler vector store / cache..."

Methodology caveat verbatim: "_Note: Zep's latency was measured from AWS us-west-2 with transit through a NAT setup._"

Zep's three cited implementation errors in Mem0's eval, verbatim:
> "1. **Incorrect User Model:** Mem0 utilized a user graph structure designed for single user-assistant interactions but assigned the user role to _both_ participants."
> "2. **Improper Timestamp Handling:** Timestamps were passed by appending them to messages, rather than using Zep's dedicated created_at field."
> "3. **Sequential vs. Parallel Searches:** Searches were performed sequentially instead of in parallel, artificially inflating Zep's reported search latency in Mem0's results."

Baseline figure quoted by Zep from Mem0's own paper, verbatim: "Mem0's own results show their system being outperformed by a simple full-context baseline (feeding the entire conversation to the LLM), which achieved a J score of **~73%**, compared to Mem0's best score of **~68%**."

LoCoMo length, verbatim: "The conversations in LoCoMo average around **16,000-26,000 tokens**."

## D3. https://help.getzep.com/mem0-to-zep — ⚠️ NO comparison numbers

Verbatim: "This guide maps common Mem0 operations to Zep. Review the current Mem0 API before you export data because Mem0 can change its data model."

The page is a pure **migration/API-mapping guide** — it contains **no accuracy, latency, token, or benchmark numbers, and no pejorative claims about Mem0**. Its operation-mapping table, verbatim:

| Mem0 operation | Zep Python method | Notes |
|---|---|---|
| Add conversation messages | `client.thread.add_messages(thread_id=..., messages=[...])` | "Create the user and thread first. Preserve message order." |
| Add a business record | `client.graph.add(user_id=..., type="json", data=...)` | "Serialize JSON data to a string. Use `type=\"text\"` for text." |
| Retrieve context for a conversation | `client.thread.get_user_context(thread_id=...)` | "Returns an assembled Context Block in `.context`." |
| Search a user's graph | `client.graph.search(user_id=..., query=..., scope="auto")` | "Use another scope when you need one result type." |
| List graph nodes | `client.graph.node.get_by_user_id(user_id=...)` | "Supports pagination and filters." |
| List graph edges | `client.graph.edge.get_by_user_id(user_id=...)` | "Each edge contains a temporal fact." |
| Delete an episode | `client.graph.episode.delete(uuid_=...)` | "Deletion also removes data derived only from that episode." |

Verbatim guidance: "Do not translate each Mem0 memory into a Zep fact. Ingest the source messages or records so Zep can build the graph."

⚠️ Note the v3 page (`help.getzep.com/mem0-to-zep`) is **not** the same as the older v2 page `help.getzep.com/v2/mem0-to-zep` — the v2 index describes it as "How to migrate from Mem0 to Zep". I did not fetch the v2 body; **the v2 page may contain older comparison numbers — unverified.**

## D4. Paper-level Mem0 comparison

The arXiv paper (2501.13956, Jan 2025) predates Mem0's LoCoMo paper and **does not name Mem0 at all** in the abstract or in the sections I fetched. Its comparison target is **MemGPT** (Letta). Verbatim: "outperforms the current state-of-the-art system, **MemGPT**, in the Deep Memory Retrieval (DMR) benchmark" and "we attempted to evaluate MemGPT using the LongMemEval dataset... we were unable to achieve successful question responses using this approach."

---

# CONTRADICTIONS / VERSION DRIFT (do not cite these as settled)

1. **Community detection algorithm.** Docs `communities.md` says "**Leiden algorithm**"; paper §2.3 says "**label propagation algorithm rather than the Leiden algorithm**"; code implements `label_propagation`. → **Docs are wrong.**
2. **Recipe count.** Docs say "The **15 recipes**" then list **16**; code defines **16**.
3. **`SearchConfig` field count.** Docs say "*four fields: one for the limit, and three more configs for each of edges, nodes, and communities*"; code has **four configs** (edge, node, **episode**, community) **plus** `limit` **plus** `reranker_min_score` = six.
4. **"no LLM-in-the-loop reranking"** (Graphiti `overview.md`) vs. the `cross_encoder` reranker family and `COMBINED_HYBRID_SEARCH_CROSS_ENCODER` being the `search_()` default.
5. **LoCoMo score for Zep varies by source**: 75.14% ± 0.17 (blog rebuttal, 2025); ~80% (retrieval-tradeoff post, Dec 2025); 94.7% (research page, 2026); 63.8% / 71.2% are LongMemEval, not LoCoMo. Different harnesses/runs — never mix them.
6. **Reranker string naming.** Graphiti Python enum value is `reciprocal_rank_fusion`; the Zep REST API value is `rrf`. `node_distance`, `episode_mentions`, `mmr`, `cross_encoder` match in both.
7. **`fact_triple`** is a live enum value in code and in the Zep REST `GraphEpisode.source` enum, but the Graphiti docs page lists only three episode types and the paper says "three core types".
8. **Blog date typo** in the mem0 rebuttal callout: "December 9, 2015" (article is dated 06 May 2025, updated 03 Jun 2026).
9. **Garbled sentence** in the 30x scaling post: "episode processing latency improved 92%, from around to 4 seconds."
10. **`maximal_marginal_relevance` uses `(mmr_lambda - 1) * max_sim`**, not the textbook `(1 - mmr_lambda) * max_sim`. Either intentional or a bug — do not describe it as standard MMR without checking.

---

# NOT FOUND / COULD NOT VERIFY

| Sought | Status |
|---|---|
| `MIN_SIMILARITY` constant | ❌ **Not found** anywhere in Graphiti `main` or docs. Nearest live constants: `DEFAULT_MIN_SCORE = 0.6`, `NODE_DEDUP_COSINE_MIN_SCORE = 0.6`. |
| `SearchFilters.node_uuids` | ❌ **Does not exist** in current `main`. Zep platform has `connected_node_uuids` / `source_node_uuids` / `target_node_uuids` / `episode_uuids` instead. |
| Temporal filter literal syntax `[["2024-01-01", "2024-06-01"]]` (bare ISO string pairs) | ❌ **Not stated on any page.** Both Graphiti Python and the Zep REST reference use `list[list[DateFilter]]`. The REST doc's example is `[[{">", date1}, {"<", date2}], [{"=", date3}]]`. |
| `rank_const` documented in prose | ❌ Only in source code (`def rrf(results, rank_const=1, min_score=0)`). No prose page explains or names it. |
| Any prose page stating `sim_min_score`, `mmr_lambda`, `bfs_max_depth`, `reranker_min_score`, or `limit` defaults **for Graphiti** | ❌ **Not stated on this page** — `searching.md` never gives these numbers. They exist only in `search_config.py` / `search_utils.py`. (The **Zep platform** page *does* tabulate `limit=10`, `scope="edges"`, `reranker="rrf"`, `max_characters=2500`, `return_raw_results=false`.) |
| `bfs_max_depth` as a Zep platform search parameter | ❌ Not in the Zep "Configurable Parameters" table; the only BFS knob documented by Zep is `bfs_origin_node_uuids` (max 5 UUIDs). |
| Exact string "no full re-indexing" | ❌ Not found on any Zep/Graphiti page fetched. Closest verbatim: "without requiring complete graph recomputation" (README) and the paper's "delaying the need for complete community refreshes" (which also says refreshes remain necessary). |
| `https://help.getzep.com/graphiti/graphiti/search.md` | ❌ 404 / not in index. Live page: `.../graphiti/working-with-data/searching.md`. |
| `https://docs.getzep.com/llms.txt`, `https://docs.getzep.com/sitemap.xml` | ❌ Cross-origin redirect to `help.getzep.com`; not retrievable under that host. |
| `help.getzep.com/sitemap.xml` | ❌ Fetch/pwsh failed (pwsh: SSL error). Not retrieved. Worked around via `llms.txt` + `/graphiti/llms.txt` + `/v2/llms.txt`. |
| `help.getzep.com/llms-full.txt` | ❌ Not retrieved (large; `web_fetch` truncates). Substituted per-page `.md` fetches. |
| `mcp_server/README.md` and `server/README.md` contents | ❌ Not fetched (README.md links them as "detailed setup instructions"; contents unverified — the main README only summarises: "Episode management (add, retrieve, delete) / Entity management and relationship handling / Semantic and hybrid search capabilities / Group management for organizing related data / Graph maintenance operations"). |
| `examples/quickstart/*.py` actual source | ❌ Not fetched; only `examples/quickstart/README.md`. |
| `MAX_SUMMARY_CHARS` numeric value | ❌ Not verified (imported from `graphiti_core/utils/text_utils.py`, not fetched). |
| v2 Mem0 comparison page body (`help.getzep.com/v2/mem0-to-zep`) | ⚠️ Index entry confirmed; **body not fetched**. May contain older comparison numbers. |
| Latency claims on `www.getzep.com/pricing/` or `/enterprise/` (SLA numbers, p95 commitments) | ❌ Not fetched. |
| Graphiti `graph-namespacing.md`, `crud-operations.md`, `custom-entity-and-edge-types.md`, `quick-start.md`, MCP server page | ❌ Not fetched in this pass (exist in the index; contents unverified for search/ingestion parameters). |
| Any Graphiti docs page documenting `SearchFilters` at all | ❌ **Not stated on this page** — `searching.md` mentions filters only in passing via `graphiti.search_()`; there is no Graphiti-docs filter reference page in the Graphiti section index. |
