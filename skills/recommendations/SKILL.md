# Recommendations Skill

## Purpose

This skill defines how BastehYab evaluates, ranks, compares, and recommends normalized internet packages.

It applies primarily to:

```text
src-tauri/src/recommendations/
```

and recommendation-specific tests.

Read this skill before:

* implementing a recommendation;
* modifying recommendation scoring;
* changing eligibility rules;
* adding ranking metrics;
* changing tie-breaking;
* introducing composite scores;
* changing how restricted traffic is valued;
* changing how unlimited packages participate in rankings;
* exposing recommendation explanations to the UI.

This skill supplements:

```text
AGENTS.md
DESIGN.md
skills/collectors/SKILL.md
skills/normalization/SKILL.md
```

Repository-wide rules in `AGENTS.md` and architectural decisions in `DESIGN.md` take precedence.

---

# 1. Recommendation Mission

The recommendation engine answers questions such as:

```text
Which package gives the most general internet per unit of money?

Which 30-day package provides the most general data?

Which package is cheapest?

Which long-term package offers the best value?

Which package provides the largest unrestricted allowance?
```

Its responsibility begins only after normalization.

Correct flow:

```text
Collectors
    ↓
Normalizers
    ↓
InternetPackage[]
    ↓
User Filters
    ↓
Recommendation Engine
    ↓
Ranked Results
    ↓
UI
```

The recommendation engine must never parse operator-specific raw responses.

---

# 2. Recommendations Are Derived

Recommendation results are derived information.

They are not operator facts.

Do not store values such as:

```text
best_value = true
recommendation_score = 92
rank = 1
```

inside the canonical package as if they came from the operator.

They must be calculated from the current normalized dataset.

---

# 3. Deterministic Recommendations

Given identical:

```text
packages
filters
recommendation strategy
configuration
```

the engine must produce identical results.

Do not use:

* randomness;
* unstable iteration order;
* current time unless the strategy explicitly depends on time;
* UI ordering;
* operator response ordering

to determine winners.

Tie-breaking must be explicit.

---

# 4. Explainability

Every recommendation must be explainable.

The system should be able to answer:

> Why was this package recommended?

Example:

```text
Best value

Price: 120,000 toman
General data: 20 GB
Cost per GB: 6,000 toman
```

The recommendation engine should expose structured reasoning data.

The UI is responsible for localization and presentation.

Do not generate Persian recommendation sentences inside Rust recommendation logic.

---

# 5. Recommendation Result

Conceptually:

```rust
struct Recommendation {
    strategy: RecommendationStrategy,
    package_id: PackageId,
    rank: usize,
    metrics: RecommendationMetrics,
    reasons: Vec<RecommendationReason>,
}
```

Exact implementation may evolve.

A recommendation should reference a package rather than duplicate the entire canonical package unnecessarily.

---

# 6. Recommendation Strategy

Strategies should be explicit domain concepts.

Conceptually:

```rust
enum RecommendationStrategy {
    BestValue,
    MostGeneralData,
    Cheapest,
    BestThirtyDay,
    BestLongTermValue,
}
```

Additional strategies may be introduced when they have clear user value.

Do not represent strategies as arbitrary string names scattered through the application.

---

# 7. Strategy Isolation

Each strategy should define:

```text
eligibility
metric
ordering
tie-breaking
explanation
```

Do not build one giant scoring function with unexplained conditions for every recommendation.

Preferred conceptual organization:

```text
recommendations/
├── mod.rs
├── best_value.rs
├── most_data.rs
├── cheapest.rs
├── thirty_day.rs
├── long_term.rs
├── metrics.rs
└── ranking.rs
```

Exact file structure may differ if a simpler organization remains clear.

---

# 8. No Operator Bias

Recommendations must not favor an operator merely because of its identity.

Never implement logic equivalent to:

```rust
if package.operator == Operator::Irancell {
    score += 10;
}
```

unless a future user-selected preference explicitly requests such behavior.

Default recommendations compare package facts, not operator brands.

---

# 9. User Filters First

Explicit user filters should normally be applied before recommendation ranking.

Conceptually:

```text
All normalized packages
        ↓
User filters
        ↓
Eligible package subset
        ↓
Recommendation strategy
        ↓
Ranking
```

Example:

```text
Operator: Rightel + Irancell
Validity: 30 days
SIM: prepaid
```

means recommendations should operate only on packages satisfying those filters.

Do not recommend an excluded package simply because it has a better score globally.

---

# 10. Strategy Eligibility Is Separate

User filters and recommendation eligibility are different.

Example:

The user may allow:

```text
all packages
```

but `BestValue` may require:

```text
known positive price
known positive general data
finite comparable allowance
```

Therefore:

```text
User-filtered packages
        ↓
Strategy eligibility
        ↓
Rank
```

Keep these stages explicit.

---

# 11. General Data Is the Default Comparison Basis

For ordinary value recommendations, use unrestricted `General` data.

Do not automatically include:

* night traffic;
* domestic-only traffic;
* social traffic;
* application-specific traffic;
* gift traffic;
* unknown traffic categories

in general price-per-GB calculations.

Example:

```text
Package A
10 GB general
+ 100 GB night
Price: 100,000

Package B
20 GB general
Price: 110,000
```

Do not treat Package A as a 110 GB package for ordinary value ranking.

---

# 12. Restricted Traffic Has Separate Value

Restricted allowances are real benefits and should not be discarded.

However, they require strategies designed for their semantics.

Potential future strategies:

```text
BestNightPackage
BestDomesticTraffic
BestSocialPackage
BestVideoPackage
```

Do not smuggle restricted traffic into `BestValue`.

---

# 13. Gift Traffic

Gift traffic must not automatically count as ordinary general data.

Even when a gift allowance appears unrestricted, preserving its distinction allows eligibility and recommendation policy to remain honest.

A future strategy may intentionally include eligible gift data.

That must be explicit.

---

# 14. Combined Packages

Combined packages containing:

```text
internet + voice
internet + SMS
internet + voice + SMS
```

must not be excluded merely because they are not internet-only.

If a combined package has comparable general internet data and price, it may participate in internet recommendations.

However, do not assign arbitrary monetary value to voice or SMS when calculating internet value.

---

# 15. Combined Package Value

For default `BestValue`:

```text
value = general internet / package price
```

The engine may mention voice/SMS as additional benefits in structured reasons.

It must not silently estimate:

```text
100 minutes = 20,000 toman
500 SMS = 10,000 toman
```

unless a future explicitly designed pricing model establishes those valuations.

---

# 16. Money Basis

All recommendation calculations must use canonical normalized money values.

Canonical monetary representation:

```text
IRR integer
```

Do not parse:

```text
"120 هزار تومان"
```

inside recommendation code.

Do not compare formatted strings.

Do not mix rial and toman.

---

# 17. Data Basis

All recommendation calculations must use canonical normalized data quantities.

Canonical finite representation:

```text
bytes
```

Do not compare display strings such as:

```text
"10 GB"
"5000 MB"
```

directly.

---

# 18. Avoid Floating Point for Core Ranking

Do not rely on floating-point division for exact ranking when integer comparison can produce the same ordering.

For price-per-data comparison:

```text
price_a / data_a
vs
price_b / data_b
```

prefer cross multiplication:

```text
price_a * data_b
vs
price_b * data_a
```

using sufficiently wide checked arithmetic.

This avoids floating-point precision issues.

---

# 19. Overflow-Safe Ratio Comparison

Cross multiplication can overflow.

Use an appropriate wider integer representation or checked arithmetic.

Do not allow malformed/extreme upstream values to produce incorrect rankings.

If exact safe comparison cannot be performed, return a controlled metric/ranking error rather than silently overflowing.

---

# 20. Display Metrics

Although exact ranking should avoid unnecessary floating point, the UI may need readable metrics such as:

```text
6,000 toman / GB
```

Derived display metrics may be calculated separately from ranking logic.

The exact ranking result must not depend on display rounding.

---

# 21. BestValue Definition

Default `BestValue` means:

> The eligible package with the lowest monetary cost per byte of unrestricted general internet.

Equivalent conceptual metric:

```text
price / general_data
```

Lower is better.

Or:

```text
general_data / price
```

Higher is better.

Use one implementation consistently.

---

# 22. BestValue Eligibility

A package is normally eligible when:

```text
price is known
price > 0
general data is known
general data > 0
general data is finite
package is not explicitly unavailable
```

Unknown availability may remain eligible unless product policy intentionally says otherwise.

Restricted allowances do not contribute to the general-data denominator.

---

# 23. Free Packages

A verified zero-price package requires special handling.

Do not divide by zero.

If a package genuinely provides:

```text
price = 0
general data > 0
```

it mathematically dominates paid packages for monetary value.

Represent this as a special value state rather than fake:

```text
price_per_gb = 0.0000001
```

Eligibility policy should distinguish genuine free packages from suspicious malformed data.

Normalization/validation is responsible for ensuring zero price has credible source evidence.

---

# 24. BestValue Example

Given:

```text
Package A
10 GB general
100,000 toman

Package B
20 GB general
160,000 toman

Package C
30 GB general
300,000 toman
```

Cost per GB:

```text
A = 10,000 toman/GB
B = 8,000 toman/GB
C = 10,000 toman/GB
```

Ranking:

```text
1. B
2. A / C according to tie-breaking
```

Do not rank solely by total data.

---

# 25. MostGeneralData

Definition:

> The eligible package with the largest finite unrestricted general-data allowance.

Primary ordering:

```text
general data descending
```

This strategy answers:

> Which package gives me the most normal internet volume?

It does not answer:

> Which package is the best value?

---

# 26. MostGeneralData Eligibility

Normally require:

```text
known positive general data
package not explicitly unavailable
```

Price does not have to be known merely to determine largest volume.

However, packages without known prices may be less useful to users and should expose that uncertainty.

Do not silently substitute price as a primary metric.

---

# 27. Cheapest

Definition:

> The eligible package with the lowest known payable price.

Primary ordering:

```text
price ascending
```

Do not divide by data.

A cheap package may contain very little data.

That is acceptable because the strategy is explicitly `Cheapest`.

---

# 28. Cheapest Eligibility

Normally require:

```text
known price
package contains internet
package not explicitly unavailable
```

Zero-priced verified packages rank before paid packages.

Unknown price packages are ineligible.

---

# 29. BestThirtyDay

Definition:

> The strongest package recommendation among packages whose verified validity is exactly 30 days.

Do not interpret:

```text
1 month
```

as exactly 30 days unless normalization has already established that equivalence.

Recommendation code must not reinterpret validity.

---

# 30. BestThirtyDay Metric

The initial default should prioritize:

```text
most unrestricted general data
```

among exact 30-day packages.

Primary ordering:

```text
general data descending
```

Secondary ordering:

```text
better value
```

Then use standard deterministic tie-breaking.

This directly supports:

> بهترین بسته ۳۰ روزه که بیشترین حجم داره

