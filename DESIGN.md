# DESIGN.md

## 1. Purpose

BastehYab is a standalone Windows desktop application for collecting, normalizing, filtering, comparing, and recommending internet packages offered by Iranian mobile operators.

The application communicates directly with official operator-owned sources and performs all processing locally on the user's device.

BastehYab has no required backend, cloud service, hosted database, user account system, telemetry service, or third-party data provider.

The initial supported operators are:

* MCI (Hamrah-e Aval)
* Irancell
* Rightel
* Samantel

The primary product goal is not merely to display package lists.

BastehYab should answer questions such as:

* Which package provides the most general internet for its price?
* Which 30-day package provides the most data?
* What is the cheapest package matching my requirements?
* Which prepaid package has the best value?
* Which package under a specific budget provides the most usable data?
* Which combined package provides the best internet value?
* Which packages are available from a specific operator?
* How do selected packages compare?

All recommendations must be deterministic, transparent, and derived from normalized package data.

---

# 2. Product Scope

## 2.1 Initial Scope

The first stable version should provide:

* direct collection from supported operators;
* concurrent operator refresh;
* normalized package representation;
* local caching;
* manual refresh;
* automatic refresh on application startup;
* package browsing;
* filtering;
* sorting;
* package comparison;
* recommendation categories;
* recommendation explanations;
* per-operator freshness information;
* partial-failure handling;
* Persian-first desktop UI;
* RTL support;
* Windows distribution.

## 2.2 Package Eligibility

Any currently advertised package containing an internet/data allowance is eligible.

This includes:

* internet-only packages;
* internet + voice packages;
* internet + SMS packages;
* internet + voice + SMS packages;
* promotional internet packages;
* night packages;
* restricted-time packages;
* combined packages;
* packages containing general and special-purpose data.

A package containing no internet/data allowance is outside the primary dataset.

## 2.3 Explicitly Out of Scope

The initial architecture does not include:

* purchasing packages;
* operator account login;
* SIM management;
* balance checking;
* user accounts;
* cloud synchronization;
* hosted BastehYab API;
* analytics;
* telemetry;
* advertising;
* remote configuration;
* mobile applications;
* browser extensions;
* automatic background Windows services.

---

# 3. Target Platform

The initial target is:

```text
Windows 10/11 x64
```

The application should support normal installation and, where practical, a portable distribution.

Expected user experience:

```text
Download
   ↓
Install / Run
   ↓
Open BastehYab
   ↓
Packages appear
```

End users must not install Node.js, Rust, Python, Docker, or any development dependency.

---

# 4. Technology Stack

## Desktop Shell

Tauri.

## Core

Rust.

## UI

* React
* TypeScript
* Vite
* Tailwind CSS

A component library may be introduced only if it provides meaningful value without unnecessarily increasing complexity.

## Networking

Rust-native HTTP client.

Recommended implementation:

```text
reqwest
```

## Async Runtime

```text
tokio
```

## Serialization

```text
serde
serde_json
```

## HTML Parsing

Use a Rust HTML parser where necessary.

Do not execute remote JavaScript.

## Local Persistence

Initial implementation:

```text
JSON-based local cache
```

SQLite is intentionally unnecessary for the initial package dataset.

SQLite may be introduced later if persistent historical data, price history, advanced indexing, or substantially larger local datasets justify it.

---

# 5. High-Level Architecture

```text
┌───────────────────────────────────────────────────────┐
│                    BastehYab.exe                      │
│                                                       │
│  ┌─────────────────────────────────────────────────┐  │
│  │                React / TypeScript               │  │
│  │                                                 │  │
│  │ Dashboard                                       │  │
│  │ Packages                                        │  │
│  │ Filters                                         │  │
│  │ Recommendations                                 │  │
│  │ Comparison                                      │  │
│  │ Refresh/Freshness UI                            │  │
│  └───────────────────────┬─────────────────────────┘  │
│                          │                            │
│                     Tauri IPC                        │
│                          │                            │
│  ┌───────────────────────▼─────────────────────────┐  │
│  │                    Rust Core                    │  │
│  │                                                 │  │
│  │ Collectors                                      │  │
│  │ Normalizers                                     │  │
│  │ Validation                                      │  │
│  │ Filtering                                       │  │
│  │ Recommendations                                 │  │
│  │ Cache                                           │  │
│  │ Refresh Orchestration                           │  │
│  └───────────────────────┬─────────────────────────┘  │
│                          │ HTTPS                      │
└──────────────────────────┼────────────────────────────┘
                           │
       ┌───────────────────┼───────────────────┐
       │                   │                   │
       ▼                   ▼                   ▼
   irancell.ir           mci.ir       portal-api.rightel.ir
                                               │
                                               ▼
                                      package.rightel.ir

                           │
                           ▼
                  payment.samantel.ir
```

The frontend must not communicate with operator websites directly.

---

# 6. Core Data Flow

The main pipeline is:

```text
Official Source
      ↓
Collector
      ↓
Raw Operator Model
      ↓
Normalizer
      ↓
Validation
      ↓
InternetPackage
      ↓
Cache
      ↓
Filter
      ↓
Recommendation / Comparison
      ↓
Tauri
      ↓
UI
```

Raw operator models must never leak into recommendation or UI logic.

---

# 7. Domain Model

## 7.1 Operator

```rust
enum Operator {
    Mci,
    Irancell,
    Rightel,
    Samantel,
}
```

Serialized values:

```text
mci
irancell
rightel
samantel
```

---

# 8. Package Identity

Operator identifiers are only guaranteed to be unique within the same operator.

Therefore the canonical identity is conceptually:

```text
(operator, external_id)
```

A derived application ID may be generated as:

```text
irancell:<external_id>
mci:<external_id>
rightel:<external_id>
samantel:<external_id>
```

