# Cache Skill

## Purpose

This skill defines how BastehYab stores, validates, refreshes, reads, expires, and recovers locally cached package data.

It applies primarily to:

```text
src-tauri/src/cache/
```

and cache-related orchestration, persistence, serialization, recovery, and tests.

Read this skill before:

* implementing local package caching;
* modifying cache serialization;
* changing refresh behavior;
* changing cache expiration;
* handling operator refresh failures;
* implementing last-known-good fallback;
* changing cache schema versions;
* implementing atomic persistence;
* adding stale-data behavior;
* changing startup loading;
* introducing cache cleanup;
* modifying per-operator refresh state.

This skill supplements:

```text
AGENTS.md
DESIGN.md
skills/collectors/SKILL.md
skills/normalization/SKILL.md
skills/recommendations/SKILL.md
```

Repository-wide rules in `AGENTS.md` and architectural decisions in `DESIGN.md` take precedence.

---

# 1. Cache Mission

BastehYab retrieves package information directly from official operator infrastructure.

Those sources may be:

* slow;
* temporarily unavailable;
* rate-limited;
* malformed;
* partially changed;
* independently unavailable.

The cache exists to make the application resilient to these conditions.

Conceptually:

```text
Official Operators
       │
       ▼
   Collectors
       │
       ▼
  Normalizers
       │
       ▼
   Validation
       │
       ▼
  Valid Dataset
       │
       ▼
      Cache
       │
       ▼
 Recommendations / UI
```

The cache stores the last trustworthy normalized package dataset.

---

# 2. Cache Is Not the Source of Truth

Official operator sources remain the authoritative upstream source.

The cache is a local snapshot.

Correct mental model:

```text
Official source
    ↓
current observation
    ↓
validated normalized snapshot
    ↓
local cache
```

Do not treat cached data as permanently authoritative.

---

# 3. Last-Known-Good Principle

The central cache rule is:

> Never replace known-good cached data with data that has not successfully passed the collection, normalization, and validation pipeline.

Correct:

```text
Existing healthy cache
        │
        ▼
Refresh attempt
        │
        ├── success ──> replace cache
        │
        └── failure ──> keep existing cache
```

Incorrect:

```text
refresh started
    ↓
delete old cache
    ↓
fetch failed
    ↓
no data
```

Never destroy usable data before replacement data is known to be valid.

---

# 4. Per-Operator Isolation

Cache package datasets independently per operator.

Conceptually:

```text
cache/
├── irancell
├── mci
├── rightel
└── samantel
```

Exact on-disk representation may differ.

The important invariant is logical isolation.

If Rightel fails:

```text
Irancell → fresh
MCI      → fresh
Rightel  → stale cached
Samantel → fresh
```

Do not invalidate the entire application dataset.

---

# 5. Failure Isolation

Operator failures must remain isolated whenever possible.

Example:

```text
Refresh all
   │
   ├── Irancell ✓
   ├── MCI ✓
   ├── Rightel ✗
   └── Samantel ✓
```

Result:

```text
Irancell → new cache
MCI      → new cache
Rightel  → previous cache
Samantel → new cache
```

Do not implement refresh as one all-or-nothing transaction across unrelated operators.

---

# 6. Cache Unit

The primary cache unit should be:

```text
OperatorSnapshot
```

Conceptually:

```rust
struct OperatorSnapshot {
    operator: Operator,
    packages: Vec<InternetPackage>,
    metadata: CacheMetadata,
}
```

This allows independent:

* refresh;
* freshness tracking;
* failure recovery;
* replacement;
* diagnostics.

---

# 7. Cache Metadata

Conceptually:

```rust
struct CacheMetadata {
    schema_version: u32,
    fetched_at: DateTime<Utc>,
    stored_at: DateTime<Utc>,
    source: Operator,
}
```

Additional metadata may be added when it has clear operational value.

Potential examples:

```text
package_count
normalizer_version
source_fingerprint
```

Do not turn metadata into an uncontrolled collection of implementation details.

---

# 8. fetched_at vs stored_at

These timestamps have different meanings.

```text
fetched_at
```

means:

> When the source data was collected.

```text
stored_at
```

means:

> When the resulting snapshot was successfully committed to local cache.

They may be close but are not semantically identical.

Do not use cache file modification time as a substitute for `fetched_at`.

---

# 9. UTC Timestamps

Persist absolute cache timestamps in UTC.

Do not persist locale-dependent strings such as:

```text
7/8/2026 18:30
```

Prefer stable machine-readable serialization.

Display localization belongs to the UI.

---

# 10. Cache Stores Normalized Data

The primary package cache should contain normalized domain objects.

Correct:

```text
InternetPackage[]
```

Not:

```text
raw Irancell response
raw MCI HTML
raw Rightel JSON
raw Samantel page
```

Raw fixtures belong to tests/debugging, not the production package cache.

---

# 11. Why Cache Normalized Data

Caching normalized data means the application can immediately use cached packages for:

```text
filtering
comparison
recommendations
UI
```

without re-running operator-specific parsing on every startup.

It also keeps volatile upstream formats outside the persistent application domain.

---

# 12. Cache Does Not Normalize

The cache layer must not interpret operator data.

It should not:

* parse prices;
* convert GB;
* classify night traffic;
* infer SIM type;
* determine package kind.

Those responsibilities belong to normalization.

Cache receives already validated normalized objects.

---

# 13. Cache Does Not Recommend

Do not persist:

```text
best_value
cheapest
most_data
recommendation_score
```

as authoritative package-cache state.

Recommendations are derived from the currently available normalized dataset.

Correct:

```text
cache
  ↓
packages
  ↓
recommendations
```

---

# 14. Refresh Pipeline

Recommended refresh flow:

```text
Start refresh
     │
     ▼
Collect raw data
     │
     ▼
Normalize
     │
     ▼
Validate
     │
     ▼
Assess dataset health
     │
     ├── unhealthy → keep old cache
     │
     ▼
Create candidate snapshot
     │
     ▼
Persist atomically
     │
     ▼
Publish new snapshot
```

Do not mutate active cached state incrementally while collection is still running.

---

# 15. Candidate Snapshot

Newly fetched data should initially be treated as a candidate.

Conceptually:

```text
current_snapshot
candidate_snapshot
```

Only after the candidate passes all required checks may it replace the current snapshot.

This protects against partial or structurally broken refreshes.

---

# 16. Atomic Replacement

Cache writes must be atomic from the application's perspective.

Never write directly over the only valid cache file in a way that can leave it half-written after:

* application crash;
* OS shutdown;
* disk error;
* process termination.

Preferred conceptual approach:

```text
serialize new snapshot
        ↓
write temporary file
        ↓
flush/finish write
        ↓
validate if necessary
        ↓
atomic rename/replace
```

Exact implementation should use safe platform-appropriate filesystem behavior.

---

# 17. Temporary Files

Temporary cache files must not be treated as valid snapshots during startup.

Example:

```text
rightel.json.tmp
```

must never outrank:

```text
rightel.json
```

unless a documented recovery protocol explicitly validates and promotes it.

Normally incomplete temporary files should be ignored or cleaned up.

---

# 18. Never Delete Before Replace

Avoid:

```rust
remove_file(cache_path)?;
write_new_cache(cache_path)?;
```

If the second operation fails, the good cache is lost.