without introducing an opaque composite score.

---

# 31. Thirty-Day Eligibility

Require:

```text
Validity::Days(30)
positive general data
package not explicitly unavailable
```

Do not include:

```text
Validity::Unknown
Validity::Hours(720)
```

unless the domain explicitly establishes equivalence before recommendation.

Recommendation code should not create new duration semantics.

---

# 32. BestLongTermValue

A long-term strategy may compare packages above a defined validity threshold.

Example product definition:

```text
validity >= 60 days
```

The exact threshold must be a documented recommendation constant/configuration.

Do not scatter:

```text
60
```

through multiple functions.

---

# 33. Long-Term Metric

Default:

```text
lowest price per general-data byte
```

among eligible long-term packages.

Validity itself should not automatically multiply package value.

Example:

```text
20 GB / 60 days
```

is not necessarily twice as valuable as:

```text
20 GB / 30 days
```

Longer validity is a separate user benefit.

---

# 34. Duration-Normalized Metrics

Future strategies may intentionally calculate concepts such as:

```text
data per day
price per day
```

These must be explicitly named.

Do not silently incorporate duration into `BestValue`.

A user seeing "best value" should not need to reverse-engineer a hidden duration multiplier.

---

# 35. Unlimited Packages

Unlimited packages cannot be ranked as though their data amount were:

```text
u64::MAX
```

or infinity in ordinary finite-volume metrics.

They require explicit strategy semantics.

---

# 36. Unlimited and MostData

If traffic is genuinely unrestricted unlimited general data, it conceptually exceeds every finite allowance.

However, ranking it alongside finite packages may produce misleading UI unless unlimited semantics are clearly represented.

A strategy may place verified unrestricted unlimited packages first while marking the metric as:

```text
Unlimited
```

rather than inventing a byte amount.

---

# 37. Unlimited and BestValue

Do not calculate:

```text
price / infinity = 0
```

and automatically call every unlimited package the best value.

Real operator unlimited offers may contain:

* fair-use limits;
* throttling;
* time restrictions;
* application restrictions.

Only packages normalized as genuinely unrestricted unlimited general data may receive special unlimited treatment.

Even then, use explicit ranking policy rather than fake numeric ratios.

---

# 38. Fair-Use Unlimited

Packages with:

```text
100 GB high speed
then unlimited throttled
```

must not automatically beat ordinary packages as infinite data.

If the current domain cannot quantify post-threshold utility accurately, exclude them from ordinary finite `BestValue` calculations or use only their comparable finite high-speed allowance when the domain explicitly models it.

Never manufacture an infinite score.

---

# 39. Night Unlimited

A package containing:

```text
10 GB general
+ unlimited 01:00–07:00
```

has:

```text
10 GB
```

for default general-data comparison.

Night unlimited traffic does not make it unlimited for `BestValue`.

---

# 40. Unknown Values

Unknown values must not receive fake numeric substitutes.

Never:

```text
unknown price = 0
unknown data = 0
unknown validity = 30 days
unknown availability = available
```

If a strategy requires a value and that value is unknown, the package is normally ineligible for that strategy.

---

# 41. Unknown Availability

`Availability::Unknown` is not equivalent to unavailable.

Default recommendation policy may allow unknown availability when the package is currently present in an official package source.

However, results should preserve the uncertainty so the UI can communicate it if useful.

`Availability::Unavailable` should normally be excluded.

---

# 42. Eligibility Reason

When practical, strategy evaluation should be capable of explaining why a package was excluded.

Conceptually:

```rust
enum IneligibilityReason {
    MissingPrice,
    MissingGeneralData,
    ZeroGeneralData,
    UnsupportedUnlimitedSemantics,
    WrongValidity,
    ExplicitlyUnavailable,
}
```

This is useful for:

* tests;
* debugging;
* diagnostics;
* future UI explanations.

Do not expose internal implementation errors directly to users.

---

# 43. Ranking Is Not Filtering

Do not silently remove packages because they score poorly.

Poor value is not invalid data.

Recommendation ranking should produce ordering over eligible packages.

The full filtered package list remains available independently.

---

# 44. Top-N Results

Strategies should support returning more than one result.

Example:

```text
Top 5 Best Value
```

instead of only one winner.

Conceptually:

```rust
recommend(
    packages,
    strategy,
    limit,
)
```

A limit must not affect relative ranking.

---

# 45. Limit Validation

Handle:

```text
limit = 0
```

explicitly.

Do not panic.

Use a reasonable maximum if needed to protect UI/performance, but avoid arbitrary small limits.

---

# 46. Tie-Breaking

Every strategy requires deterministic tie-breaking.

Do not rely on:

```text
input order
HashMap order
operator response order
```

---

# 47. Standard Tie-Breaking

Unless a strategy has a stronger semantic reason, use a documented sequence such as:

```text
1. primary strategy metric
2. more general data
3. lower price
4. longer known validity
5. operator stable key
6. package stable ID
```

The exact sequence may vary per strategy.

Final tie-breakers must guarantee deterministic ordering.

---

# 48. BestValue Tie-Breaking

For equal exact value ratio:

```text
1. more general data
2. lower total price
3. longer known validity
4. stable operator ordering
5. package ID
```

Reasoning:

If two packages have identical cost per GB, users generally gain more usable data from the larger package, provided they are willing to pay its price.

Do not hide this policy.

---

# 49. Cheapest Tie-Breaking

For equal price:

```text
1. more general data
2. longer known validity
3. better exact value ratio
4. stable operator ordering
5. package ID
```