Example:

```text
rightel:328
```

Do not use package names as identifiers.

---

# 9. InternetPackage

Conceptual Rust model:

```rust
struct InternetPackage {
    id: PackageId,

    operator: Operator,
    external_id: String,

    name: String,

    price: Money,

    validity: Validity,

    data_allowances: Vec<DataAllowance>,

    voice: Option<VoiceAllowance>,
    sms: Option<SmsAllowance>,

    sim_types: Vec<SimType>,

    package_kind: PackageKind,

    availability: Availability,

    purchase: PurchaseInfo,

    metadata: PackageMetadata,
}
```

The exact Rust syntax may evolve during implementation, but these semantic boundaries should remain.

---

# 10. Money

All monetary calculations must use integers.

Never use floating point values for stored prices.

Conceptual model:

```rust
struct Money {
    amount: u64,
    currency: Currency,
}
```

Canonical internal currency should be:

```rust
IRR
```

Operator values expressed in toman must be converted to rial during normalization.

UI may display toman for Persian users.

Example:

```text
Internal:
1,500,000 IRR

UI:
150,000 تومان
```

Recommendation calculations must use the canonical internal value.

---

# 11. Validity

Avoid forcing every validity period into an inaccurate number of days.

Conceptual model:

```rust
enum Validity {
    Hours(u32),
    Days(u32),
    Unknown,
}
```

Common values:

```text
1 day
3 days
7 days
30 days
60 days
90 days
```

If an operator explicitly provides a validity that cannot safely be converted, preserve its semantic representation rather than guessing.

For recommendation categories such as `best_30_day`, eligibility requires an actual normalized 30-day validity.

---

# 12. Data Allowances

Internet volume is not a single scalar.

A package may contain multiple allowances with different restrictions.

Conceptual model:

```rust
struct DataAllowance {
    amount_bytes: Option<u64>,

    unlimited: bool,

    kind: DataAllowanceKind,

    time_window: Option<TimeWindow>,

    description: Option<String>,
}
```

`amount_bytes == None` does not automatically mean unlimited.

Unlimited must be represented explicitly.

## DataAllowanceKind

```rust
enum DataAllowanceKind {
    General,
    Night,
    Domestic,
    International,
    Social,
    ApplicationSpecific,
    Gift,
    Other,
}
```

Example:

```text
Package:
10 GB general
+
20 GB from 01:00 to 07:00
```

becomes:

```text
allowance #1
kind: General
amount: 10 GB

allowance #2
kind: Night
amount: 20 GB
time: 01:00–07:00
```

It must NOT become:

```text
30 GB general
```

---

# 13. Time Restrictions

Conceptual model:

```rust
struct TimeWindow {
    start: LocalTime,
    end: LocalTime,
}
```

A time-restricted allowance must remain distinguishable from unrestricted traffic.

If the operator provides an ambiguous textual restriction that cannot be safely parsed, preserve the description and avoid inventing a structured time range.

---

# 14. Voice and SMS

Combined packages must remain in the dataset.

Conceptual models:

```rust
struct VoiceAllowance {
    minutes: Option<u32>,
    unlimited: bool,
}

struct SmsAllowance {
    count: Option<u32>,
    unlimited: bool,
}
```

Unknown values must remain unknown.

Do not interpret missing voice/SMS fields as zero unless the source semantics guarantee that meaning.

---

# 15. SIM Types

```rust
enum SimType {
    Prepaid,
    Postpaid,
    Tdlte,
    DataSim,
    Other,
}
```

A package may support multiple SIM types.

Example:

```text
Irancell package
sim_types:
- prepaid
- postpaid
```

Do not duplicate a package solely to represent multiple SIM types unless the operator actually exposes them as distinct purchasable offers.

---

# 16. Package Kind

```rust
enum PackageKind {
    InternetOnly,
    Combined,
}
```

`Combined` means the package contains internet plus another meaningful service such as:

* voice;
* SMS;
* another bundled telecom benefit.

A night package is not automatically a combined package.

Traffic restriction and package composition are separate concepts.

---

# 17. Availability

Conceptual model:

```rust
enum Availability {
    Available,
    Unavailable,
    Unknown,
}
```

Do not silently discard an otherwise valid package merely because availability cannot be determined.

Recommendation logic may choose to exclude explicitly unavailable packages.

---

# 18. Purchase Information

BastehYab does not purchase packages.

It may expose official purchase information when safely available.

Conceptual model:

```rust
struct PurchaseInfo {
    official_url: Option<String>,
    ussd_code: Option<String>,
}
```

Only official operator destinations are allowed.

Do not construct undocumented purchase URLs from guesses.

---

# 19. Package Metadata

```rust
struct PackageMetadata {
    fetched_at: DateTime<Utc>,

    source_url: String,

    regulatory_code: Option<String>,

    offer_code: Option<String>,

    original_description: Option<String>,
}
```

Additional operator-specific values should not pollute the shared domain model unless they have application-wide meaning.

---

# 20. Raw Data

Raw operator data is useful for diagnostics and parser development but should not become part of normal UI contracts.

Collectors may temporarily retain sanitized raw structures during processing.

If raw responses are cached for debugging in development builds, they must:

* remain local;
* avoid secrets;
* avoid authentication tokens;
* be disabled or minimized in production.

---

# 21. Operator Collectors

Each operator has an isolated collector.

Suggested layout:

```text
src-tauri/src/
├── collectors/
│   ├── mod.rs
│   ├── irancell.rs
│   ├── mci.rs
│   ├── rightel.rs
│   └── samantel.rs
│
├── normalizers/
│   ├── mod.rs
│   ├── irancell.rs
│   ├── mci.rs
│   ├── rightel.rs
│   └── samantel.rs
```

Collector responsibilities:

```text
HTTP
 ↓
Raw response
 ↓
Operator-specific parsing
 ↓
Raw typed representation
```

Normalizer responsibilities:

```text
Raw typed representation
 ↓
Semantic interpretation
 ↓
InternetPackage
```

---

# 22. Irancell Collector

Known source:

```text
GET https://irancell.ir/e/products/5e16bf95d11fd7209ba56b20
```

Current behavior:

```text
HTTP GET
   ↓
JSON
   ↓
Deserialize
   ↓
Normalize
```

No browser automation is required.

Do not send captured browser cookies.

Start with minimal headers.

The known product/group identifier:

```text
5e16bf95d11fd7209ba56b20
```

must be isolated as collector configuration rather than scattered through implementation.

If a reliable method is discovered to derive this identifier from an official page, prefer discovery over permanent hard-coding when it does not materially increase complexity or fragility.

The collector must validate that the response resembles the expected package dataset before replacing cached Irancell data.

---

# 23. Rightel Collector

Rightel uses a website authentication flow.

## Step 1 — Authentication

```text
POST
https://portal-api.rightel.ir/user-management/api/v1/auth/authenticate
```

Payload:

```json
{
  "username": "website"
}
```

Expected response conceptually:

```json
{
  "data": {
    "token": "..."
  },
  "error": null
}
```

The token is a website-scoped temporary bearer token.

Never hard-code a captured token.

## Step 2 — Package Retrieval

```text
GET
https://portal-api.rightel.ir/extra-package/api/v1/extra-package-direct/web-site/purchasable-package
```

Authorization:

```text
Authorization: Bearer <website token>
```

The currently observed request includes:

```text
?d=<timestamp>
```

If required, generate it locally using the current timestamp.

Do not assume cache-busting parameters have business meaning.

## Token Handling

The token should remain in memory.

It does not need persistent storage for the initial implementation.

A new token may be obtained when:

* no token exists;
* the current token is expired;
* the package request returns an authentication-related failure.

Do not log the complete token.

---

# 24. MCI Collector

Known source:

```text
https://mci.ir/internet-plans
```

The page currently provides package data inside the initial HTML.

The visible pagination is client-side and must not be treated as evidence that separate network requests are necessary for each page.

Known structured JavaScript data includes:

```text
packegesObj
```

Preferred strategy:

```text
GET HTML
   ↓
Locate embedded package data
   ↓
Safely extract data
   ↓
Parse
   ↓
Normalize
```

Do NOT execute the page JavaScript.

If structured extraction proves less reliable than parsing stable HTML attributes, the collector may use HTML parsing instead.

Browser automation is not justified by the currently known behavior.

Pagination in the UI of the official website must not limit BastehYab to the first ten packages.

---

# 25. Samantel Collector

Known source:

```text
https://payment.samantel.ir/package
```

The initial HTML contains structured JavaScript package definitions in:

```text
objectData
```

Preferred base strategy:

```text
GET HTML
   ↓
Extract objectData safely
   ↓
Parse
   ↓
Normalize
```

Samantel also exposes an internal website endpoint:

```text
POST /api/mediator/samantel/
```

with behavior including:

```text
method=getpackagelist
mobile=<number>
```

This endpoint appears subscriber/mobile-dependent.

The initial general package collector must not require the user to provide a phone number unless future evidence demonstrates that doing so is necessary for the product's intended dataset.

For general package discovery, prefer public package definitions that do not require user-specific information.

Do not invent or ship a fake subscriber number solely to query personalized endpoints.

---

# 26. Collector Result

Each collector should return a result that distinguishes success, failure, and diagnostics.

Conceptually:

```rust
struct CollectorResult {
    operator: Operator,

    packages: Vec<InternetPackage>,

    fetched_at: DateTime<Utc>,

    status: CollectorStatus,

    warnings: Vec<CollectorWarning>,
}
```

Possible statuses:

```rust
enum CollectorStatus {
    Success,
    Partial,
    Failed,
}
```

A collector returning zero packages should not automatically count as success.

---

# 27. Suspicious Results

A successful HTTP request can still produce an invalid dataset.

Examples:

* zero packages;
* HTML error page with status 200;
* authentication page;
* malformed package objects;
* unexpectedly missing prices;
* response schema change.

Before replacing cache, apply sanity validation.

Initial validation should focus on semantic correctness rather than rigid historical counts.

Avoid rules such as:

```text
must always contain exactly 40 packages
```

because legitimate operator changes are expected.

Prefer rules such as:

```text
response must contain at least one valid internet package
```

and:

```text
a meaningful portion of parsed records must satisfy required invariants
```

---

# 28. Refresh Orchestrator

The application should expose one refresh operation that independently runs all enabled collectors.

Conceptually:

```rust
async fn refresh_all() -> RefreshResult
```

Execution:

```text
               refresh_all
                    │
      ┌─────────────┼─────────────┐
      │             │             │
      ▼             ▼             ▼
  Irancell         MCI         Rightel
                                  │
                                  ▼
                              Samantel
```

The actual execution should be concurrent rather than sequential where possible.

Use independent results.

A Rightel failure must not cancel Irancell/MCI/Samantel.

---

# 29. Refresh Result

Conceptual model:

```rust
struct RefreshResult {
    operators: Vec<OperatorRefreshResult>,
    completed_at: DateTime<Utc>,
}
```

Each operator result contains:

```rust
struct OperatorRefreshResult {
    operator: Operator,

    status: RefreshStatus,

    package_count: usize,

    fetched_at: Option<DateTime<Utc>>,

    using_cached_data: bool,

    error: Option<PublicError>,
}
```

The UI should receive sanitized errors, not low-level internal details or secrets.

---

# 30. Local Cache

Initial cache location should use the platform-appropriate Tauri application data directory.