Replacement should preserve the previous valid snapshot until the new one is safely committed.

---

# 19. Serialization

Use a stable explicit serialization format.

JSON is acceptable for the initial application because:

* datasets are small;
* debugging is easy;
* migrations are understandable;
* human inspection is useful.

Do not introduce a database merely for cache persistence unless future requirements justify it.

---

# 20. Cache Schema Version

Every persistent cache format must have an explicit schema version.

Conceptually:

```json
{
  "schema_version": 1,
  "operator": "irancell",
  "fetched_at": "...",
  "stored_at": "...",
  "packages": []
}
```

Do not rely on application version alone.

---

# 21. Why Schema Version Exists

The normalized domain may evolve.

Examples:

```text
new allowance type
changed enum representation
new validity model
different money structure
new availability semantics
```

Without versioning, old cache files may deserialize incorrectly or acquire unintended meanings.

---

# 22. Version Is Semantic

Increment cache schema version when persisted representation becomes incompatible.

Do not increment it for unrelated application changes.

Examples requiring consideration:

```text
field renamed
enum representation changed
required field added
serialized meaning changed
type changed
```

---

# 23. Unknown Future Version

If the application encounters:

```text
schema_version > supported_version
```

do not attempt to interpret the file optimistically.

Treat it as incompatible.

The application should:

```text
ignore incompatible snapshot
attempt fresh refresh
```

without crashing.

---

# 24. Older Version

For:

```text
schema_version < current_version
```

choose explicitly between:

```text
migration
```

or:

```text
safe invalidation + refresh
```

For small internet-package caches, safe invalidation may be preferable to complex migration machinery.

Do not add migrations unless preserving cache materially improves UX.

---

# 25. Cache Invalidation Is Safe

The cache is reproducible from official sources.

Therefore deleting an incompatible cache is acceptable when necessary.

This differs from a user database.

Do not build database-grade migration complexity for disposable cache data without a clear reason.

---

# 26. But Do Not Delete Prematurely

Although cache is reproducible, upstream sources may currently be unavailable.

If an old cache can still be safely read and semantically understood, keeping it may be better than immediately deleting it.

Schema compatibility policy should balance:

```text
correctness
resilience
implementation complexity
```

Never reinterpret incompatible data merely to avoid a refresh.

---

# 27. Freshness

Each operator snapshot has independent freshness.

Conceptually:

```text
age = now - fetched_at
```

Freshness must not be based on:

* application startup time;
* UI render time;
* filesystem modification time.

---

# 28. Freshness Policy

Use centralized configuration.

Conceptually:

```text
fresh_for
stale_after
```

or a simpler TTL if sufficient.

Do not scatter hard-coded durations throughout the application.

---

# 29. Fresh vs Stale

A useful conceptual model:

```text
Fresh
Stale
Missing
```

Potential future extension:

```text
Expired
```

if product semantics require distinguishing old-but-usable from too-old-to-trust.

Keep the initial model as simple as possible while preserving required behavior.

---

# 30. Stale Does Not Mean Invalid

A stale cache may still be useful.

Example:

```text
cached 6 hours ago
operator currently unreachable
```

The package list may still be far more useful than showing nothing.

Do not automatically discard stale data solely because TTL elapsed.

---

# 31. Stale-While-Revalidate

Preferred startup behavior:

```text
load cached data
      ↓
show usable data quickly
      ↓
if stale, refresh in background/application flow
      ↓
replace when successful
```

The user should not necessarily wait for all operators before seeing package data.

Exact UI orchestration is defined elsewhere, but cache APIs should support this model.

---

# 32. Fresh Cache

If cache is still considered fresh, startup may use it immediately without unnecessary network requests.

Whether a manual refresh bypasses freshness is a separate explicit policy.

---

# 33. Manual Refresh

When the user explicitly requests refresh:

```text
Refresh
```

the application should normally attempt operator collection even if cache is currently fresh.

Manual refresh expresses explicit intent to obtain current information.

Do not merely return the existing cache and pretend a refresh occurred.

---

# 34. Automatic Refresh

Automatic refresh may respect freshness TTL.

Conceptually:

```text
if fresh:
    use cache

if stale:
    use cache
    attempt refresh

if missing:
    attempt refresh
```

Do not create constant background polling unless required by the product.

---

# 35. "Live" Data Semantics

BastehYab aims to provide current package information.

This does not require continuous requests every few seconds.

Operator package catalogs change relatively infrequently.

"Current" should mean:

```text
refreshable from official sources
+
clear freshness
+
last-known-good fallback
```

not:

```text
hammer every operator continuously
```

---

# 36. Avoid Aggressive Polling

Do not repeatedly fetch operator package catalogs at very short intervals.

This would:

* waste bandwidth;
* increase operator load;
* increase rate-limit risk;
* increase blocking risk;
* provide little user benefit.

Use intentional refresh policy.

---

# 37. No Server Dependency

Cache is entirely local.

Do not:

* upload snapshots;
* synchronize cache through BastehYab servers;
* require cloud storage;
* send package history to third parties.

BastehYab has no required central backend for package caching.

---

# 38. Cache Location

Use the operating system's appropriate application cache/data directory through Tauri-supported path facilities.

Do not store production cache relative to:

```text
current working directory
repository root
executable directory
```

Those locations may be unwritable or unstable.

---

# 39. No Administrator Requirement

Cache storage must work under normal user permissions.

BastehYab must not require administrator/root privileges merely to persist package data.

---

# 40. Portable Assumptions

Do not assume:

```text
C:\...
/home/...
```

in cache code.

Use platform path APIs.

Even if the first target is Windows, keep filesystem logic portable where practical.

---

# 41. File Names

Cache filenames should be stable and safe.

Example:

```text
irancell.json
mci.json
rightel.json
samantel.json
```

Do not derive filenames directly from untrusted upstream strings.

---

# 42. Cache Directory Ownership

Only BastehYab-controlled cache files should be managed by cache cleanup logic.

Never recursively delete arbitrary parent directories.

Never assume everything inside a broad OS cache directory belongs to BastehYab.

---

# 43. Startup Loading

On startup:

```text
locate cache directory
      ↓
load each operator independently
      ↓
validate envelope/version
      ↓
deserialize
      ↓
validate cached domain invariants
      ↓
publish usable snapshots
```

One corrupt operator file must not prevent loading other operators.

---

# 44. Corrupt Cache

If a cache file contains:

* malformed JSON;
* truncated content;
* invalid schema;
* impossible domain values;

treat that operator cache as unusable.

Do not crash the entire application.

Attempt a fresh operator refresh when appropriate.

---

# 45. Corrupt Cache Isolation

Example:

```text
irancell.json ✓
mci.json      ✓
rightel.json  corrupt
samantel.json ✓
```

Result:

```text
Irancell available
MCI available
Samantel available
Rightel refresh attempted
```

Do not discard all four.

---

# 46. Cached Data Is Still Untrusted Input

Although BastehYab wrote the cache originally, files on disk may be:

* manually edited;
* corrupted;
* truncated;
* replaced;
* produced by another application version.

Deserialize defensively.

Do not use `unwrap()` on persistent cache content.

---

# 47. Validate After Deserialize

Successful JSON parsing does not guarantee semantic validity.

Example:

```json
{
  "price": 0,
  "data": 0
}
```

may deserialize correctly but still violate current domain expectations.