---

# 50. MostData Tie-Breaking

For equal general data:

```text
1. lower price when known
2. better exact value ratio when comparable
3. longer known validity
4. stable operator ordering
5. package ID
```

Unknown price must not accidentally beat a known cheaper price because of an implementation detail.

---

# 51. BestThirtyDay Tie-Breaking

Primary:

```text
more general data
```

Then:

```text
lower price
better value
stable operator ordering
package ID
```

Validity is already fixed at 30 days and therefore does not participate further.

---

# 52. Stable Operator Ordering

Operator identity may be used only as a final deterministic tie-breaker.

Example:

```text
Irancell
MCI
Rightel
Samantel
```

The exact enum ordering is not a quality preference.

It merely prevents unstable results when all meaningful metrics are equal.

Do not present the operator tie-break as a recommendation reason.

---

# 53. Recommendation Metrics

Expose structured metrics used to produce a result.

Conceptually:

```rust
struct RecommendationMetrics {
    price_irr: Option<u64>,
    general_data_bytes: Option<u64>,
    validity_days: Option<u32>,
    value_ratio: Option<ValueRatio>,
}
```

Do not calculate UI strings here.

---

# 54. Exact Value Ratio

Prefer an exact representation.

Conceptually:

```rust
struct ValueRatio {
    price_irr: u64,
    data_bytes: u64,
}
```

This permits exact comparison without requiring a rounded decimal.

Display conversion can happen later.

---

# 55. Recommendation Reasons

Reasons should be structured concepts.

Example:

```rust
enum RecommendationReason {
    LowestCostPerGeneralData,
    LargestGeneralData,
    LowestPrice,
    ExactThirtyDayValidity,
    IncludesVoice,
    IncludesSms,
}
```

The frontend maps these to localized text.

Do not return:

```text
"این بسته خیلی به‌صرفه است"
```

from Rust.

---

# 56. Reasons Must Be True

Do not attach:

```text
LowestPrice
```

merely because the package is cheap.

It must actually satisfy the meaning of the reason within the evaluated candidate set.

Recommendation explanations must correspond to real ranking facts.

---

# 57. Reasons vs Marketing

Recommendation reasons are factual explanations.

Avoid subjective labels such as:

```text
Amazing
Perfect
Excellent
Must Buy
```

unless future UI copy deliberately uses them independently of engine reasoning.

The engine should remain factual.

---

# 58. Composite Scores

Avoid composite scores unless simpler transparent strategies cannot express the product requirement.

Do not start with:

```text
score =
    data * 0.4 +
    price * 0.3 +
    validity * 0.2 +
    operator * 0.1
```

This is difficult to explain and mixes incompatible units.

Prefer explicit rankings.

---

# 59. When Composite Scores Are Appropriate

A composite strategy may be introduced only when:

1. the user-facing concept genuinely combines multiple preferences;
2. factors are normalized appropriately;
3. weights have documented product meaning;
4. the result can be explained;
5. tests establish expected behavior;
6. operator identity is not secretly weighted.

Example future feature:

```text
Balanced Recommendation
```

could justify such a model.

It must not silently replace `BestValue`.

---

# 60. User-Weighted Recommendations

Future versions may allow users to state preferences such as:

```text
I care more about volume.
I want the cheapest package.
I need at least 30 days.
I use night traffic heavily.
```

Those preferences should become explicit constraints/weights.

Do not infer them from unrelated behavior without a designed personalization feature.

---

# 61. No Hidden Personalization

Default recommendations must not depend on:

* device identifiers;
* browsing history;
* operator preference guessed from previous clicks;
* location;
* telemetry;
* remote profiles.

BastehYab is self-hosted/local-first.

Recommendation inputs should be explicit and inspectable.

---

# 62. Minimum Constraints

Recommendation queries may support constraints such as:

```text
minimum general data
maximum price
minimum validity
maximum validity
allowed operators
allowed SIM types
internet-only / combined
```

Apply these as filters before ranking.

Do not encode them as arbitrary score penalties.

If the user says:

```text
maximum 200,000 toman
```

a 210,000 toman package must not win because its score is better.

---

# 63. Maximum Price

Convert UI input into canonical IRR before recommendation execution.

Recommendation code receives:

```text
Money / IRR amount
```

not localized toman strings.

---

# 64. Minimum Data

Convert UI input into canonical bytes before recommendation execution.

Recommendation code should not know whether the user typed:

```text
10 GB
10240 MB
```

---

# 65. Validity Constraints

Validity filters must operate on normalized semantics.

Do not parse strings like:

```text
"ماهانه"
```

inside recommendation code.

---

# 66. SIM Constraints

If the user explicitly selects:

```text
Prepaid
```

packages known to be incompatible should be excluded.

Packages with unknown SIM applicability require explicit filter policy.

Do not automatically treat unknown as compatible.

A strict filter should normally exclude unknown values.

---

# 67. Strict vs Broad Filtering

If the product supports it, distinguish:

```text
strict compatibility
```

from:

```text
include unknown compatibility
```

Do not silently mix these semantics.

The default should favor trustworthy matching over unsupported assumptions.

---

# 68. Package Kind Filter

Users may choose:

```text
All
InternetOnly
Combined
```

This filtering uses normalized `PackageKind`.

Recommendation code must not inspect raw voice/SMS strings to reclassify packages.

---

# 69. Restricted Allowance Filters

Users may eventually request:

```text
include night packages
domestic traffic only
application-specific packages
```

These filters should use normalized allowance kinds.