Conceptually:

```text
%APPDATA%/
└── BastehYab/
    └── cache/
        └── packages.json
```

Do not rely on the current working directory.

---

# 31. Cache Structure

Conceptually:

```json
{
  "schemaVersion": 1,
  "operators": {
    "irancell": {
      "fetchedAt": "...",
      "packages": []
    },
    "mci": {
      "fetchedAt": "...",
      "packages": []
    },
    "rightel": {
      "fetchedAt": "...",
      "packages": []
    },
    "samantel": {
      "fetchedAt": "...",
      "packages": []
    }
  }
}
```

Cache updates should be atomic where practical:

```text
write temporary file
      ↓
flush
      ↓
replace previous cache
```

Avoid corrupting the last valid cache if the process terminates during a write.

---

# 32. Cache Replacement Rules

Per operator:

```text
Fetch
  ↓
Parse
  ↓
Normalize
  ↓
Validate
  ↓
Valid?
 ┌──┴──┐
Yes    No
 │      │
 ▼      ▼
Replace Keep previous
cache   cache
```

Never replace valid cached packages with an invalid empty result merely because the HTTP request succeeded.

---

# 33. Startup Behavior

Preferred startup sequence:

```text
Application starts
       ↓
Read cache
       ↓
Cache available?
  ┌────┴────┐
 Yes        No
  │          │
  ▼          ▼
Render      Render
cached      loading state
data         │
  │          │
  └────┬─────┘
       ↓
Refresh all operators
       ↓
Emit progress
       ↓
Update successful operators
       ↓
Persist cache
       ↓
Update UI
```

Cached data should allow the application to become useful immediately.

Network refresh must not unnecessarily block initial rendering.

---

# 34. Manual Refresh

The UI exposes:

```text
Refresh packages
```

Only one full refresh should run at a time.

Repeated clicks must not create unlimited concurrent refresh operations.

The UI should show per-operator progress.

Example:

```text
Irancell      Updating...
MCI           Updated · 40 packages
Rightel       Updating...
Samantel      Using cached data
```

---

# 35. Filtering Model

Conceptual filter:

```rust
struct PackageFilter {
    operators: Option<Vec<Operator>>,

    sim_types: Option<Vec<SimType>>,

    min_price: Option<u64>,
    max_price: Option<u64>,

    min_general_data_bytes: Option<u64>,
    max_general_data_bytes: Option<u64>,

    min_validity_days: Option<u32>,
    max_validity_days: Option<u32>,
    exact_validity_days: Option<u32>,

    package_kinds: Option<Vec<PackageKind>>,

    include_time_restricted: bool,

    has_voice: Option<bool>,
    has_sms: Option<bool>,
}
```

The exact API may evolve, but filters remain domain-level operations.

---

# 36. General Data Calculation

Recommendation and filtering frequently require the amount of unrestricted general data.

Define a reusable domain function conceptually:

```rust
fn general_data_bytes(package: &InternetPackage) -> Option<u64>
```

It sums only allowances that qualify as unrestricted general data.

It must not include:

* night-only traffic;
* application-specific traffic;
* gift traffic with incompatible restrictions;
* domestic-only traffic when comparing general internet;
* unknown traffic.

If any semantic ambiguity prevents a trustworthy calculation, the function should represent uncertainty rather than fabricate precision.

---

# 37. Price Per GB

For eligible finite general-data packages:

```text
price_per_gb =
    price_in_canonical_currency /
    general_data_in_gib_or_defined_decimal_gb
```

The project must choose one explicit GB convention and use it consistently.

Recommended normalization for telecom package display:

```text
1 GB = 1024 MB
1 MB = 1024 KB
```

Do not mix decimal and binary conversions across operators.

Internally, use bytes or another single canonical unit.

Display rounding is a UI concern.

---

# 38. Recommendation Architecture

Recommendation types should be modeled explicitly.

Conceptually:

```rust
enum RecommendationType {
    BestValue,
    Cheapest,
    MostData,

    Best30Day,
    Cheapest30Day,
    MostData30Day,

    BestUnderBudget,

    BestPrepaid,
    BestPostpaid,

    BestCombined,
    BestNight,

    BestLongTerm,
}
```

Some recommendation types require parameters.

Example:

```text
BestUnderBudget(max_price)
```

The exact Rust representation may therefore use structured variants.

---

# 39. Recommendation Result

Conceptually:

```rust
struct Recommendation {
    recommendation_type: RecommendationType,

    package: InternetPackage,

    metrics: RecommendationMetrics,

    explanation: RecommendationExplanation,
}
```

Metrics may contain:

```rust
struct RecommendationMetrics {
    general_data_bytes: Option<u64>,
    price_per_gb: Option<f64>,
    validity_days: Option<u32>,
}
```

Floating point may be used for derived display/ranking ratios where appropriate, but not for stored money.

Where exact comparison can be performed using integer arithmetic or rational comparison, prefer it to avoid unnecessary floating-point ranking errors.

---

# 40. Best Value

Default `BestValue` means:

> The eligible package providing the lowest price per unit of unrestricted general internet.

Eligibility requires:

* package is available or availability is not explicitly unavailable;
* finite known price;
* price > 0;
* known finite unrestricted general data;
* general data > 0;
* no incompatible restriction on the data being scored.

Conceptually:

```text
lowest(price / general_data)
```

For comparison, avoid division when possible:

```text
A.price / A.data < B.price / B.data
```

can be compared using cross multiplication with sufficiently wide integer arithmetic.

Night traffic must not improve default `BestValue`.

Gift or special-purpose traffic must not silently improve default `BestValue`.

---

# 41. Most Data

`MostData` ranks by unrestricted general data.

Tie-breaking:

1. greater general data;
2. lower price;
3. longer validity where meaningful;
4. stable package ID.