Run appropriate domain/cache validation after deserialization.

---

# 48. Cache Envelope

Prefer a top-level envelope rather than serializing a bare package array.

Conceptually:

```rust
struct CacheEnvelope {
    schema_version: u32,
    operator: Operator,
    fetched_at: DateTime<Utc>,
    stored_at: DateTime<Utc>,
    packages: Vec<InternetPackage>,
}
```

This supports validation and future evolution.

---

# 49. Operator Consistency

A cache envelope for:

```text
operator = Rightel
```

must not contain packages claiming:

```text
operator = Irancell
```

Treat cross-operator inconsistencies as cache corruption.

Do not silently rewrite package operators while loading.

---

# 50. Duplicate Package IDs

A snapshot should not contain duplicate canonical package IDs.

If duplicates appear:

* investigate during candidate validation;
* reject or deterministically resolve only when source semantics justify it.

Do not silently let `HashMap` overwrite one package and hide the issue.

---

# 51. Empty Snapshot

An empty package list is not automatically a valid refresh.

For known operators normally exposing packages:

```text
previous cache: 40 packages
new response: 0 packages
```

is suspicious.

Do not overwrite healthy cache with empty data merely because the HTTP request succeeded.

---

# 52. Dataset Health

Before cache replacement, assess dataset health.

Possible signals:

```text
raw record count
normalized count
rejected count
final package count
previous package count
```

Health assessment belongs to refresh orchestration/domain validation, but cache replacement must require an approved candidate.

---

# 53. Suspicious Package Drop

Example:

```text
previous: 40
candidate: 2
```

may indicate:

* parser breakage;
* pagination failure;
* upstream redesign;
* authentication failure returning partial data.

Do not automatically overwrite the 40-package cache.

---

# 54. No Universal Drop Threshold

Avoid blindly defining:

```text
if new_count < old_count * 0.8:
    reject
```

for every operator without evidence.

Some operators may legitimately remove many packages.

Dataset-health rules should combine context and operator knowledge.

A conservative drop heuristic may be used as a warning/safety mechanism, but it must be documented and testable.

---

# 55. First Refresh

When no previous cache exists, package-count comparison is unavailable.

Candidate health must rely on:

```text
collector success
normalization success
domain validity
reasonable non-empty dataset
operator-specific expectations
```

Do not require previous cache to bootstrap.

---

# 56. Partial Normalization Failure

Example:

```text
raw: 40
normalized: 38
rejected: 2
```

may still be healthy.

Example:

```text
raw: 40
normalized: 2
rejected: 38
```

is likely suspicious.

The exact acceptance policy belongs to refresh validation.

Cache should only receive a candidate marked safe to commit.

---

# 57. Cache API Should Not Guess Health

Avoid:

```rust
cache.save(packages);
```

where the cache internally guesses whether those packages are trustworthy.

Prefer orchestration that clearly communicates an approved snapshot.

Conceptually:

```rust
cache.commit(validated_snapshot)?;
```

This keeps cache persistence separate from collector semantics.

---

# 58. Previous Snapshot

During refresh, keep the current snapshot available until commit succeeds.

Correct:

```text
current = old snapshot

candidate created
candidate persisted successfully

current = candidate
```

Do not mutate `current` before persistence succeeds.

---

# 59. Memory vs Disk State

The application may maintain an in-memory snapshot for fast access.

Disk cache provides persistence.

These states must remain consistent.

Preferred commit order:

```text
candidate
    ↓
persist atomically
    ↓
disk commit succeeds
    ↓
publish candidate to memory
```

This avoids showing state that was never successfully persisted.

---

# 60. Persistence Failure

If collection succeeds but cache persistence fails:

```text
network ✓
normalize ✓
validate ✓
disk write ✗
```

do not destroy the previous disk snapshot.

The application may decide whether to temporarily use the fresh in-memory candidate, but must clearly distinguish that from persisted cache state.

Prefer predictable consistency over hidden divergence.

---

# 61. Read Failure

If disk cache cannot be read because of permissions or filesystem problems:

* do not panic;
* attempt network refresh where possible;
* report a structured cache error;
* allow the rest of the application to function if possible.

---

# 62. Directory Creation

Create the application cache directory lazily/safely when needed.

Handle:

```text
already exists
permission denied
path conflict
disk errors
```

without panicking.

---

# 63. Concurrency

Multiple refresh operations may attempt to update the same operator.

Prevent races.

Conceptually:

```text
Rightel refresh A
Rightel refresh B
```

must not produce:

```text
A writes temp
B writes temp
A renames
B overwrites unexpectedly
```

Use per-operator synchronization or refresh deduplication.

---

# 64. Per-Operator Locking

Prefer locking at operator granularity.

A slow Irancell refresh should not prevent reading or refreshing MCI cache unnecessarily.

Conceptually:

```text
Irancell lock
MCI lock
Rightel lock
Samantel lock
```

Avoid one global cache lock around network operations.

---

# 65. Never Hold Cache Lock During HTTP

Do not:

```text
lock cache
    ↓
HTTP request
    ↓
parse
    ↓
normalize
    ↓
unlock
```

Network operations may take seconds.

Locks should protect short critical sections such as:

```text
commit
publish
metadata update
```

---

# 66. Refresh Deduplication

If two callers request the same operator refresh simultaneously, consider sharing or rejecting duplicate work.

Do not unnecessarily send duplicate upstream requests.

Possible behavior:

```text
refresh already in progress
        ↓
reuse/wait for same refresh
```

Keep implementation simple unless concurrency requirements demand more.

---

# 67. Generation Ordering

If overlapping refreshes are possible, prevent an older refresh from replacing a newer result.

Example:

```text
Refresh A starts at 10:00
Refresh B starts at 10:01

B finishes at 10:02
A finishes at 10:03
```

A must not necessarily overwrite B merely because it finished later.

Use refresh generation/start metadata if needed.

---

# 68. Refresh State

Per-operator runtime state may conceptually include:

```rust
enum RefreshState {
    Idle,
    Refreshing,
}
```

with metadata such as:

```text
last_attempt_at
last_success_at
last_error
```

This runtime state is distinct from the persisted package snapshot.

---

# 69. Last Attempt vs Last Success

Track these separately when useful.

Example:

```text
last_success: 10:00
last_attempt: 11:00
last_attempt failed
```

The package dataset is still from 10:00.

Do not update `fetched_at` to 11:00 merely because a failed refresh was attempted.

---

# 70. Failed Refresh Does Not Refresh Age

If refresh fails:

```text
cached fetched_at = 10:00
attempt at 11:00 fails
```

cache age remains based on:

```text
10:00
```

Do not make stale data appear fresh after a failed request.

---

# 71. Refresh Result

Conceptually:

```rust
enum RefreshOutcome {
    Updated,
    KeptCached,
    NoData,
}
```

with structured details.

Potential fields:

```text
operator
previous_fetched_at
new_fetched_at
error category
```

Exact design may differ.

---

# 72. KeptCached Is Not Success

If refresh failed but old data remains available, distinguish:

```text
refresh succeeded
```

from:

```text
refresh failed, cached fallback used
```

The UI may choose to show a subtle warning.

Do not report stale fallback as a successful live refresh.

---

# 73. Missing Cache + Refresh Failure

If:

```text
no cache
+
operator refresh fails
```