Do not implement operator-specific keyword matching in recommendation code.

---

# 70. Comparison Set

Recommendation explanations are relative to the evaluated candidate set.

Example:

```text
"Cheapest"
```

means cheapest among packages surviving:

```text
user filters
+
strategy eligibility
```

not necessarily cheapest package in the entire national dataset.

The UI should have enough context to communicate this correctly.

---

# 71. Recommendation Context

Conceptually:

```rust
struct RecommendationContext {
    filters: PackageFilters,
    strategy: RecommendationStrategy,
    limit: usize,
}
```

The exact structure may differ.

Keep strategy inputs explicit rather than relying on global mutable state.

---

# 72. Pure Core Logic

Recommendation ranking should ideally be pure.

Conceptually:

```rust
fn rank(
    packages: &[InternetPackage],
    context: &RecommendationContext,
) -> Result<Vec<Recommendation>, RecommendationError>
```

It should not:

* perform HTTP requests;
* read cache files;
* mutate package data;
* access UI state;
* write configuration;
* perform IPC.

This makes behavior deterministic and easy to test.

---

# 73. No Network Access

Recommendation code must never contact operators.

If package data is stale, that is a refresh/cache concern.

Correct:

```text
Refresh
    ↓
Normalized packages
    ↓
Recommend
```

Incorrect:

```text
Recommend
    ↓
fetch Irancell
fetch MCI
```

---

# 74. No Cache Mutation

Recommendation execution must not update package cache.

It reads package values supplied by the caller.

Derived recommendations may optionally be cached elsewhere in the future if profiling demonstrates a need, but package cache remains separate.

---

# 75. Performance

Expected package counts are small enough that straightforward in-memory filtering and sorting are preferred.

For:

```text
tens
hundreds
even a few thousand packages
```

clarity is more important than complex indexing.

Do not introduce:

* databases;
* search engines;
* background services;
* remote recommendation infrastructure

for basic ranking.

---

# 76. Complexity

Typical strategy complexity:

```text
filter: O(n)
sort: O(n log n)
```

is acceptable.

If only the top result is required, a linear scan may be simpler.

Choose based on clarity and reuse rather than premature optimization.

---

# 77. Stable Sorting

Either use stable sorting or provide complete deterministic comparison keys.

Do not allow equal metrics to produce arbitrary ordering between executions.

---

# 78. Recommendation Errors

Use structured errors for actual engine failures.

Conceptually:

```rust
enum RecommendationError {
    InvalidLimit,
    ArithmeticOverflow,
    InvalidConfiguration,
}
```

No eligible packages is normally not an engine error.

It is a valid result:

```text
[]
```

with optional structured empty-result context.

---

# 79. Empty Results

Examples:

```text
No 30-day packages
No packages below selected price
No packages for selected SIM type
```

are normal outcomes.

Do not panic.

Do not fall back to an ineligible package.

If the user asks for:

```text
30-day package
```

do not silently recommend a 7-day package because none matched.

---

# 80. No Automatic Constraint Relaxation

Never silently relax user requirements.

Bad:

```text
No package <= 100,000 toman
→ recommend 120,000 toman
```

Bad:

```text
No 30-day package
→ recommend 31-day package
```

If future UX wants "closest alternatives," implement it as a separate explicit strategy/result section.

---

# 81. Alternative Recommendations

A future API may provide:

```text
exact_matches
alternatives
```

These must remain clearly separated.

Do not label an alternative as satisfying the original constraints.

---

# 82. Freshness

Recommendation quality depends on dataset freshness, but freshness is not normally a ranking metric.

Do not favor one package because its operator was fetched 30 seconds later than another.

The UI may display:

```text
last updated
```

separately.

---

# 83. Partial Refreshes

If one operator failed during refresh but valid cached data remains, recommendations may use that cached data according to cache/orchestration policy.

The recommendation engine itself should not decide whether stale data is acceptable.

It should receive package availability/freshness metadata if that distinction needs to be exposed.

---

# 84. Stale Data

Do not silently penalize stale packages using an arbitrary score.

If stale-data exclusion is desired, make it an explicit eligibility policy outside or within a clearly named recommendation context rule.

---

# 85. Recommendation Provenance

For debugging and UI transparency, a result may carry:

```text
strategy
evaluated candidate count
rank
metrics
reasons
```

Example:

```text
strategy: BestValue
candidates: 27
rank: 1
```

Do not expose unnecessary internals such as comparator implementation details.

---

# 86. Ranking Metadata

Conceptually:

```rust
struct RecommendationSet {
    strategy: RecommendationStrategy,
    evaluated_count: usize,
    results: Vec<Recommendation>,
}
```

This can help the UI distinguish:

```text
best among 40 packages
```

from:

```text
best among 2 matching packages
```

---

# 87. Recommendation Labels

The recommendation engine should expose semantic keys.

Example:

```text
best_value
most_general_data
cheapest
best_30_day
best_long_term_value
```

Frontend i18n maps them to:

```text
بهترین ارزش خرید
بیشترین حجم
ارزان‌ترین
بهترین بسته ۳۰ روزه
...
```

Do not hard-code translated labels in Rust.

---

# 88. UI Cards

The engine may provide everything needed for recommendation cards:

```text
package ID
rank
strategy
metrics
reason keys
```

The UI obtains package details from the canonical package list.

Avoid duplicating full package structures unless it materially simplifies IPC and remains consistent.

---

# 89. Recommendation Sections

The initial UI may expose sections such as:

```text
Best Value
Most Data
Cheapest
Best 30-Day Package
Best Long-Term Value
```