Tie-breaking must remain deterministic.

---

# 42. Cheapest

`Cheapest` ranks eligible packages by price.

Tie-breaking:

1. lower price;
2. greater general data;
3. longer validity where meaningful;
4. stable package ID.

---

# 43. 30-Day Recommendations

`Best30Day`, `Cheapest30Day`, and `MostData30Day` first restrict eligibility to:

```text
validity == 30 days
```

Then apply their corresponding ranking algorithm.

Do not treat 28-day or 31-day packages as 30-day packages unless product requirements explicitly change.

---

# 44. Best Under Budget

Input:

```text
maximum price
```

Eligibility:

```text
package.price <= maximum
```

Default ranking:

```text
maximum unrestricted general data
```

Tie-breaking:

1. more general data;
2. lower price;
3. better price per GB;
4. stable package ID.

---

# 45. Best Combined

Only `PackageKind::Combined` packages are eligible.

The initial definition should remain conservative.

Because assigning monetary value to voice minutes and SMS would require arbitrary assumptions, the default combined recommendation should not invent a universal conversion such as:

```text
1 SMS = X rial
1 minute = Y rial
```

unless a future documented scoring model explicitly defines it.

For the initial version, `BestCombined` may rank combined packages by internet value while clearly indicating the included additional benefits.

---

# 46. Best Night

Only appropriate night/time-restricted allowances participate.

This recommendation is separate from default general-data recommendations.

The explanation must prominently state the time restriction.

---

# 47. Long-Term Packages

Initial definition:

```text
validity > 30 days
```

If future product requirements need categories such as 60/90/180-day packages, implement them explicitly.

---

# 48. Recommendation Explanation

Every recommendation must produce machine-readable explanation metadata.

Do not generate recommendation explanations by parsing UI strings.

Conceptually:

```rust
enum RecommendationReason {
    LowestPricePerGb {
        price_per_gb: MoneyPerData,
    },

    MostGeneralData {
        bytes: u64,
    },

    LowestPrice {
        price: Money,
    },

    BestWithinBudget {
        budget: Money,
        bytes: u64,
    },
}
```

The UI translates this into Persian.

Example:

```text
این بسته در میان بسته‌های واجد شرایط،
کمترین هزینه را به ازای هر گیگابایت اینترنت عمومی دارد.
```

---

# 49. Filtering Before Recommendation

Recommendation requests may include filters.

Pipeline:

```text
All packages
     ↓
PackageFilter
     ↓
Eligible packages
     ↓
Recommendation-specific eligibility
     ↓
Ranking
     ↓
Recommendation
```

Example:

```text
Operator: Irancell + MCI
SIM: prepaid
Validity: 30 days
Budget: <= 200,000 toman
      ↓
BestValue
```

The recommendation engine must not bypass filters.

---

# 50. Package Comparison

Users should be able to compare a small number of selected packages.

Recommended maximum:

```text
4 packages
```

Comparison dimensions:

* operator;
* price;
* validity;
* general data;
* restricted data;
* price per GB;
* SIM type;
* package kind;
* voice;
* SMS;
* time restrictions;
* availability.

Comparison must not invent missing values.

Use:

```text
Unknown
```

instead of:

```text
0
```

when the source does not provide the information.

---

# 51. Tauri Commands

Keep commands explicit.

Initial conceptual command surface:

```text
get_packages
refresh_packages
get_recommendations
compare_packages
get_refresh_status
```

Potential signatures:

```rust
get_packages(filter: Option<PackageFilter>)
    -> Vec<InternetPackage>

refresh_packages()
    -> RefreshResult

get_recommendations(request: RecommendationRequest)
    -> Vec<Recommendation>

compare_packages(ids: Vec<PackageId>)
    -> ComparisonResult
```

Do not expose generic HTTP commands to React.

Bad:

```text
http_get(url)
```

Good:

```text
refresh_packages()
```

---

# 52. Tauri Events

Refresh progress may be delivered using events.

Conceptual events:

```text
refresh://started
refresh://operator-started
refresh://operator-completed
refresh://operator-failed
refresh://completed
```

Event payloads must use typed serializable structures.

Avoid using events for operations where a normal command response is sufficient.

---

# 53. Frontend Structure

Suggested structure:

```text
src/
├── app/
│   ├── App.tsx
│   └── routes.tsx
│
├── components/
│   ├── package/
│   ├── recommendation/
│   ├── filters/
│   ├── comparison/
│   └── common/
│
├── pages/
│   ├── HomePage.tsx
│   ├── PackagesPage.tsx
│   └── ComparePage.tsx
│
├── hooks/
│
├── lib/
│   ├── tauri.ts
│   └── formatters.ts
│
├── stores/
│
├── types/
│
└── i18n/
```

Avoid unnecessary global state.

Use local/component state where ownership is local.

Introduce a state library only if application complexity justifies it.

---

# 54. Main Navigation

Initial primary areas:

```text
Home / Recommendations
Packages
Compare
```

Settings should only be introduced when actual settings exist.

Do not create empty navigation sections for speculative future features.

---

# 55. Home / Recommendations

The initial screen should immediately answer:

> What are the best packages right now?

Suggested sections:

```text
Best value
Most data
Cheapest
Best 30-day
```

Users should be able to adjust basic criteria without navigating through complex configuration.

Each recommendation card should show:

* operator;
* package name;
* general data;
* validity;
* price;
* important restrictions;
* recommendation reason;
* freshness.

---

# 56. Packages Page

The packages view provides complete exploration.

Desktop layout:

```text
┌────────────────────────────────────────────────────────┐
│ Search / Sort / Refresh                                │
├───────────────┬────────────────────────────────────────┤
│               │                                        │
│ Filters       │ Package list                           │
│               │                                        │
│ Operator      │ [Package]                              │
│ SIM type      │ [Package]                              │
│ Price         │ [Package]                              │
│ Data          │ ...                                    │
│ Validity      │                                        │
│ Package type  │                                        │
│               │                                        │
└───────────────┴────────────────────────────────────────┘
```

Filters should update results quickly using local data.

Changing filters must not trigger operator network requests.

---

# 57. Package Card

A package card should prioritize:

```text
Operator
Package name

General data
Validity
Price

Price per GB

SIM type
Important restrictions
Combined-package indicators
```

Do not overload cards with every raw field.

Detailed allowances can be shown in an expandable details view.

---

# 58. Comparison UX

Users may select packages for comparison.

A persistent comparison indicator may show:

```text
3 / 4 packages selected
```

The comparison page presents aligned attributes.

Recommendation badges inside comparison must correspond to actual calculated metrics.

Avoid decorative "best" badges without algorithmic justification.

---

# 59. Freshness UX

Freshness is part of product trust.

The UI must distinguish:

```text
Fresh
Cached
Stale
Updating
Failed
```

Example:

```text
Irancell
Updated 2 minutes ago
```

or:

```text
Rightel
Could not refresh · showing data from 38 minutes ago
```

Do not hide stale data behind a generic success state.

---

# 60. Offline Behavior

BastehYab cannot obtain new packages without internet access.

However, if valid cache exists:

```text
Offline
  ↓
Load cache
  ↓
Allow filtering
  ↓
Allow recommendations
  ↓
Allow comparison
```

The application remains useful with previously collected data.

Display that data may be outdated.

---

# 61. Persian and RTL

The initial UI is Persian-first.

Requirements:

* correct RTL layout;
* Persian-friendly typography;
* Persian number formatting where appropriate;
* toman display for user-facing prices;
* correct mixed-direction rendering for codes/IDs;
* no assumptions that all strings are LTR.

Business logic must not depend on localized UI strings.

Enums and structured values are translated at the presentation layer.

---

# 62. Localization Readiness

Although Persian is the initial language, UI strings should not be scattered throughout components.

Use centralized translation resources from the beginning.

Example:

```text
i18n/
├── fa.json
└── en.json
```

English support may be incomplete initially, but architecture should not make localization expensive later.

Operator-provided Persian package names may remain source content rather than translation keys.

---

# 63. Formatting

Formatting belongs to the UI layer.

Examples:

```text
1536000 IRR
      ↓
153,600 تومان
```

and:

```text
10737418240 bytes
      ↓
10 گیگابایت
```

Do not store formatted strings as domain values.

---

# 64. Search

Local package search may match:

* package name;
* operator display name;
* relevant package description.

Search must operate on already collected local data.

Typing into search must never send requests to operator websites.

---

# 65. Sorting

Initial sort modes:

```text
Best value
Lowest price
Highest price
Most data
Least data
Longest validity
Shortest validity
```

Sorting semantics must reuse domain calculations where relevant.

Do not implement separate price-per-GB logic in React.

---

# 66. HTTP Client Design

Use a shared HTTP client configuration where practical.

Configure:

* reasonable connect timeout;
* reasonable request timeout;
* HTTPS;
* controlled redirects;
* minimal headers.

Each collector may add only the headers it actually requires.

Do not blindly impersonate a complete browser request.

---

# 67. Retry Policy

Retries should be conservative.

Recommended default:

```text
maximum 1 retry
```

for transient network/server failures.

Do not automatically retry:

* deterministic parsing failures;
* obvious authentication contract failures;
* invalid source structure.

Use bounded backoff.

The goal is resilience, not aggressive traffic generation.

---

# 68. User-Agent

BastehYab should use an identifiable application User-Agent where accepted.

Conceptually:

```text
BastehYab/<version>
```

If a particular official endpoint demonstrably requires browser-like behavior, document the exception in that collector.

---

# 69. Rate Discipline

A manual refresh should normally require only the minimum number of requests necessary per operator.

Do not crawl unrelated pages.

Do not repeatedly fetch identical resources.

Do not refresh automatically in tight intervals.

The initial application does not require continuous background polling.

---

# 70. Automatic Refresh Policy

Initial policy:

```text
Refresh on application startup
```

after cache has been loaded.

Do not implement minute-by-minute background refresh in the initial version.

Manual refresh remains available.

A future configurable refresh interval may be introduced only if justified.

---

# 71. Error Model

Errors should be categorized.

Conceptually:

```rust
enum CollectorError {
    Network,
    Timeout,
    HttpStatus,
    Authentication,
    InvalidResponse,
    Parse,
    Normalize,
    Validation,
}
```

Internal errors may contain detailed context.

Frontend-safe errors should contain:

```text
operator
category
user-safe message
retryable
```

Do not expose bearer tokens, cookies, or sensitive raw headers.

---

# 72. Partial Success

The application is designed around partial success.

Example:

```text
Irancell   Success
MCI        Success
Rightel    Failed
Samantel   Success
```

Expected behavior:

```text
Use fresh:
Irancell
MCI
Samantel

Use previous Rightel cache if available.

Display Rightel freshness/error state.

Continue recommendations using eligible available data,
while making freshness visible.
```

A single collector failure must not cause a global error screen.

---

# 73. Recommendation and Stale Data

Cached packages may still participate in recommendations.

However, recommendation results should retain freshness metadata so the UI can communicate when the winning package comes from stale data.

Do not silently imply that all compared packages were fetched at the same time.

---

# 74. Duplicate Handling

Duplicates may occur due to upstream representation.

Do not deduplicate solely by:

* name;
* price;
* volume.

Prefer operator-specific stable identifiers.

Two packages with identical display properties may still represent different purchasable offers.

Deduplication rules must be conservative.