then that operator has no usable package data.

Do not manufacture an empty successful snapshot.

Return a clear unavailable state.

Other operators remain usable.

---

# 74. Overall Dataset State

The application may aggregate per-operator states.

Example:

```text
Irancell → Fresh
MCI      → Fresh
Rightel  → StaleFallback
Samantel → Unavailable
```

The combined package list can still be useful.

Do not reduce this to a single misleading global boolean.

---

# 75. Per-Operator Freshness

When presenting combined recommendations, packages may originate from snapshots with different ages.

Cache should preserve operator-level freshness metadata so orchestration/UI can expose this if necessary.

Recommendation logic should not silently alter ranking based on freshness.

---

# 76. Manual Cache Clear

If the application provides:

```text
Clear Cache
```

it must intentionally remove BastehYab-owned cached snapshots.

After clearing:

```text
no cache
```

is a legitimate state.

Do not automatically recreate fake empty snapshots.

---

# 77. Cache Clear Safety

Before deleting:

* resolve the BastehYab cache directory;
* target known cache files;
* avoid path traversal;
* avoid broad recursive deletion outside the owned directory.

Never construct deletion paths from arbitrary user/operator strings.

---

# 78. Cache Clear vs Settings

Clearing package cache must not remove unrelated user configuration.

Keep:

```text
cache
settings
```

logically separated.

A user asking to clear package data should not lose UI preferences.

---

# 79. Cache Clear vs Logs

Likewise, cache cleanup should not accidentally remove diagnostics unless the product explicitly defines a full data reset.

Separate storage responsibilities.

---

# 80. Cache Size

Package metadata is small.

Do not implement complex eviction algorithms such as:

```text
LRU
LFU
multi-gigabyte quota management
```

for the initial product.

Per-operator snapshots should remain compact.

---

# 81. Historical Snapshots

The initial cache should store the latest trustworthy snapshot, not an unlimited package history.

Do not accumulate:

```text
irancell-2026-08-01.json
irancell-2026-08-02.json
irancell-2026-08-03.json
...
```

without a product requirement.

This is a cache, not analytics/history storage.

---

# 82. Optional Backup Snapshot

A single previous backup may be justified for crash/recovery safety if atomic replacement behavior on supported platforms benefits from it.

Example:

```text
rightel.json
rightel.json.bak
```

Do not create indefinite generations.

If backups are introduced, define exactly:

```text
when created
when restored
when deleted
how validated
```

---

# 83. Backup Is Not Automatically Trusted

A backup must pass the same:

```text
schema
deserialization
domain validation
operator consistency
```

checks as the primary cache.

Do not restore a corrupt `.bak` merely because the main file failed.

---

# 84. Recovery Order

If backup support exists:

```text
primary valid
    ↓ yes
use primary

primary invalid
    ↓
backup valid
    ↓ yes
use backup

otherwise
    ↓
network refresh
```

Keep recovery deterministic.

---

# 85. Checksums

Do not add cryptographic checksums merely to detect ordinary partial writes if atomic filesystem replacement already solves the problem.

A checksum may detect accidental corruption but does not establish data authenticity.

Add one only if it provides concrete value.

---

# 86. Cache Encryption

Package catalogs are public information.

Do not encrypt cache by default.

Encryption adds complexity without protecting meaningful secrets.

Sensitive credentials/tokens must not be persisted in the package cache in the first place.

---

# 87. Never Cache Authentication Tokens

Critical rule:

Do not persist operator authentication/session tokens inside package snapshots.

Examples:

```text
Rightel Bearer token
cookies
session IDs
CSRF tokens
temporary authentication response
```

must not enter persistent package cache.

Authentication material belongs only to the collector request lifecycle unless a separately reviewed design explicitly requires otherwise.

---

# 88. No Cookie Persistence

Do not create a browser-like persistent cookie jar for operators unless absolutely required.

Prefer ephemeral request/session state.

Package cache must remain independent from HTTP session state.

---

# 89. No Raw Headers

Do not cache HTTP request/response headers alongside package snapshots.

They may contain:

* cookies;
* tokens;
* server details;
* transient identifiers.

Persist only domain-relevant metadata.

---

# 90. No Raw Responses by Default

Do not store raw HTML/JSON responses in production cache for debugging.

Raw responses may:

* consume unnecessary disk;
* contain transient tokens;
* change independently;
* expose unnecessary data.

Use sanitized fixtures for tests.

Development diagnostics must be explicit and safe.

---

# 91. Cache Logging

Useful cache logs include:

```text
operator
operation
snapshot age
package count
cache hit/miss
refresh outcome
schema version
```

Example:

```text
operator=rightel
cache=stale
packages=42
refresh=failed
fallback=used
```

Do not log full package datasets during normal operation.

---

# 92. Secret Redaction

Cache errors must never include:

```text
Authorization headers
cookies
session tokens
authentication payloads
```

Cache code should not normally have access to these values at all.

---

# 93. Cache Hit

A cache hit means a usable snapshot was loaded.

Distinguish:

```text
fresh hit
stale hit
```

if freshness affects orchestration.

Do not use "hit" to imply network freshness.

---

# 94. Cache Miss

A cache miss includes situations such as:

```text
file absent
unsupported schema
corrupt snapshot
invalid domain data
```

Diagnostics may distinguish them internally.

UI should receive useful high-level state rather than raw filesystem errors.

---

# 95. Structured Cache Errors

Conceptually:

```rust
enum CacheError {
    DirectoryUnavailable,
    ReadFailed,
    WriteFailed,
    SerializationFailed,
    DeserializationFailed,
    UnsupportedSchemaVersion,
    InvalidSnapshot,
    OperatorMismatch,
    AtomicReplaceFailed,
}
```

Exact variants may differ.

Do not collapse everything into:

```text
Cache error
```

internally.

---

# 96. Safe Error Context

Useful error context:

```text
operator
operation
schema version
path category
```

Avoid exposing unnecessary absolute filesystem paths to normal UI unless needed for diagnostics.

---

# 97. No Panic on Cache Failure

Cache is a resilience mechanism.

It must not become a single point of failure.

Avoid:

```rust
fs::read(path).unwrap()
serde_json::from_slice(&data).unwrap()
fs::write(path, bytes).expect(...)
```

for production cache operations.

Return structured errors.

---

# 98. Domain Validation on Write

Even if callers are expected to provide valid snapshots, cache commit should protect important persistence invariants where inexpensive.

Examples:

```text
operator consistency
schema version
duplicate IDs
invalid empty identity
```

Do not persist obviously invalid state.

---

# 99. Domain Validation on Read

Validate persisted snapshots again after deserialization.

The file may no longer match assumptions made when originally written.

Read validation and write validation provide defense in depth.

---

# 100. Serialization Round Trip

Tests should verify:

```text
snapshot
   ↓ serialize
bytes
   ↓ deserialize
snapshot
```

preserves meaningful domain state.

Especially test:

```text
unknown values
unlimited allowances
multiple allowances
combined packages
time windows
enum values
```

---

# 101. Atomic Write Tests

Where practical, test the atomic persistence helper separately.

Verify:

```text
new snapshot replaces old snapshot
temporary file does not remain after success
failed serialization does not touch old file
failed write does not intentionally delete old file
```

Filesystem-failure simulation may use abstractions if justified.

---

# 102. Corruption Tests