Each section should invoke a defined strategy.

Do not create separate ad hoc sorting logic in React for each card.

---

# 90. Single Ranking Authority

Rust recommendation logic is the authoritative ranking implementation.

The frontend must not re-rank results differently.

Bad architecture:

```text
Rust says package A is #1
React recalculates and shows package B
```

Frontend sorting for ordinary package browsing is separate from named recommendation strategies.

---

# 91. Browsing Sort vs Recommendation

User-selected list sorting such as:

```text
price low → high
data high → low
validity
```

may share comparator helpers.

But a browse sort is not automatically a recommendation.

Keep concepts distinct.

---

# 92. BestValue vs SortByValue

These may use the same exact metric.

Difference:

```text
SortByValue
→ orders a package list

BestValue
→ recommendation strategy + eligibility + reasons + ranking metadata
```

Reuse metric logic without conflating product semantics.

---

# 93. Tests

Every recommendation strategy must have deterministic unit tests.

At minimum test:

```text
normal ranking
ties
unknown fields
restricted allowances
combined packages
zero price
unlimited traffic
explicit unavailable packages
empty input
no eligible packages
filter interaction
```

---

# 94. BestValue Tests

Include cases such as:

```text
10 GB / 100k
20 GB / 160k
30 GB / 300k
```

Expected winner:

```text
20 GB / 160k
```

Also test exact ratio ties.

---

# 95. Restricted Traffic Test

Example:

```text
A:
10 GB general
+ 100 GB night
100k

B:
20 GB general
150k
```

Default `BestValue` must compare:

```text
A = 10 GB / 100k
B = 20 GB / 150k
```

not:

```text
A = 110 GB / 100k
```

This is a critical regression test.

---

# 96. Domestic Traffic Test

Example:

```text
A:
10 GB general
+ 50 GB domestic

B:
20 GB general
```

`MostGeneralData` must rank B above A.

Domestic traffic must not be added to general volume.

---

# 97. Application-Specific Test

Example:

```text
A:
5 GB general
+ 100 GB video-service traffic

B:
20 GB general
```

Default general-data recommendations must not rank A as 105 GB.

---

# 98. Combined Package Test

Example:

```text
A:
20 GB general
+ 100 minutes
150k

B:
20 GB general
160k
```

For `BestValue`, A may beat B because its internet value is already better.

Do not need to assign monetary value to the 100 minutes.

---

# 99. Voice Does Not Distort Internet Value

Example:

```text
A:
10 GB
+ unlimited voice
100k

B:
20 GB
150k
```

Default `BestValue` should still compare internet value:

```text
A = 10 GB / 100k
B = 20 GB / 150k
```

Do not arbitrarily boost A because voice is unlimited.

---

# 100. Zero Price Tests

Test:

```text
0 price + valid general data
0 price + zero data
0 price + unknown data
```

Only the credible free internet package should receive special free-value treatment.

No division by zero.

---

# 101. Unlimited Tests

Test independently:

```text
unlimited general
unlimited night
finite general + unlimited night
fair-use unlimited
unknown amount
```

Ensure they do not collapse into one semantic case.

---

# 102. Unknown Price Test

A package with:

```text
price: unknown
general data: 100 GB
```

must not win `BestValue` or `Cheapest`.

It may participate in `MostGeneralData` if the strategy does not require price.

---

# 103. Unknown Data Test

A package with:

```text
price: known
general data: unknown
```

must not participate in `BestValue` or `MostGeneralData`.

It may still be browsable.

---

# 104. Availability Test

Verify:

```text
Available → eligible
Unavailable → excluded
Unknown → policy-defined behavior
```

Keep unknown behavior explicit.

---

# 105. Thirty-Day Tests

Test:

```text
30 days
29 days
31 days
unknown
1 month if not normalized to 30 days
```

Only exact normalized 30-day validity qualifies.

---

# 106. Filter Tests

Test combinations:

```text
operator
price ceiling
minimum data
validity
SIM type
package kind
```

Ensure recommendation ranking never escapes the filtered candidate set.

---

# 107. Tie Tests

Create packages with identical:

```text
value ratio
data
price
validity
```

and verify final ordering remains deterministic using stable operator/package identity.

Run the test repeatedly if useful.

---

# 108. Input Order Independence

Given identical packages in different input order:

```text
[A, B, C]
[C, A, B]
[B, C, A]
```

the recommendation ranking must be identical.

This is an important deterministic test.

---

# 109. Exact Arithmetic Tests

Use ratios that expose floating-point problems.

Example:

```text
Package A:
price = 10
data = 3

Package B:
price = 20
data = 6
```

These are exactly equal ratios.

Tie-breaking, not floating-point noise, must determine order.

---

# 110. Overflow Tests

Use large valid integer values to verify ratio comparison does not overflow.

Do not rely on debug-build overflow panics as validation.

---

# 111. Property Tests

Property-based tests may be valuable for ranking invariants.

Examples:

```text
reordering input does not change ranking

lowering a package's price while everything else remains equal
cannot worsen its BestValue rank

increasing general data while everything else remains equal
cannot worsen its BestValue rank
```

Introduce property-testing dependencies only when they provide meaningful coverage.

---

# 112. Ranking Invariants

Important invariants include:

```text
same inputs → same outputs

explicitly unavailable package never wins

restricted data never becomes general implicitly

unknown required metric never beats known valid metric

changing input order does not change rank

recommendation does not mutate packages
```

These should guide tests and reviews.

---

# 113. Benchmarking