---

# 75. Unknown Data

Unknown is a first-class state.

Examples:

```text
unknown validity
unknown SIM type
unknown availability
unknown allowance category
```

Do not convert unknown to:

```text
0
false
general
prepaid
```

merely to simplify UI or ranking.

Recommendations should exclude packages when required ranking information is unknown.

Browsing should still show them when useful.

---

# 76. Validation

Validation occurs after normalization.

Examples of invalid states:

```text
empty package ID
negative/impossible price representation
zero data for an alleged internet package
unlimited == true with contradictory finite semantics
invalid time range representation
```

Some anomalies should produce warnings rather than rejection.

Validation rules should distinguish:

```text
Fatal
Warning
```

Do not discard usable packages because optional metadata is missing.

---

# 77. Testing Architecture

Tests are divided into:

```text
Unit
Fixture-based parser
Integration
Live/manual
```

## Unit Tests

Test:

* unit conversion;
* money conversion;
* allowance classification;
* filtering;
* sorting;
* recommendation algorithms;
* tie-breaking;
* validity handling;
* comparison metrics.

## Fixture Tests

Store sanitized examples of official responses.

Suggested layout:

```text
src-tauri/tests/
└── fixtures/
    ├── irancell/
    ├── mci/
    ├── rightel/
    └── samantel/
```

Tests parse fixtures without internet access.

## Integration Tests

Test multiple internal modules together without relying on live operators where possible.

## Live Checks

Live operator requests must not be required by the normal test suite.

They may be implemented as explicitly invoked diagnostic/integration commands.

---

# 78. Parser Regression Tests

When an operator changes its source format and breaks parsing:

```text
Capture sanitized representative response
        ↓
Create/update fixture
        ↓
Write failing regression test
        ↓
Fix parser
        ↓
Verify normalized output
```

Do not fix upstream parser breakage without regression coverage when a representative fixture can reasonably be created.

---

# 79. Security Boundary

The React frontend is less trusted than the Rust core.

Do not expose:

* arbitrary HTTP;
* arbitrary filesystem reads;
* arbitrary filesystem writes;
* shell execution;
* process spawning.

Tauri capabilities should expose only application-specific operations.

---

# 80. Remote Content

Operator-provided content is untrusted.

Never:

```text
eval(remote_js)
```

Never inject remote HTML directly into the application DOM.

Render source text as text.

Embedded JavaScript data must be extracted and parsed without executing the script.

---

# 81. External Links

When opening an official operator page from the UI:

* allow only known/validated HTTPS destinations;
* avoid arbitrary URLs supplied by untrusted source data;
* clearly indicate that the link opens an external operator website.

---

# 82. Secrets

The repository must never contain:

* captured Rightel bearer tokens;
* browser cookies;
* personal subscriber numbers;
* authentication sessions;
* private credentials.

Website-scoped temporary credentials must be obtained dynamically through the official public flow.

---

# 83. Logging

Production logging should be useful but restrained.

Allowed examples:

```text
Rightel authentication succeeded
Rightel returned 62 raw packages
Rightel normalized 60 packages
Rightel rejected 2 invalid records
```

Avoid:

```text
Authorization: Bearer eyJ...
Cookie: ...
Full raw response containing sensitive data
```

---

# 84. Performance Goals

The dataset is small.

Optimize primarily for:

* fast startup;
* responsive filtering;
* minimal network requests;
* low memory usage;
* predictable behavior.

Do not introduce complex indexing or database infrastructure prematurely.

Filtering and recommendation over several hundred packages should be effectively instantaneous.

---

# 85. Build and Distribution

Release builds should produce a normal Windows desktop artifact.

Development toolchains are build-time concerns only.

Release packaging should eventually support:

```text
Installer
```

and, if technically practical:

```text
Portable executable/package
```

Do not make portable distribution a blocker for the initial functional release.

---

# 86. Application Identity

Project name:

```text
BastehYab
```

Repository:

```text
bastehyab
```

Executable:

```text
BastehYab.exe
```

Suggested application identifier:

```text
com.bastehyab.app
```

License:

```text
MIT
```

---

# 87. Proposed Repository Structure

```text
bastehyab/
│
├── AGENTS.md
├── DESIGN.md
├── README.md
├── LICENSE
│
├── skills/
│   ├── collectors/
│   │   └── SKILL.md
│   │
│   ├── normalization/
│   │   └── SKILL.md
│   │
│   ├── recommendations/
│   │   └── SKILL.md
│   │
│   └── cache/
│       └── SKILL.md
│
├── src/
│   ├── app/
│   ├── components/
│   ├── pages/
│   ├── hooks/
│   ├── lib/
│   ├── stores/
│   ├── types/
│   └── i18n/
│
├── src-tauri/
│   ├── src/
│   │   ├── collectors/
│   │   │   ├── mod.rs
│   │   │   ├── irancell.rs
│   │   │   ├── mci.rs
│   │   │   ├── rightel.rs
│   │   │   └── samantel.rs
│   │   │
│   │   ├── normalizers/
│   │   │   ├── mod.rs
│   │   │   ├── irancell.rs
│   │   │   ├── mci.rs
│   │   │   ├── rightel.rs
│   │   │   └── samantel.rs
│   │   │
│   │   ├── domain/
│   │   │   ├── mod.rs
│   │   │   ├── package.rs
│   │   │   ├── allowance.rs
│   │   │   ├── money.rs
│   │   │   ├── operator.rs
│   │   │   ├── filter.rs
│   │   │   └── recommendation.rs
│   │   │
│   │   ├── recommendations/
│   │   │   ├── mod.rs
│   │   │   ├── best_value.rs
│   │   │   ├── cheapest.rs
│   │   │   ├── most_data.rs
│   │   │   └── eligibility.rs
│   │   │
│   │   ├── cache/
│   │   │   ├── mod.rs
│   │   │   └── file_cache.rs
│   │   │
│   │   ├── refresh/
│   │   │   ├── mod.rs
│   │   │   └── orchestrator.rs
│   │   │
│   │   ├── commands/
│   │   │   ├── mod.rs
│   │   │   ├── packages.rs
│   │   │   ├── recommendations.rs
│   │   │   └── refresh.rs
│   │   │
│   │   ├── error.rs
│   │   └── lib.rs
│   │
│   └── tests/
│       └── fixtures/
│
├── package.json
├── tsconfig.json
└── vite.config.ts
```