Test cache files containing:

```text
empty file
truncated JSON
invalid JSON
wrong schema version
wrong operator
duplicate IDs
invalid package structure
```

Expected behavior:

```text
no panic
cache rejected
other operator caches unaffected
```

---

# 103. Stale Tests

Given:

```text
fetched_at = known timestamp
now = controlled timestamp
TTL = known duration
```

test:

```text
fresh
boundary
stale
```

Do not use real wall-clock sleeps in unit tests.

---

# 104. Clock Injection

Freshness logic should be testable without waiting for real time.

Prefer passing:

```text
now
```

or using a small clock abstraction where necessary.

Do not scatter direct `Utc::now()` calls throughout freshness calculations.

---

# 105. Refresh Failure Tests

Test:

```text
valid old cache
+
collector failure
```

Expected:

```text
old cache remains intact
```

Also test:

```text
old cache
+
normalization failure
```

Expected:

```text
old cache remains intact
```

And:

```text
old cache
+
candidate validation failure
```

Expected:

```text
old cache remains intact
```

---

# 106. Successful Refresh Test

Given:

```text
old snapshot
+
healthy candidate
```

expected:

```text
new snapshot persisted
new snapshot published
old snapshot no longer current
```

with correct new `fetched_at`.

---

# 107. Empty Candidate Test

Given:

```text
old snapshot: 40 packages
candidate: 0 packages
```

candidate must not automatically overwrite the old snapshot.

Health validation should reject suspicious emptiness.

---

# 108. Partial Operator Test

Simulate:

```text
Irancell success
MCI failure
Rightel success
Samantel failure
```

Verify successful operators update independently and failed operators retain their own previous snapshots where available.

---

# 109. Concurrent Refresh Test

If refresh concurrency is implemented, test two simultaneous updates for the same operator.

Verify:

* no corrupted cache;
* no partial JSON;
* deterministic winner;
* no stale generation overwriting newer data.

---

# 110. Different Operators Concurrently

Refreshing:

```text
Irancell
Rightel
```

simultaneously should not require unnecessary serialization through a global lock.

Tests may verify logical isolation where architecture supports it.

---

# 111. Cache Schema Tests

For every supported schema version:

```text
known version → expected behavior
old supported → migrate/read
old unsupported → invalidate
future version → reject
```

Keep schema policy explicit.

---

# 112. Fixture Independence

Do not use live operator requests in cache unit tests.

Use deterministic normalized package fixtures.

Network behavior belongs to collector/integration tests.

---

# 113. Integration Test

A useful integration test:

```text
raw fixture
    ↓
collector parser
    ↓
normalizer
    ↓
validated snapshot
    ↓
cache commit
    ↓
cache reload
    ↓
recommendation
```

This verifies boundaries work together without live internet access.

---

# 114. Recommendation Consistency After Reload

A normalized package dataset should produce equivalent recommendation results:

```text
before cache serialization
```

and:

```text
after cache reload
```

assuming identical recommendation context.

This catches semantic serialization bugs.

---

# 115. Cache Does Not Own UI State

Do not store:

```text
selected operator
selected filters
window size
theme
language
last open tab
```

in the package cache.

Those belong to application settings/state.

Keep responsibilities separate.

---

# 116. Cache Does Not Own HTTP State

Do not store:

```text
ETag
cookies
Bearer tokens
CSRF tokens
browser sessions
```

inside `OperatorSnapshot`.

If future conditional HTTP requests use ETag/Last-Modified, create a deliberately reviewed HTTP metadata mechanism separate from package domain data.

---

# 117. Conditional Requests

Future collectors may benefit from:

```text
ETag
If-None-Match
Last-Modified
If-Modified-Since
```

when official sources support them.

This optimization must not compromise correctness.

A `304 Not Modified` may preserve the existing snapshot while updating appropriate successful-check metadata.

Do not change package `fetched_at` semantics without defining whether it means:

```text
catalog content retrieval time
```

or:

```text
last successful freshness verification
```

Keep those concepts separate if necessary.

---

# 118. Last Verified At

If conditional requests become important, introduce a separate field such as:

```text
last_verified_at
```

rather than overwriting:

```text
fetched_at
```

with a different meaning.

Semantic timestamps must remain precise.

---

# 119. Cache and Pagination

Collectors are responsible for obtaining the complete operator dataset.

Cache must not persist page-by-page partial results as the authoritative snapshot.

For an operator with 40 packages over four pages:

```text
page 1
page 2
page 3
page 4
    ↓
complete normalized candidate
    ↓
single operator snapshot
```

Do not replace cache after page 1.

---

# 120. Cache and Authentication

For operators such as Rightel where a temporary public website token may be required:

```text
authenticate
    ↓
fetch package data
    ↓
normalize
    ↓
token discarded
    ↓
cache packages
```

Never serialize the temporary token into cache.

---

# 121. Cache and Dynamic HTML

For operators whose data comes from HTML/document parsing:

```text
HTML
  ↓
collector parser
  ↓
raw package records
  ↓
normalization
  ↓
cache
```

Do not persist the HTML document as the package cache.

---

# 122. Refresh All

A high-level refresh-all operation should conceptually run:

```text
refresh Irancell
refresh MCI
refresh Rightel
refresh Samantel
```

with independent outcomes.

The final result should report each operator separately.

Do not return only:

```text
success: false
```

because one operator failed.

---

# 123. Refresh Summary

Conceptually:

```rust
struct RefreshSummary {
    operators: Vec<OperatorRefreshResult>,
}
```

Possible operator states:

```text
Updated
FreshCacheUsed
StaleCacheUsed
Unavailable
Failed
```

Exact naming may evolve.

Keep enough distinction for UI transparency.

---

# 124. Startup Strategy

Recommended conceptual startup:

```text
Application starts
      │
      ▼
Load all usable caches
      │
      ▼
Immediately expose available packages
      │
      ▼
Determine stale/missing operators
      │
      ▼
Refresh those operators
      │
      ▼
Publish successful updates
```

This avoids unnecessary blank/loading screens when cached data exists.

---

# 125. First Launch

On first launch:

```text
no cache
    ↓
refresh operators
    ↓
publish results independently
```

The UI should be able to receive Irancell results even if another operator is still loading/failing.

Cache APIs should not require all operators to finish before any data becomes available.

---

# 126. Offline Launch

If the application starts without internet:

```text
fresh/stale cache exists
    ↓
use cache
```

If:

```text
no cache
+
no internet
```

show an unavailable state.

Do not require an internet connection merely to launch the application.

---

# 127. Offline Recommendations

Recommendations can run entirely from cached normalized data.

Correct:

```text
offline
  ↓
cache
  ↓
packages
  ↓
recommendations
```

The UI should communicate data age where appropriate.

---

# 128. Cache Freshness and User Trust

Never make stale data appear freshly retrieved.

If the most recent successful Rightel snapshot is from yesterday, preserve that fact even if:

```text
user opened app today
refresh attempted today
refresh failed today
```

Data timestamps represent successful data state, not activity.

---

# 129. Cache Freshness and Recommendations

A recommendation produced from stale data may still be mathematically correct for that snapshot.

The engine should not label it differently internally.

The surrounding application may display:

```text
Based on data updated 8 hours ago
```

through localized UI.

---

# 130. No Cache-Based Price History

Do not infer:

```text
price increased
price decreased
new package
removed package
```

unless BastehYab intentionally introduces historical snapshot storage.

The initial last-known-good cache does not provide reliable long-term history.

---

# 131. Snapshot Comparison

Comparing:

```text
old snapshot
new candidate
```

during refresh is allowed for health validation.

This does not mean old snapshots should be permanently retained.

Use the comparison transiently.

---

# 132. Candidate Diff

Useful health diagnostics may include:

```text
added packages
removed packages
changed packages
package count delta
```

This can help detect scraper breakage.

Do not block legitimate changes merely because a diff is large without a defined health rule.

---

# 133. Package Identity Matters

Snapshot comparison must use canonical package identity:

```text
operator + external_id
```

not:

```text
array index
package name alone
price alone
```

This is why stable normalization identity is critical.

---

# 134. Changed Package

If the same package ID has:

```text
new price
new allowance
new validity
new availability
```

the new validated snapshot should replace the old package.

Do not merge individual fields from old and new versions unless explicitly required.

A snapshot represents one coherent observation.

---

# 135. No Field-Level Stale Mixing

Avoid:

```text
new package price
+
old package volume
+
old validity
```

because some new fields were missing.

Normalization should determine whether the new package is valid.

Cache should not synthesize hybrid package records across refresh generations.

---

# 136. Snapshot Coherence

An operator snapshot should represent one coherent collection generation.

Do not combine:

```text
half packages fetched at 10:00
half packages fetched yesterday
```

inside a single snapshot unless orchestration explicitly models such partial provenance.

Default behavior should prefer coherent last-known-good snapshots.

---

# 137. Operator Failure Fallback Is Snapshot-Level

If Rightel refresh fails:

```text
keep previous Rightel snapshot
```

Do not selectively merge whatever fragments were successfully parsed from the failed refresh into the old snapshot.

This prevents subtle partial-data corruption.

---

# 138. Successful Partial Source

If an operator intentionally exposes multiple independent package sources and the collector can prove partial updates are semantically safe, that requires an explicit operator-specific design.

Do not make partial merging the generic cache behavior.

---

# 139. Cache Lifetime

Do not promise that cache persists forever.

OS cleanup, user action, application upgrades, or corruption may remove it.

The application must always be able to rebuild package cache from official sources.

---

# 140. Cache Compatibility with App Updates

On application update:

```text
read cache envelope
    ↓
check schema
    ↓
use / migrate / invalidate
```

Do not assume cache created by the previous binary is automatically compatible.

---

# 141. Development Cache

Development builds should avoid accidentally depending on stale developer cache during tests.

Unit/integration tests should use temporary directories.

Do not write tests against the user's real application cache.

---

# 142. Temporary Test Directories

Filesystem tests should create isolated temporary directories.

Each test owns its files.

Clean them after execution where practical.

Do not use fixed global paths that create test interference.

---

# 143. Test Parallelism

Cache tests may run concurrently.

Avoid shared filenames/directories such as:

```text
/tmp/bastehyab-test-cache.json
```

Use unique temporary test locations.

---

# 144. Cache Abstraction

Keep persistence behind a focused abstraction.

Conceptually:

```rust
trait CacheStore {
    fn load(&self, operator: Operator)
        -> Result<Option<OperatorSnapshot>, CacheError>;

    fn commit(
        &self,
        snapshot: &OperatorSnapshot,
    ) -> Result<(), CacheError>;

    fn remove(
        &self,
        operator: Operator,
    ) -> Result<(), CacheError>;
}
```

Exact trait design is optional.

Do not introduce abstraction merely for ceremony, but keep filesystem details out of unrelated business logic.

---

# 145. Filesystem Store

The initial implementation can use:

```text
FileCacheStore
```

backed by JSON.

Do not implement Redis, SQLite, PostgreSQL, or remote storage for package caching without a real requirement.

---

# 146. In-Memory Cache

An in-memory representation may sit above filesystem persistence.

Conceptually:

```text
File cache
    ↓
load
    ↓
in-memory snapshots
    ↓
fast application access
```

Do not confuse this with a second source of truth.

Persisted snapshots provide startup recovery.

---

# 147. Cache Read Performance

Package files are small.

Reading/deserializing them at startup is acceptable.

Do not introduce:

```text
memory-mapped files
custom binary formats
database indexes
```

without profiling evidence.

---

# 148. Cache Write Frequency

Write only after meaningful successful snapshot updates.

Do not rewrite package cache:

* every UI render;
* every filter change;
* every recommendation;
* every application tick.

This reduces unnecessary disk activity.

---

# 149. Unchanged Refresh

If a successful refresh returns semantically identical package data, the application may choose whether to rewrite the snapshot.

If avoiding rewrite:

* still preserve successful verification metadata appropriately;
* do not falsely change package content timestamps.

Keep semantics explicit.

---

# 150. Semantic Equality

If snapshot equality is needed, compare meaningful normalized package state.

Do not compare raw serialized JSON strings because:

* field ordering may differ;
* formatting may differ;
* irrelevant metadata may differ.

Define semantic equality intentionally.

---

# 151. Source Ordering

Operator package order should not determine cache identity or correctness.

If source order changes but package content does not:

```text
[A, B, C]
→
[C, A, B]
```

this should not necessarily be treated as meaningful catalog change.

Canonical ordering may be used for deterministic serialization if useful.

---

# 152. Deterministic Serialization

Where practical, persist packages in deterministic order.

For example:

```text
stable package ID
```

This improves:

* debugging;
* diffs;
* tests;
* reproducibility.

Do not rely on unordered map iteration.

---

# 153. Cache Security Boundary

Cache files are local untrusted persistent input.

Apply:

```text
bounded parsing
schema validation
domain validation
safe paths
no executable content
```

Do not execute anything obtained from cache.

---

# 154. Bounded Cache Reads

Package cache should be small.

Consider rejecting absurdly large files before full deserialization.

Example:

```text
hundreds of megabytes
```

for one operator package catalog is clearly suspicious.

Use a reasonable upper bound based on expected application scale.

Do not allocate memory based blindly on malicious file contents.

---

# 155. Bounded Collections

Likewise, reject absurd package counts.

Expected package counts are small.

A cache claiming:

```text
50,000,000 packages
```

should not be trusted.

Choose conservative bounds generous enough for legitimate growth.

---

# 156. Bounded Strings

If domain deserialization allows arbitrarily huge strings, malicious cache modification could waste memory.

Use reasonable validation for:

```text
package names
descriptions
external IDs
URLs
USSD codes
```

Exact bounds should be generous and domain-appropriate.

---

# 157. Path Safety

Operator cache paths should come from trusted enum mappings.

Preferred:

```rust
match operator {
    Operator::Irancell => "irancell.json",
    Operator::Mci => "mci.json",
    Operator::Rightel => "rightel.json",
    Operator::Samantel => "samantel.json",
}
```

Do not use:

```rust
format!("{}.json", user_input)
```

for filesystem paths.

---

# 158. Symlink Considerations

Do not implement elaborate filesystem security prematurely, but avoid following arbitrary user-controlled paths.

The cache directory and filenames should be entirely application-controlled.

---

# 159. Error Recovery

Cache failures should degrade functionality, not terminate the application.

Desired progression:

```text
fresh cache
    ↓
stale cache
    ↓
network refresh
    ↓
operator unavailable
```