Recommendation logic is unlikely to be a performance bottleneck initially.

Do not add benchmarks until needed.

If benchmarking becomes useful, test realistic package counts such as:

```text
100
1,000
10,000
```

rather than artificial millions unrelated to product scale.

---

# 114. Recommendation Configuration

Strategy constants should be centralized.

Potential examples:

```text
long-term minimum days
default recommendation limit
maximum result limit
```

Do not expose every implementation detail as user configuration.

Only make something configurable when there is product value.

---

# 115. No Magic Weights

Avoid unexplained values such as:

```rust
score += data * 0.37;
score -= price * 0.22;
```

Any nontrivial weighting system must have explicit documented semantics and dedicated tests.

---

# 116. Adding a New Strategy

Before implementing a recommendation strategy, define:

```text
User question:
What question does this answer?

Eligibility:
Which packages can participate?

Primary metric:
What determines quality?

Restricted traffic policy:
Which allowance kinds count?

Unknown-value policy:
Which unknowns exclude participation?

Unlimited policy:
How are unlimited packages treated?

Tie-breaking:
How are equal candidates ordered?

Explanation:
What facts can the UI show?
```

If these cannot be answered clearly, the strategy is not ready to implement.

---

# 117. New Strategy Example

Suppose the product adds:

```text
Best Night Package
```

Define first:

```text
Eligibility:
known night allowance

Primary metric:
lowest price per night-data byte

General traffic:
does not contribute to primary metric

Unlimited night:
explicit special handling

Tie:
more night data
lower price
stable ID
```

Only then implement it.

---

# 118. Recommendation Registry

If strategies grow, a centralized registry may expose supported recommendation types.

Conceptually:

```rust
fn recommend(
    strategy: RecommendationStrategy,
    packages: &[InternetPackage],
    context: &RecommendationContext,
) -> Result<RecommendationSet, RecommendationError>
```

Keep dispatch explicit.

Avoid runtime plugin complexity for a small local application.

---

# 119. No Dynamic Code

Recommendation strategies are application code.

Do not:

* download ranking scripts;
* execute remote JavaScript;
* load arbitrary recommendation plugins;
* accept executable formulas from operators.

BastehYab must remain self-contained.

---

# 120. Recommendation Security

Recommendation inputs may originate partly from UI filters.

Validate:

```text
limits
numeric ranges
enum values
```

at the command/domain boundary.

Do not assume frontend input is trusted merely because the application is desktop-local.

---

# 121. No External AI Requirement

Core recommendations must not depend on:

* cloud AI;
* LLM APIs;
* remote recommendation services;
* embeddings;
* third-party analytics.

The initial problem is deterministic structured ranking and should remain local.

---

# 122. Future Natural-Language Queries

If future BastehYab supports:

```text
"یه بسته زیر ۲۰۰ تومن میخوام که حداقل ۲۰ گیگ باشه"
```

natural-language interpretation must ultimately produce explicit structured filters:

```text
max_price
min_general_data
```

The recommendation engine itself still operates on structured inputs.

Do not let opaque AI output directly assign package scores.

---

# 123. User Trust

Recommendation labels imply specific claims.

If BastehYab says:

```text
Best Value
```

the result must be mathematically best according to the documented candidate set and metric.

If BastehYab says:

```text
Most Data
```

it must actually have the largest comparable general allowance.

Avoid recommendation labels whose meaning cannot be defended.

---

# 124. Disclaimer Through Precision

Do not compensate for weak logic with generic disclaimers.

Instead make recommendation semantics precise.

Prefer:

```text
Lowest cost per GB of unrestricted data
```

over an undefined:

```text
Best package
```

unless `Best package` has a documented composite meaning.

---

# 125. Initial Recommended Strategies

The first production version should prioritize a small number of understandable recommendations:

```text
BestValue
MostGeneralData
Cheapest
BestThirtyDay
BestLongTermValue
```

These cover useful and distinct questions without introducing arbitrary scoring.

Additional strategies should be added based on actual product usefulness.

---

# 126. Suggested BestValue Reason Data

A result may expose:

```text
general_data_bytes
price_irr
exact value ratio
```

The UI can render:

```text
20 GB
160,000 toman
8,000 toman per GB
```

without recommendation code producing localized strings.

---

# 127. Suggested MostData Reason Data

Expose:

```text
general_data_bytes
price if known
validity if known
```

The UI can explain that the package has the largest unrestricted allowance among matching packages.

---

# 128. Suggested Cheapest Reason Data

Expose:

```text
price_irr
general_data_bytes if known
validity if known
```

The primary reason remains price.

Do not call it best value unless it also independently wins `BestValue`.

---

# 129. Multiple Badges

The same package may legitimately win multiple strategies.

Example:

```text
Package X

Best Value
Cheapest
Best 30-Day
```

Do not force each recommendation category to select a different package merely for UI variety.

Correctness is more important than visual diversity.

---

# 130. Duplicate Recommendation Cards

Although one package may win multiple strategies, the UI may choose how to present duplicate winners.

That is a presentation decision.

Do not alter recommendation results solely to avoid duplicate cards.

---

# 131. Recommendation Result Identity

A recommendation result should be identifiable by something equivalent to:

```text
strategy + package_id + rank
```

Do not create random recommendation IDs unless required for UI mechanics.

---

# 132. Recommendation Serialization

Recommendation results crossing the Tauri boundary must use stable serialization.

Use explicit enum representations such as:

```text
best_value
most_general_data
cheapest
best_30_day
best_long_term_value
```