This is a target organization, not a requirement to create empty files prematurely.

Create modules when responsibilities actually exist.

---

# 88. Skill Responsibilities

## `skills/collectors/SKILL.md`

Defines:

* HTTP collection practices;
* per-operator source contracts;
* parser strategy;
* upstream validation;
* fixture requirements;
* collector diagnostics.

## `skills/normalization/SKILL.md`

Defines:

* raw-to-domain conversion;
* units;
* price normalization;
* validity normalization;
* allowance classification;
* unknown-data policy.

## `skills/recommendations/SKILL.md`

Defines:

* eligibility;
* ranking;
* tie-breaking;
* explanations;
* filter interaction;
* restricted traffic semantics.

## `skills/cache/SKILL.md`

Defines:

* cache schema;
* atomic persistence;
* migration/versioning;
* stale-data behavior;
* corruption recovery.

## `skills/ui/SKILL.md`

Defines:

* desktop UX;
* RTL;
* component patterns;
* recommendation presentation;
* filtering UX;
* freshness/error states;
* comparison UX.

---

# 89. Implementation Milestones

Development should proceed incrementally.

## Milestone 1 — Foundation

Create:

* Tauri project;
* React/TypeScript frontend;
* Rust module structure;
* shared domain primitives;
* basic application shell;
* tests/build pipeline.

No fake production architecture should be added merely to make the UI look complete.

## Milestone 2 — Irancell

Implement:

```text
Irancell HTTP
→ deserialize
→ normalize
→ validate
→ fixture tests
```

Expose results through a temporary/internal Tauri command.

## Milestone 3 — Rightel

Implement:

```text
authenticate
→ token
→ packages
→ normalize
→ validate
→ fixture tests
```

## Milestone 4 — MCI

Implement:

```text
HTML
→ safe embedded-data extraction
→ parse
→ normalize
→ validate
→ fixture tests
```

Ensure all packages are extracted regardless of official-page pagination.

## Milestone 5 — Samantel

Implement:

```text
HTML
→ safe objectData extraction
→ parse
→ normalize
→ validate
→ fixture tests
```

Do not require subscriber information for general package collection.

## Milestone 6 — Refresh + Cache

Implement:

* concurrent collection;
* partial success;
* local cache;
* atomic writes;
* startup cache loading;
* per-operator freshness.

## Milestone 7 — Filters

Implement domain filters and tests.

## Milestone 8 — Recommendations

Implement:

* BestValue;
* Cheapest;
* MostData;
* 30-day recommendations;
* budget recommendation;
* combined recommendation;
* night recommendation;
* deterministic explanations.

## Milestone 9 — Production UI

Implement:

* Home recommendations;
* package browser;
* filters;
* sorting;
* refresh status;
* stale/error states;
* details;
* comparison.

## Milestone 10 — Hardening

Perform:

* parser regression coverage;
* malformed-data tests;
* offline behavior tests;
* Tauri capability review;
* dependency review;
* production logging review;
* Windows release build;
* installer testing.

---

# 90. Architectural Decision Rules

When implementation reveals an unknown, prefer decisions in this order:

1. preserve correctness;
2. preserve local-first architecture;
3. preserve source transparency;
4. preserve domain semantics;
5. minimize upstream requests;
6. minimize dependencies;
7. minimize complexity;
8. optimize performance only where measurable.

Do not sacrifice semantic correctness merely to make all operator data fit a simpler model.

---

# 91. Source Changes

External operator behavior is not part of BastehYab's stable contract.

The stable contract is:

```text
InternetPackage
```

Therefore:

```text
Operator changes API
       ↓
Collector changes
       ↓
Normalizer may change
       ↓
InternetPackage remains stable
       ↓
Recommendation/UI remain unaffected
```

This isolation is one of the central architectural goals.

---

# 92. Design Invariants

The following are fundamental:

```text
Operator-specific data
        ↓
must be normalized
        ↓
before domain use
```

```text
Restricted data
        ≠
General data
```

```text
Unknown
        ≠
Zero
```

```text
Collector failure
        ≠
Application failure
```

```text
HTTP 200
        ≠
Valid dataset
```

```text
Cached
        ≠
Fresh
```

```text
Best value
        ≠
Best in every possible sense
```

```text
Combined package
        ≠
Non-internet package
```

```text
Frontend
        ≠
Scraper
```

```text
Official operator source
        =
Only required external dependency
```

---

# 93. Final Product Model

BastehYab should ultimately behave as:

```text
                 BastehYab
                     │
          ┌──────────┴──────────┐
          │                     │
      Live Sources          Local Cache
          │                     │
          └──────────┬──────────┘
                     │
                  Collect
                     │
                  Normalize
                     │
                  Validate
                     │
              Unified Dataset
                     │
          ┌──────────┼──────────┐
          │          │          │
        Browse     Filter     Compare
          │          │          │
          └──────────┼──────────┘
                     │
                Recommend
                     │
                     ▼
              Explainable Result
```

The application should remain small, understandable, deterministic, local, and resilient to individual operator failures.

Complexity should be introduced only when it directly improves correctness, resilience, maintainability, or user experience.