The application should use the best trustworthy state available.

---

# 160. Recovery Priority

For an operator, conceptual priority is:

```text
successful fresh refresh
        >
valid fresh cache
        >
valid stale cache
        >
no data
```

Do not prefer malformed "new" data over old valid data.

---

# 161. Trust Ordering

Age and validity are different dimensions.

A newer invalid candidate is worse than an older valid snapshot.

Therefore:

```text
validity first
freshness second
```

not:

```text
newest timestamp always wins
```

---

# 162. Cache API Semantics

Prefer APIs that expose state clearly.

Conceptually:

```rust
struct CachedSnapshot {
    snapshot: OperatorSnapshot,
    freshness: CacheFreshness,
}
```

instead of returning only:

```rust
Vec<InternetPackage>
```

when callers need freshness information.

---

# 163. CacheFreshness

Conceptually:

```rust
enum CacheFreshness {
    Fresh,
    Stale,
}
```

Missing cache should be represented by:

```text
Option
```

or an explicit state depending on API design.

Do not encode missing as:

```text
Fresh + empty packages
```

---

# 164. OperatorDataState

At orchestration level, a richer state may be useful:

```rust
enum OperatorDataState {
    Fresh,
    Stale,
    Refreshing,
    Unavailable,
}
```

Do not force all runtime states into persistent cache metadata.

Persist only what is needed across application restarts.

---

# 165. Cache vs Orchestrator

The cache layer answers:

```text
What valid snapshot do I have?
Can I persist this approved snapshot safely?
How old is it?
```

The refresh orchestrator answers:

```text
Should I fetch now?
Which operators should refresh?
What happens after collector failure?
Should stale data be displayed?
```

Keep this boundary clear.

---

# 166. Cache vs Collector

Collector:

```text
obtains source data
```

Cache:

```text
stores validated normalized snapshots
```

Collector must not directly write cache files.

Correct:

```text
collector
  ↓
normalizer
  ↓
validator
  ↓
orchestrator
  ↓
cache
```

---

# 167. Cache vs Normalizer

Normalizer must not read previous cache values to "fix" current operator data.

Avoid:

```text
current price missing
→ use previous cached price
```

That creates hidden hybrid snapshots.

Unknown current data should remain unknown or make the candidate invalid according to normalization/health rules.

---

# 168. Cache vs Recommendation

Recommendation engine should receive packages from the current application dataset.

It should not directly open cache files.

Correct:

```text
cache store
   ↓
application state
   ↓
recommendation engine
```

This keeps ranking logic pure and testable.

---

# 169. Cache vs UI

Frontend should not know filesystem paths or JSON cache structure.

Expose high-level Tauri commands/state such as:

```text
packages
operator freshness
last successful update
refresh status
```

Do not let React directly manipulate cache files.

---

# 170. i18n

Cache/domain errors should expose machine-readable categories.

Frontend handles translations.

Do not return Rust strings such as:

```text
"کش خراب است"
```

as the primary error contract.

---

# 171. Refresh UX Support

Cache/orchestration should provide enough information for UI states such as:

```text
Updating...
Updated just now
Using cached data
Could not update Rightel
Last successful update: ...
```

The UI decides exact wording.

---

# 172. Progressive Updates

If operators refresh independently, application state may update progressively:

```text
Irancell completes
    ↓
UI gets new Irancell packages

Rightel still loading
```

Do not require refresh-all to wait before publishing every successful operator update unless architectural simplicity strongly favors it.

---

# 173. Recommendation Refresh

When a new operator snapshot is published:

```text
combined package dataset changes
    ↓
recommendations may be recalculated
```

Do not persist recommendation results merely to avoid this inexpensive recalculation.

---

# 174. Refresh Cancellation

If application shutdown or cancellation interrupts refresh before commit:

```text
old snapshot remains
```

A candidate that never committed must not become authoritative.

---

# 175. Application Crash

Crash scenarios must preserve last-known-good cache as much as possible.

Atomic replacement is the primary protection.

After restart:

```text
load valid committed snapshot
ignore incomplete temp state
```

---

# 176. Disk Full

If disk is full during cache write:

```text
new candidate cannot persist
```

Do not delete old cache in an attempt to force the write unless explicitly designed.

Return a write failure.

The application may still use available data in memory for the current session.

---

# 177. Read-Only Filesystem

If cache directory becomes read-only:

* reading existing snapshots may still work;
* refresh may still fetch current packages;
* persistence may fail.

Do not crash.

Expose persistence failure separately from collection success.

---

# 178. Clock Problems

System clocks can be wrong.

Freshness logic should handle future timestamps conservatively.

Example:

```text
fetched_at > now
```

Do not calculate negative age and treat the cache as fresh for years.

Use a safe policy such as:

```text
future timestamp beyond small tolerance
→ suspicious metadata
```

and handle explicitly.

---

# 179. Timestamp Bounds

Validate cache timestamps.

Reject clearly impossible or malformed timestamps.

Do not over-engineer historical validation, but prevent absurd values from corrupting freshness logic.

---

# 180. Refresh Timeouts

HTTP timeouts belong primarily to collectors.

Cache should not remain locked or blocked waiting for network operations.

A timeout results in:

```text
refresh failure
    ↓
keep last-known-good snapshot
```

---

# 181. Rate Limits

If an operator rate-limits refresh:

```text
429 / equivalent
```

do not delete cache.

Use existing snapshot.

Backoff/retry policy belongs to collector/orchestrator logic.

Cache simply preserves good state.

---

# 182. Authentication Failure

If temporary operator authentication fails:

```text
refresh fails
```

Existing package cache remains intact.

Do not interpret authentication error responses as empty package catalogs.

---

# 183. HTML/API Format Changes

If an operator redesign causes:

```text
collector success at HTTP layer
but parser returns nonsense/zero packages
```

dataset-health validation must prevent cache replacement.

This is one of the primary reasons for last-known-good caching.

---

# 184. HTTP 200 Is Not Cache Success

Never equate:

```text
HTTP 200
```

with:

```text
safe to replace cache
```

A valid refresh requires the full pipeline:

```text
transport success
+
parse success
+
normalization success
+
domain validation
+
dataset health
+
atomic persistence
```

---

# 185. Cache Commit Definition

A cache commit is successful only after the new snapshot is durably/atomically established according to the persistence implementation.

Do not report success immediately after serialization but before filesystem replacement.

---

# 186. Metrics

Local diagnostics may count:

```text
cache hits
cache misses
stale loads
successful commits
failed commits
fallback uses
```

Do not send these metrics remotely unless a future explicit telemetry design is approved.

---

# 187. No Telemetry Requirement

The cache system must function without:

* analytics;
* crash-reporting services;
* cloud monitoring;
* remote metrics.

Everything required for normal operation remains local.

---

# 188. Development Diagnostics

For debugging, a developer-facing command may eventually expose:

```text
operator
schema version
package count
fetched_at
stored_at
freshness
last refresh status
```

Do not expose tokens or raw upstream payloads.

---

# 189. Clear Separation of Data Classes

Keep at least these concepts separate:

```text
Package Cache
Application Settings
Runtime Refresh State
Collector HTTP Session State
Logs/Diagnostics
```

Do not place them all into one generic JSON blob.

---

# 190. Cache Review Checklist

When reviewing cache changes verify:

```text
Is cache per operator?

Is last-known-good preserved?

Are writes atomic?

Can one corrupt operator cache be isolated?

Is schema version explicit?

Are timestamps semantically correct?

Are stale snapshots still usable?

Are failed refreshes distinguished from successful ones?

Can malformed cache crash the app?

Are tokens/cookies excluded?

Are raw responses excluded?

Are paths application-controlled?

Are writes bounded?

Are reads validated?

Are concurrent refreshes safe?

Does recommendation remain independent?

Does frontend avoid filesystem knowledge?
```

---

# 191. Prohibited Cache Patterns

Do not:

```rust
fs::remove_file(path)?;
fs::write(path, new_data)?;
```

for replacement.

Do not:

```rust
let cache = fs::read(path).unwrap();
```

Do not:

```rust
let snapshot: Snapshot =
    serde_json::from_slice(&bytes).unwrap();
```

Do not:

```text
refresh failed
→ clear cache
```

Do not:

```text
HTTP 200 + empty array
→ overwrite 40-package cache
```

Do not:

```text
Rightel failed
→ invalidate Irancell/MCI/Samantel
```

Do not:

```text
failed refresh
→ update fetched_at
```

Do not:

```text
Bearer token
→ serialize into cache
```

Do not:

```text
unknown cache version
→ deserialize anyway
```

Do not:

```text
stale
→ automatically delete
```

Do not:

```text
cache file exists
→ assume valid
```

Do not:

```text
new partial package
+
old fields
→ hybrid package
```

---

# 192. Preferred Cache Load Shape

Conceptually:

```rust
pub fn load(
    &self,
    operator: Operator,
    now: DateTime<Utc>,
) -> Result<Option<CachedSnapshot>, CacheError> {
    let path = self.path_for(operator);

    if !path.exists() {
        return Ok(None);
    }

    let bytes = read_bounded(&path)?;

    let envelope: CacheEnvelope =
        deserialize(&bytes)?;

    validate_schema(&envelope)?;
    validate_operator(operator, &envelope)?;
    validate_snapshot(&envelope)?;

    let freshness =
        determine_freshness(envelope.fetched_at, now);

    Ok(Some(CachedSnapshot {
        snapshot: envelope.into_snapshot(),
        freshness,
    }))
}
```

Exact implementation may differ.

Important stages:

```text
bounded read
    ↓
deserialize
    ↓
schema check
    ↓
operator check
    ↓
domain validation
    ↓
freshness
```

---

# 193. Preferred Cache Commit Shape

Conceptually:

```rust
pub fn commit(
    &self,
    snapshot: &OperatorSnapshot,
) -> Result<(), CacheError> {
    validate_snapshot(snapshot)?;

    let bytes = serialize(snapshot)?;

    let temp_path =
        self.temp_path_for(snapshot.operator);

    write_complete(&temp_path, &bytes)?;

    atomic_replace(
        &temp_path,
        &self.path_for(snapshot.operator),
    )?;

    Ok(())
}
```

The real implementation must account for supported platform filesystem semantics.

Never expose a partially written final file.

---

# 194. Preferred Refresh Shape

Conceptually:

```rust
pub async fn refresh_operator(
    operator: Operator,
) -> OperatorRefreshResult {
    let previous = cache.load(operator).ok().flatten();

    let candidate = match collect_and_normalize(operator).await {
        Ok(candidate) => candidate,
        Err(error) => {
            return fallback(previous, error);
        }
    };

    if let Err(error) =
        validate_candidate(&candidate, previous.as_ref())
    {
        return fallback(previous, error);
    }

    if let Err(error) =
        cache.commit(&candidate.snapshot)
    {
        return persistence_failure(
            previous,
            candidate,
            error,
        );
    }

    publish(candidate.snapshot);

    OperatorRefreshResult::Updated
}
```

Exact implementation may evolve.

The invariant is:

```text
old valid state survives every failure before successful commit
```

---

# 195. Preferred Startup Shape

Conceptually:

```text
for each operator:
    load cache independently

publish usable cached snapshots

for each stale/missing operator:
    schedule/perform refresh independently
```

Do not block all startup data on the slowest operator.

---

# 196. Cache Architecture

The intended boundary is:

```text
                Official Operators
                        │
                        ▼
                    Collectors
                        │
                        ▼
                   Normalizers
                        │
                        ▼
                    Validation
                        │
                        ▼
                Candidate Snapshot
                        │
                  Health Check
                        │
                        ▼
              ┌───────────────────┐
              │    Cache Store    │
              │                   │
              │ Last-Known-Good   │
              │ Per Operator      │
              │ Atomic Persistence│
              └─────────┬─────────┘
                        │
                        ▼
                Application State
                        │
             ┌──────────┴──────────┐
             ▼                     ▼
       Recommendations             UI
```

Cache is the resilience boundary between volatile external infrastructure and the local application experience.

---

# 197. Failure Architecture

Desired behavior:

```text
                    Refresh
                       │
             ┌─────────┼─────────┐
             │         │         │
           HTTP      Parse    Normalize
          failure    failure    failure
             │         │         │
             └─────────┼─────────┘
                       ▼
                Reject Candidate
                       │
                       ▼
              Keep Last-Known-Good
                       │
                       ▼
               Continue Application
```

A broken upstream response must not automatically become broken local state.

---

# 198. Core Invariants

The cache implementation must preserve these invariants:

```text
1. A failed refresh never destroys valid cached data.

2. Operators are isolated from each other's failures.

3. Only validated normalized snapshots become authoritative.

4. Cache replacement is atomic.

5. Stale data is distinct from invalid data.

6. Failed refresh attempts do not make old data appear fresh.

7. Authentication/session secrets are never persisted.

8. Cache corruption never crashes the whole application.

9. Cache schema compatibility is explicit.

10. Recommendations operate on package data, not cache internals.
```

Treat violations of these rules as architectural defects.

---

# 199. Implementation Priority

Implement cache functionality in this order:

```text
1. Cache envelope/domain types

2. Per-operator paths

3. Serialization/deserialization

4. Schema validation

5. Domain validation on read/write

6. Atomic persistence

7. Freshness calculation

8. Last-known-good fallback

9. Per-operator refresh integration

10. Concurrency protection

11. Diagnostics

12. Optional recovery enhancements
```

Do not begin with advanced backup/history systems before basic atomic last-known-good persistence is correct.

---

# 200. Final Principle

BastehYab cache exists to preserve trustworthy package information when external operator infrastructure is unreliable.

The central rule is:

```text
Never trade known-good data
for unverified newer data.
```

The preferred decision sequence is:

```text
Do I have a valid cache?
        │
        ├── yes ───────────────┐
        │                      │
        ▼                      │
Is it fresh?                   │
        │                      │
   yes ─┴─> use it             │
        │                      │
       no                      │
        ▼                      │
use stale cache                │
+ attempt refresh              │
        │                      │
        ▼                      │
Is candidate healthy?          │
        │                      │
       no ─────────────────────┘
        │
       yes
        ▼
Can it be committed safely?
        │
       no ─────────────────────┘
        │
       yes
        ▼
Replace old snapshot atomically
        │
        ▼
Publish fresh data
```

Cache correctness is more important than cache freshness.

An older trustworthy snapshot is preferable to a newer incomplete, malformed, semantically invalid, or partially collected dataset.