Do not depend on Rust debug output.

---

# 133. Frontend Contract

Frontend types must preserve:

```text
strategy
package_id
rank
metrics
reasons
```

and any required candidate-set metadata.

Nullability must match Rust semantics.

Do not convert unknown metrics into zero during IPC mapping.

---

# 134. Recommendation Diagnostics

Useful development diagnostics:

```text
strategy
input package count
filter-matching count
eligible count
result count
```

Example:

```text
strategy=best_value
input=42
filtered=18
eligible=15
results=5
```

Do not log every package's full source description during normal operation.

---

# 135. Debug Ranking

Development-only diagnostics may expose comparator values when investigating unexpected rankings.

Example:

```text
A: 1,200,000 IRR / 20 GiB
B: 900,000 IRR / 10 GiB
```

Keep such output structured and safe.

No operator credentials exist at this layer and none should ever reach it.

---

# 136. Separation from Analytics

Do not collect analytics merely because recommendation results exist.

The recommendation engine should not report:

```text
which package user clicked
which operator user prefers
which recommendation converted
```

to any server.

BastehYab's core recommendation system is local.

---

# 137. Recommendation Versioning

If recommendation semantics materially change after release, consider a strategy/version constant for diagnostics or tests.

Example:

```text
BestValue v1
```

may define:

```text
general unrestricted traffic only
```

Do not introduce versioning prematurely, but avoid silently changing user-facing meaning without updating tests/documentation.

---

# 138. Code Review Checklist

For recommendation changes verify:

```text
Does it use only normalized data?

Is the user question clearly defined?

Is eligibility explicit?

Are unknown values handled honestly?

Are restricted allowances treated correctly?

Are unlimited packages handled explicitly?

Is ranking deterministic?

Are ties deterministic?

Is arithmetic safe?

Can the result be explained?

Are user filters respected?

Is operator identity absent from quality scoring?

Are UI strings absent?

Are network calls absent?

Are tests included?
```

---

# 139. Prohibited Recommendation Patterns

Do not implement equivalents of:

```rust
let total_data =
    general + night + domestic + social;
```

for default value comparison.

Do not:

```rust
let price = package.price.unwrap_or(0);
```

Do not:

```rust
let data = package.data.unwrap_or(u64::MAX);
```

Do not:

```rust
if operator == Operator::Mci {
    score += 10;
}
```

Do not:

```rust
score += voice_minutes as f64 * 0.25;
```

without an explicitly designed valuation model.

Do not:

```rust
packages.shuffle();
```

before selecting equal candidates.

Do not:

```rust
return packages.first();
```

after relying on upstream ordering.

Do not:

```text
30-day unavailable
→ silently choose 31-day
```

Do not:

```text
unknown = zero
unknown = best
unknown = available
```

---

# 140. Preferred Strategy Shape

Conceptually:

```rust
pub fn recommend_best_value(
    packages: &[InternetPackage],
    filters: &PackageFilters,
    limit: usize,
) -> Result<RecommendationSet, RecommendationError> {
    let candidates = apply_filters(packages, filters);

    let mut eligible = candidates
        .into_iter()
        .filter_map(evaluate_best_value_eligibility)
        .collect::<Vec<_>>();

    eligible.sort_by(compare_best_value);

    Ok(build_recommendation_set(
        RecommendationStrategy::BestValue,
        eligible,
        limit,
    ))
}
```

Exact implementation may differ.

The important separation is:

```text
filter
    ↓
eligibility
    ↓
metric
    ↓
ranking
    ↓
result/explanation
```

---

# 141. Preferred Metric Architecture

Shared metric helpers may include:

```text
general_data_bytes(package)
known_price(package)
exact_value_ratio(package)
known_validity_days(package)
```

These helpers operate only on normalized domain types.

They must not know about:

```text
Irancell
MCI
Rightel
Samantel
```

---

# 142. Recommendation Boundary

The architecture should maintain:

```text
             Operator Infrastructure
                       │
                       ▼
                  Collectors
                       │
                       ▼
                  Normalizers
                       │
                       ▼
══════════════ Stable Domain Boundary ══════════════
                       │
                       ▼
                InternetPackage[]
                       │
                       ▼
                    Filters
                       │
                       ▼
             Recommendation Engine
                       │
              ┌────────┼─────────┐
              ▼        ▼         ▼
          BestValue  Cheapest  MostData
              │        │         │
              └────────┼─────────┘
                       ▼
             RecommendationSet
                       │
                       ▼
                 Tauri IPC
                       │
                       ▼
                      UI
```

No upstream-specific knowledge should cross into the recommendation layer.

---

# 143. Recommendation Philosophy

BastehYab should prefer explicit mathematical comparisons over vague ideas of "smart recommendations."

For example:

```text
Best Value
=
lowest price per unrestricted GB
```

is useful because it is:

```text
understandable
testable
deterministic
explainable
operator-neutral
```

The application does not need opaque scoring to provide useful recommendations.

---

# 144. Final Principle

The recommendation engine must answer a clearly defined user question using normalized facts and transparent rules.

The core rule is:

```text
Filter explicitly.
Compare equivalent things.
Treat unknowns honestly.
Keep restricted traffic separate.
Rank deterministically.
Explain the result.
```

Never manipulate package semantics to produce a more interesting recommendation.

If two packages cannot be compared fairly under a strategy, do not pretend they can.

If no package satisfies the requested criteria, return no exact recommendation rather than silently changing the user's requirements.

Recommendation quality depends more on correct semantics and transparent ranking than on complex scoring.
