# Collectors Skill

## Purpose

This skill defines how BastehYab discovers, retrieves, parses, validates, tests, and maintains package data from official Iranian mobile operator sources.

It applies to all code related to:

```text
src-tauri/src/collectors/
```

and collector-specific fixtures/tests.

Read this skill before:

* implementing a new operator collector;
* modifying an existing collector;
* investigating an upstream breakage;
* changing HTTP behavior;
* adding authentication logic;
* changing embedded-data extraction;
* adding or updating collector fixtures.

This skill supplements:

```text
AGENTS.md
DESIGN.md
```

Repository-wide rules in `AGENTS.md` and architectural decisions in `DESIGN.md` take precedence.

---

# 1. Collector Mission

A collector has one primary responsibility:

> Reliably obtain the package dataset exposed by an operator's official infrastructure and convert the upstream response into an operator-specific typed representation suitable for normalization.

Conceptually:

```text
Official Operator Source
        ↓
HTTP
        ↓
Response Validation
        ↓
Operator-Specific Parsing
        ↓
Raw Typed Packages
        ↓
Normalizer
```

A collector is not responsible for deciding which package is best.

It must not implement:

* recommendation scoring;
* user filters;
* price-per-GB ranking;
* UI formatting;
* Persian localization;
* cross-operator comparison.

Those responsibilities belong elsewhere.

---

# 2. Official Sources Only

Collectors must retrieve package information directly from infrastructure controlled by the relevant operator.

Allowed examples:

```text
mci.ir
irancell.ir
rightel.ir
portal-api.rightel.ir
samantel.ir
payment.samantel.ir
```

Do not use:

* unofficial package websites;
* price-comparison websites;
* mirrors;
* search-engine caches;
* community-maintained datasets;
* scraping proxy services;
* third-party APIs;
* BastehYab-hosted intermediary APIs.

If ownership or authenticity of a source is unclear, do not silently adopt it.

---

# 3. Source Selection

Use the simplest reliable source capable of providing the required package dataset.

Preferred order:

```text
1. Official structured endpoint
        ↓
2. Structured endpoint used by official website
        ↓
3. Structured data embedded in official HTML
        ↓
4. Official HTML elements/attributes
        ↓
5. Browser automation
```

Browser automation is a last resort.

Do not introduce:

* Playwright;
* Selenium;
* Chromium;
* WebDriver;
* browser embedding

unless simpler collection strategies have been demonstrated to be insufficient and the architectural change has been explicitly approved.

---

# 4. Network Discovery

When investigating an operator website, determine where package data actually originates.

Inspect:

* initial document response;
* Fetch/XHR requests;
* embedded JSON;
* JavaScript variables containing data;
* HTML data attributes;
* pagination behavior;
* authentication requests;
* API responses.

Do not assume that visible client-side pagination implies network pagination.

For example:

```text
Showing 1–10 of 40 packages
```

does not necessarily mean four HTTP requests are required.

The complete dataset may already exist in the initial HTML.

---

# 5. Browser Requests Are Evidence, Not Implementation Templates

Captured browser traffic is useful for understanding the official frontend.

Do not blindly copy entire browser requests into production collectors.

A browser request may contain irrelevant headers such as:

```text
Accept-Language
Cache-Control
Pragma
Sec-Fetch-Dest
Sec-Fetch-Mode
Sec-Fetch-Site
sec-ch-ua
sec-ch-ua-mobile
sec-ch-ua-platform
Connection
```

Start with the minimum required request.

Add a header only when:

1. the upstream source requires it; or
2. it has a clear semantic purpose.

Do not cargo-cult captured headers.

---

# 6. Cookies

Do not hard-code captured cookies.

Never commit:

* browser session cookies;
* F5/session cookies;
* tracking cookies;
* analytics cookies;
* personal authentication cookies.

If an endpoint works without cookies, do not send them.

If an official public flow genuinely requires cookies, create and manage a local request session programmatically and document why the cookies are required.

Cookies must never come from a developer's captured browser session.

---

# 7. Authentication Tokens

Temporary tokens must be obtained dynamically through the official website flow when required.

Never commit or hard-code captured bearer tokens.

Bad:

```rust
const RIGHTEL_TOKEN: &str = "eyJhbGciOi...";
```

Correct conceptual flow:

```text
Authenticate through official public website flow
        ↓
Receive temporary token
        ↓
Keep token in memory
        ↓
Use token for package request
        ↓
Discard when application exits / expires
```

Do not log full tokens.

---

# 8. HTTP Client

Prefer a shared configured HTTP client.

Recommended stack:

```text
reqwest
tokio
```

The shared client should provide sensible defaults for:

* HTTPS;
* connection timeout;
* request timeout;
* redirect policy;
* User-Agent;
* connection reuse.

Collectors may customize behavior only when required by their upstream source.

Do not create a completely new HTTP stack for every operator.

---

# 9. User-Agent

Where accepted, use an identifiable application User-Agent.

Conceptually:

```text
BastehYab/<version>
```

Do not pretend to be Chrome merely because captured browser requests used Chrome.

If an operator endpoint demonstrably rejects non-browser User-Agents, document that exception close to the collector implementation and cover the behavior where practical.

---

# 10. Request Timeouts

Every network request must have a bounded timeout.

A collector must never wait indefinitely.

Timeout values should accommodate normal Iranian network conditions without making application refresh excessively slow.

Avoid extreme values in either direction.

Timeout failures must become structured collector errors.

---

# 11. Retry Policy

Retries must be conservative.

Default policy:

```text
initial request
    ↓
transient failure?
    ↓
at most one retry
```

Potentially retryable failures include:

* temporary connection failure;
* timeout;
* selected 5xx responses.

Do not blindly retry:

* parsing errors;
* schema mismatches;
* deterministic 4xx errors;
* invalid package data;
* authentication contract errors.

Authentication expiration may trigger a fresh authentication attempt where the collector protocol explicitly supports it.

Do not create retry loops.

---

# 12. Request Volume

Collectors must minimize load on operator infrastructure.

One refresh should issue only the requests necessary to obtain the package dataset.

Avoid:

* crawling unrelated pages;
* fetching individual package pages when the list already contains required data;
* requesting the same resource repeatedly;
* parallel request storms;
* brute-force endpoint discovery during normal application operation.

Discovery work belongs to development, not runtime.

---

# 13. Concurrency

Different operators should normally be collected concurrently by the refresh orchestrator.

Within one operator collector, concurrency should only be introduced when the upstream protocol genuinely requires multiple independent requests.

Do not aggressively parallelize requests to the same operator.

The collector itself must not assume that another collector succeeded.

---

# 14. Collector Isolation

Each operator has an isolated implementation.

Expected structure:

```text
collectors/
├── mod.rs
├── irancell.rs
├── mci.rs
├── rightel.rs
└── samantel.rs
```

Do not create one giant collector containing conditional logic such as:

```rust
match operator {
    Operator::Mci => { /* hundreds of lines */ }
    Operator::Irancell => { /* hundreds of lines */ }
    ...
}
```

Operator-specific upstream behavior belongs in its own module.

Shared HTTP utilities may be reused.

---

# 15. Raw Models

Each collector may define operator-specific response types.

Example conceptually:

```rust
struct RawRightelPackage {
    ...
}
```

or:

```rust
struct RawIrancellPackage {
    ...
}
```

Raw models should reflect upstream semantics rather than prematurely forcing them into the shared domain model.

Do not make one universal `RawPackage` for all operators.

The normalization layer exists specifically because upstream structures differ.

---

# 16. Tolerant Deserialization

External APIs change.

When using `serde`, avoid unnecessary rigidity.

Optional upstream fields should generally be modeled as optional when absence is legitimate.

Unknown additional JSON fields should normally be tolerated.

Do not reject an entire API response merely because the operator added an unrelated field.

At the same time, do not silently accept missing fields required to identify or interpret a package.

Balance forward compatibility with semantic validation.

---

# 17. HTML Collection

When the source is HTML:

```text
GET document
    ↓
Verify expected page
    ↓
Locate package source
    ↓
Extract only required data
    ↓
Parse safely
```

Do not depend on broad CSS selectors when a more semantically stable identifier exists.

Prefer:

* stable IDs;
* meaningful data attributes;
* known script variables;
* structured embedded data.

Avoid selectors based solely on:

* visual position;
* generated CSS classes;
* nth-child relationships;
* cosmetic wrapper hierarchy.

---

# 18. Embedded JavaScript Data

Some official pages embed package data in JavaScript variables.

Example conceptually:

```javascript
var packages = [...];
```

BastehYab may extract the data representation.

It must not execute arbitrary remote JavaScript.

Forbidden approaches include:

```text
eval
JavaScript engine execution
browser execution solely to obtain the variable
```

Preferred approach:

```text
HTML
 ↓
Locate known assignment
 ↓
Extract balanced data expression
 ↓
Convert/parse safely
 ↓
Typed raw model
```

Do not use a naive regex such as:

```regex
var packages = (.*);
```

for complex nested data if it can terminate incorrectly on nested objects, strings, escaped characters, or semicolons.

Use a parser or a bounded balanced-delimiter extraction strategy where appropriate.

---

# 19. JavaScript-Like Objects

Embedded objects may not always be strict JSON.

Potential differences include:

* single quotes;
* unquoted keys;
* trailing commas;
* JavaScript booleans/null;
* escaped strings;
* nested arrays/objects.

Do not execute the object as JavaScript.

Use a safe parsing/conversion strategy appropriate to the actual upstream syntax.

Do not implement a large JavaScript interpreter for this purpose.

If the upstream representation changes materially, update the parser and fixture coverage.

---

# 20. Response Validation

HTTP success does not mean collector success.

Before parsing, validate basic expectations.

For JSON endpoints consider:

* status code;
* Content-Type when reliable;
* body presence;
* expected top-level structure.

For HTML sources consider:

* status code;
* non-empty document;
* expected page markers;
* expected embedded data marker or package container.

An HTML login/error page returned with HTTP 200 must not replace valid cached package data.

---

# 21. Collection Success

A collector is successful only when it produces a credible package dataset.

The following must not automatically count as success:

```text
HTTP 200 + zero packages
HTTP 200 + parse warnings for every package
HTTP 200 + unrelated HTML
HTTP 200 + authentication page
```

At least one valid internet-containing package should normally be present for a general package collector.

Do not rely on a hard-coded historical package count.

Bad:

```rust
if packages.len() != 40 {
    return Err(...);
}
```

Better:

```text
Dataset is non-empty
Required package identity is present
A meaningful number of records are parseable
Dataset semantics match expected package source
```

---

# 22. Suspicious Dataset Protection

Collectors protect the local cache from obviously bad upstream results.

Examples of suspicious results:

* previously many packages, now parser returns zero;
* every package has missing price;
* every package lacks internet information;
* HTML source marker disappeared;
* API returned a different application payload.

The collector/validation pipeline should return failure or partial status rather than claiming a clean success.

Do not invent package data to compensate.

---

# 23. Partial Parsing

One malformed package should not necessarily invalidate an otherwise valid dataset.

Conceptually:

```text
50 upstream records
        ↓
47 valid raw packages
3 malformed records
        ↓
Collector result: partial/success-with-warnings
```

Whether a malformed record is skipped depends on whether it can be safely identified and isolated.

Do not silently ignore malformed records.

Record a sanitized warning.

If failures indicate the entire schema changed, fail the collector instead of returning misleading partial data.

---

# 24. Diagnostics

Collector diagnostics should help identify upstream breakage.

Useful diagnostic information:

```text
operator
request stage
HTTP status
parser stage
raw record count
parsed record count
rejected record count
high-level failure category
```

Do not include:

* bearer tokens;
* cookies;
* full authentication headers;
* personal information;
* unnecessarily large raw responses.

---

# 25. Collector Errors

Use structured error categories.

Conceptually:

```rust
enum CollectorError {
    Network,
    Timeout,
    HttpStatus,
    Authentication,
    UnexpectedContent,
    InvalidResponse,
    Parse,
    Validation,
}
```

An error should retain operator and stage context.

Example:

```text
operator: rightel
stage: package_request
category: authentication
```

rather than only:

```text
something went wrong
```

---

# 26. Public vs Internal Errors

Internal diagnostic errors may be detailed.

UI-facing errors must be sanitized.

Example internal:

```text
Rightel package request returned HTTP 401 after token refresh
```

Possible UI message:

```text
دریافت اطلاعات رایتل ناموفق بود.
```

Do not expose internal URLs, credentials, response bodies, or implementation details unnecessarily to ordinary users.

---

# 27. Collector Output

Collectors should return operator-specific raw package data plus collection metadata.

Normalization should happen through the corresponding normalizer.

Conceptually:

```rust
struct RawCollection<T> {
    operator: Operator,
    fetched_at: DateTime<Utc>,
    source_url: String,
    records: Vec<T>,
    warnings: Vec<CollectorWarning>,
}
```

Exact types may differ if the implementation benefits from another clean representation.

The important boundary is:

```text
Collector
    ↓
Raw operator-specific data

Normalizer
    ↓
InternetPackage
```

---

# 28. No Recommendation Logic

Never do this inside a collector:

```rust
packages.sort_by_best_value();
```

Never discard a package because:

```text
it is expensive
it has poor value
another operator has a better package
```

Collectors collect facts.

Recommendations interpret normalized facts.

---

# 29. No UI Formatting

Collectors must not generate strings such as:

```text
"۱۵۰ هزار تومان"
"۱۰ گیگ"
"۳۰ روزه"
"بهترین بسته"
```

Preserve upstream strings only when they are actual source data.

Display formatting belongs to the UI.

---

# 30. Package Inclusion

The collector must not intentionally exclude a package merely because it also contains non-internet benefits.

Examples that remain eligible upstream records:

```text
10 GB internet

10 GB internet
+ 100 minutes

5 GB internet
+ 500 SMS

20 GB internet
+ voice
+ SMS
```

A package with no internet/data component is outside the primary BastehYab package dataset and may be rejected during semantic normalization/validation.

---

# 31. Operator: Irancell

## Known Official Page

```text
https://irancell.ir/o/1001/mobile-internet-packages
```

## Known Structured Source

```text
GET https://irancell.ir/e/products/5e16bf95d11fd7209ba56b20
```

Current strategy:

```text
GET structured endpoint
        ↓
JSON response
        ↓
Typed Irancell raw model
        ↓
Irancell normalizer
```

No browser automation is required.

---

# 32. Irancell Request Rules

Begin with a minimal request.

Do not copy captured values such as:

```text
f5avra... session cookies
tracking cookies
browser sec-* headers
```

unless future investigation proves a specific value is required.

The observed endpoint is same-origin to the official Irancell website and currently provides package-related structured data.

The known identifier:

```text
5e16bf95d11fd7209ba56b20
```

should be defined once in the Irancell collector/configuration.

Do not scatter it throughout tests and implementation.

---

# 33. Irancell Source Evolution

The product identifier may change.

If the endpoint stops working:

1. inspect the official package page;
2. determine whether a new structured request is used;
3. determine whether the identifier moved;
4. update only the Irancell collector;
5. update fixtures/tests;
6. preserve the normalized contract.

Do not immediately introduce browser automation.

---

# 34. Operator: Rightel

## Official Frontend

```text
https://package.rightel.ir/
```

## API Host

```text
https://portal-api.rightel.ir
```

Rightel requires a website-scoped authentication step before package retrieval.

---

# 35. Rightel Authentication

Request:

```text
POST /user-management/api/v1/auth/authenticate
```

Host:

```text
https://portal-api.rightel.ir
```

Payload:

```json
{
  "username": "website"
}
```

Expected response shape conceptually:

```json
{
  "data": {
    "token": "<temporary token>"
  },
  "error": null
}
```

The collector must validate that a non-empty token exists before proceeding.

Never use a token captured during development.

---

# 36. Rightel Package Request

After authentication:

```text
GET /extra-package/api/v1/extra-package-direct/web-site/purchasable-package
```

Header:

```text
Authorization: Bearer <token>
```

The official frontend has been observed adding:

```text
?d=<timestamp>
```

Treat this as a probable cache-busting parameter unless evidence shows otherwise.

Generate it dynamically if required.

Do not hard-code observed timestamp values.

---

# 37. Rightel Token Lifecycle

Initial strategy:

```text
No token
   ↓
Authenticate
   ↓
Store token in memory
   ↓
Fetch packages
```

If a package request fails specifically because authentication expired:

```text
Authentication failure
        ↓
Discard token
        ↓
Authenticate once
        ↓
Retry package request once
```

Do not create an infinite authentication loop.

Persistent token storage is unnecessary for the initial design.

---

# 38. Rightel Security

Never log:

```text
Authorization header
full JWT
authentication response containing token
```

Diagnostic logs may state:

```text
Rightel authentication succeeded
```

without exposing credential material.

---

# 39. Operator: MCI

## Official Page

```text
https://mci.ir/internet-plans
```

Current observations show the package dataset is available in the initial document.

The official page may visually paginate the list, for example:

```text
1–10 of 40
```

but this pagination is client-side.

Do not make separate requests for each visual page unless future upstream behavior actually changes.

---

# 40. MCI Structured Data

Known embedded variable:

```text
packegesObj
```

Note the upstream spelling.

Do not silently rename the searched source marker to `packagesObj`.

Internal Rust types may use correct naming, but extraction must target the actual upstream marker.

Preferred strategy:

```text
GET /internet-plans
        ↓
HTML
        ↓
Locate packegesObj assignment
        ↓
Safely extract data expression
        ↓
Parse raw packages
        ↓
Normalize
```

---

# 41. MCI HTML Fallback

The page has also been observed exposing package-related HTML elements and attributes.

If embedded structured data becomes unavailable but stable semantic HTML still contains the full dataset, HTML parsing may be used.

Prefer whichever official representation is:

* complete;
* semantically rich;
* stable;
* safely parseable.

Do not maintain two complicated parsers without a concrete resilience benefit.

If fallback parsing is implemented, tests must cover both paths.

---

# 42. MCI Pagination

The collector's goal is the complete package dataset.

Never intentionally return only the first visible page because the official UI displays ten records at a time.

Collection should be based on source data, not visual pagination state.

---

# 43. MCI Liferay Details

The MCI website may expose Liferay-specific values such as:

```text
p_auth
portlet identifiers
action URLs
```

Do not use them merely because they exist in the HTML.

Only adopt a Liferay action endpoint if package collection genuinely requires it and the behavior has been verified.

Prefer the simpler initial-document strategy while it remains complete and reliable.

---

# 44. Operator: Samantel

## Official Page

```text
https://payment.samantel.ir/package
```

Current observations show package definitions embedded in the initial HTML.

Known variable:

```text
objectData
```

Preferred strategy:

```text
GET page
    ↓
HTML
    ↓
Locate objectData
    ↓
Safely extract data
    ↓
Parse
    ↓
Normalize
```

No browser automation is currently required.

---

# 45. Samantel Subscriber-Dependent Endpoint

The website also exposes behavior through:

```text
POST /api/mediator/samantel/
```

with parameters including concepts such as:

```text
method=getpackagelist
mobile=<subscriber number>
```

This appears subscriber-dependent.

The general BastehYab collector must not require a user's mobile number when public package definitions are sufficient.

Do not:

* request personal subscriber numbers;
* hard-code a random number;
* use a developer's number;
* generate fake subscriber numbers;
* probe subscriber-specific package results during normal collection.

If future product requirements intentionally introduce subscriber-specific offers, that must be designed separately.

---

# 46. Samantel Source Preference

For the general package catalog:

```text
public embedded objectData
```

is preferred over subscriber-specific APIs while it remains complete enough for the product requirements.

If evidence later shows `objectData` is incomplete or stale, investigate the official site's actual behavior before changing strategy.

Do not assume the subscriber endpoint is automatically more authoritative merely because it is an API.

---

# 47. Fixtures

Every collector must have sanitized fixture coverage.

Suggested structure:

```text
src-tauri/tests/fixtures/
├── irancell/
│   ├── packages.json
│   └── malformed.json
│
├── rightel/
│   ├── auth.json
│   ├── packages.json
│   └── malformed.json
│
├── mci/
│   ├── internet-plans.html
│   └── malformed.html
│
└── samantel/
    ├── packages.html
    └── malformed.html
```

Names may evolve with actual test needs.

Do not create meaningless fixture files merely to satisfy this proposed structure.

---

# 48. Fixture Sanitization

Fixtures must preserve the structure necessary to reproduce parser behavior.

Remove:

* tokens;
* cookies;
* personal phone numbers;
* session IDs;
* tracking identifiers;
* unrelated large page sections where unnecessary.

Do not sanitize so aggressively that the parser test no longer resembles the actual upstream format.

---

# 49. Fixture Size

Prefer minimal representative fixtures when possible.

However, when parser correctness depends on surrounding HTML/script context, preserve enough context to test realistic extraction.

A fixture should answer:

> Would this test fail if our parser stopped understanding the operator's real representation?

If not, the fixture may be too synthetic.

---

# 50. Fixture Provenance

When practical, document in test code or fixture metadata:

* operator;
* source type;
* capture date;
* sanitization notes.

Do not store authentication secrets as provenance.

Example:

```text
Operator: MCI
Source: /internet-plans initial HTML
Captured: 2026-08
Sanitized: unrelated HTML removed
```

---

# 51. Parser Tests

At minimum, parser tests should verify:

```text
expected records are discovered
nested values are parsed
optional fields behave correctly
malformed records do not crash
source marker disappearance is detected
empty datasets are not considered healthy
```

Operator-specific edge cases should be added as they are discovered.

---

# 52. No Live Network in Unit Tests

Normal tests must not depend on:

```text
irancell.ir
mci.ir
rightel.ir
samantel.ir
```

Reasons include:

* operator downtime;
* network availability;
* CI restrictions;
* rate discipline;
* upstream changes;
* non-determinism.

Use fixtures for deterministic tests.

---

# 53. Live Collector Checks

Live checks may exist separately for development diagnostics.

They must be explicitly invoked.

Examples conceptually:

```text
cargo test --ignored live_irancell
```

or a dedicated development command.

They must not run automatically during the normal test suite.

Live tests must remain conservative in request count.

---

# 54. Mockable HTTP Boundary

Collector parsing should be separable from network transport.

Prefer architecture conceptually similar to:

```text
fetch()
   ↓
response body
   ↓
parse(body)
   ↓
raw records
```

This allows parser tests to operate entirely on fixtures.

Do not require a real `reqwest::Response` just to test parsing logic.

---

# 55. Parser Function Design

Prefer small explicit functions.

Example:

```text
fetch_irancell()
parse_irancell_response()
```

rather than one enormous function that:

```text
creates client
requests endpoint
parses JSON
normalizes values
writes cache
updates UI
```

Network transport, parsing, normalization, and persistence are separate responsibilities.

---

# 56. Upstream String Handling

Operator strings may contain:

* Persian digits;
* Arabic digits;
* Latin digits;
* non-breaking spaces;
* Persian/Arabic character variants;
* localized separators;
* inconsistent whitespace.

Collectors may perform minimal syntactic cleanup required to parse raw structures.

Semantic conversion belongs primarily to normalization.

Do not aggressively rewrite source strings before the normalizer sees them.

---

# 57. Character Encoding

Official pages are expected to use modern web encodings, normally UTF-8.

Handle Persian text without lossy conversion.

Do not assume ASCII.

If an upstream page declares another encoding, handle it deliberately rather than silently corrupting text.

---

# 58. Pagination

If a future operator uses actual network pagination:

1. identify official pagination parameters;
2. determine termination condition;
3. request pages sequentially or with conservative bounded concurrency;
4. deduplicate only using reliable identifiers;
5. enforce a safety limit.

Never create an unbounded loop based solely on upstream `next` values.

Example safety concept:

```text
maximum expected page traversal
```

should prevent pathological or cyclic pagination.

The exact limit must be based on observed protocol, not an arbitrary tiny number.

---

# 59. Infinite Scroll

If an official website later introduces infinite scroll, first inspect its network behavior.

Do not automate scrolling immediately.

Infinite scroll commonly uses a structured endpoint underneath.

Prefer that endpoint when it is part of the official website flow and can be safely queried.

---

# 60. Anti-Bot or Access Restrictions

Do not attempt to bypass deliberate operator access controls.

If an endpoint begins requiring:

* CAPTCHA;
* interactive anti-bot verification;
* subscriber authentication;
* device-bound credentials;

stop and reassess the collector strategy.

Do not introduce CAPTCHA solving, fingerprint spoofing, challenge bypasses, or similar mechanisms.

Look for another legitimate official public source.

---

# 61. Source Fallbacks

A collector may support multiple official representations if there is a demonstrated resilience benefit.

Example:

```text
MCI embedded structured data
        ↓ unavailable
stable package HTML
        ↓
fallback parser
```

Fallbacks must:

* remain official;
* be tested;
* produce the same raw semantic contract;
* not hide persistent primary-source breakage.

Log/diagnose when fallback behavior is used.

Do not accumulate speculative fallback paths.

---

# 62. Source URLs

Source URLs should be centralized per collector.

Example conceptually:

```rust
const PACKAGE_PAGE_URL: &str = "...";
const PACKAGE_API_URL: &str = "...";
```

Avoid duplicated string literals.

This simplifies:

* maintenance;
* review;
* testing;
* upstream migrations.

---

# 63. Dynamic Query Parameters

Values such as:

```text
timestamps
cache busters
pagination offsets
```

must be generated from their semantics.

Never copy a captured value like:

```text
?d=1786114111770
```

into production code.

---

# 64. Hard-Coded IDs

Some official endpoints may require a stable product/category ID.

Hard-coding such an ID is acceptable when:

* it is part of the observed official source contract;
* no simpler stable discovery mechanism exists;
* it is defined once;
* it is documented;
* collector failure clearly exposes when it becomes invalid.

Do not hide unexplained magic identifiers inside request construction.

---

# 65. Data Completeness

The collector should obtain all relevant package records exposed by its chosen general source.

Do not intentionally limit collection to:

* featured packages;
* first page;
* cheapest packages;
* recommended packages;
* a specific validity period

unless that source itself only represents a clearly scoped category and additional official sources are intentionally collected.

The product needs the complete general internet-package dataset available through the selected official public source.

---

# 66. Operator-Specific Package Categories

Operators may divide packages into categories such as:

```text
daily
weekly
monthly
long-term
special
night
combined
prepaid
postpaid
```

Do not assume category names are semantically identical across operators.

Collectors preserve upstream category information where useful.

The normalizer maps it into BastehYab domain semantics.

---

# 67. Source Ordering

Do not assume upstream record ordering has business meaning unless documented.

Recommendation and UI sorting happen later.

A collector should not depend on:

```text
first package is cheapest
last package is newest
```

unless the source contract explicitly establishes it.

---

# 68. Duplicate Upstream Records

If upstream returns apparent duplicates, preserve them until there is sufficient identity information to safely determine they are duplicates.

Do not deduplicate by display name alone.

Do not deduplicate by:

```text
price + volume
```

alone.

Deduplication is safe only when operator-specific identity semantics support it.

---

# 69. Redirects

Follow redirects only within reasonable HTTP client policy.

Unexpected redirects to:

* login pages;
* unrelated domains;
* anti-bot pages

should be treated as suspicious.

Do not silently parse the redirected page as package data.

---

# 70. Content-Type

Content-Type is useful but not infallible.

If an expected JSON endpoint returns:

```text
text/html
```

treat that as a strong indication of an error/redirect/challenge page.

If an operator incorrectly labels valid JSON, document and test the exception rather than globally disabling content checks.

---

# 71. Maximum Response Size

Package datasets should be relatively small.

Where practical, protect against unexpectedly huge responses.

Do not allow an upstream server to make BastehYab consume unbounded memory.

Response-size protection should be generous enough for legitimate operator pages but finite.

---

# 72. Cancellation

Where supported by the application architecture, collection should be cancellable when:

* the application exits;
* a refresh is superseded;
* shutdown begins.

Do not leave unnecessary background requests running after their results can no longer be used.

---

# 73. Refresh Deduplication

Only one active full refresh should normally exist.

If the user repeatedly clicks Refresh:

```text
Refresh
Refresh
Refresh
Refresh
```

do not launch four independent sets of collector requests.

The refresh orchestrator owns this policy, but collectors must not rely on uncontrolled duplicate execution.

---

# 74. Cache Independence

Collectors do not directly decide which cache file to overwrite.

Correct layering:

```text
Collector
   ↓
Normalizer
   ↓
Validation
   ↓
Refresh Orchestrator
   ↓
Cache
```

Do not write package cache files from `irancell.rs`, `mci.rs`, etc.

---

# 75. Freshness Timestamp

`fetched_at` means:

> the time BastehYab successfully obtained the source response used for the current dataset.

It does not mean:

* package creation time;
* operator publication time;
* cache write time.

Use UTC internally.

---

# 76. Source Timestamp

If an operator provides its own package update timestamp, preserve it separately where semantically useful.

Do not replace BastehYab's `fetched_at` with an upstream timestamp.

These represent different facts.

---

# 77. Development Investigation

When a collector breaks, investigate before coding.

Recommended sequence:

```text
1. Open official source
2. Inspect current document/network behavior
3. Compare with fixture
4. Identify exact upstream change
5. Update parser/request contract
6. Update fixture
7. Add regression test
8. Verify normalization
```

Do not randomly modify selectors until tests pass.

---

# 78. Upstream Breakage Classification

Determine whether breakage is:

```text
endpoint changed
authentication changed
response schema changed
embedded variable renamed
HTML structure changed
data semantics changed
temporary operator outage
access restriction introduced
```

Different failures require different fixes.

Do not treat every failure as a parser bug.

---

# 79. Parser Change Scope

When one operator changes, prefer modifying only:

```text
that collector
that raw model
that normalizer if semantics changed
that operator's fixtures/tests
```

Avoid unrelated refactoring of all collectors during an urgent upstream repair.

---

# 80. Shared Abstractions

Create shared collector abstractions only after repeated real patterns emerge.

Good candidates may include:

* shared HTTP client;
* bounded body reader;
* common request error mapping;
* fixture helpers;
* safe embedded-data extraction utilities.

Avoid premature abstractions such as:

```text
UniversalIranianOperatorScraperFactory
```

that hide fundamentally different protocols.

Clarity is more valuable than artificial uniformity.

---

# 81. Trait Usage

A collector trait may be introduced if it simplifies orchestration.

Conceptually:

```rust
trait Collector {
    type RawPackage;

    async fn collect(&self)
        -> Result<RawCollection<Self::RawPackage>, CollectorError>;
}
```

However, do not force incompatible operator workflows into an awkward trait merely for symmetry.

The refresh orchestrator needs consistent outcomes, not necessarily identical internal implementation.

---

# 82. Logging Levels

Conceptually:

```text
INFO
collector started/completed
record counts

WARN
partial parsing
fallback used
suspicious optional data

ERROR
collector failed
authentication failed
source structure invalid

DEBUG
safe parser diagnostics
```

Never require debug logs to understand whether a collector succeeded.

---

# 83. Development Raw Dumps

During parser development it may be useful to inspect raw responses.

Such behavior must:

* be development-only;
* require explicit action;
* store locally;
* avoid authentication material;
* never be silently enabled in production.

Do not create a permanent production "dump every response" feature.

---

# 84. Tests for Known Operator Contracts

At minimum, preserve regression coverage for these known behaviors.

## Irancell

Verify:

```text
structured JSON can be parsed
package records are discovered
missing optional fields do not crash parser
invalid top-level response fails safely
```

## Rightel

Verify:

```text
auth response token extraction
missing token fails authentication
package response parsing
authentication errors are categorized
```

Do not use a real JWT in fixtures.

Use an obvious placeholder such as:

```text
TEST_TOKEN
```

if a token field is required by the fixture.

## MCI

Verify:

```text
packegesObj is located
complete embedded dataset is extracted
visual pagination does not affect parser
malformed/missing assignment fails safely
```

## Samantel

Verify:

```text
objectData is located
embedded data is parsed
no subscriber number is required
missing/malformed objectData fails safely
```

---

# 85. Acceptance Criteria for a Collector

A collector is not complete merely because it works once on a developer machine.

Before considering a collector complete:

1. the official source is documented;
2. the request flow is understood;
3. unnecessary captured headers/cookies are removed;
4. temporary credentials are obtained dynamically;
5. response parsing is isolated from networking;
6. raw response types are explicit where practical;
7. malformed responses fail safely;
8. empty datasets do not overwrite good data;
9. sanitized fixture coverage exists;
10. parser tests pass offline;
11. secrets are absent from source and fixtures;
12. the collector integrates with the normalizer;
13. failure does not affect unrelated operators;
14. request count is minimal;
15. browser automation is not used unless explicitly approved.

---

# 86. Adding a New Operator

When adding another operator:

```text
Research official source
        ↓
Document request/data flow
        ↓
Choose simplest source
        ↓
Create collector module
        ↓
Define raw types
        ↓
Create sanitized fixture
        ↓
Implement parser
        ↓
Implement normalizer
        ↓
Add parser tests
        ↓
Integrate refresh orchestration
        ↓
Verify failure isolation
```

Do not copy another collector and merely rename fields unless the protocols are genuinely equivalent.

---

# 87. New Operator Checklist

Before implementation answer:

```text
What is the official package page?

Where does the complete dataset originate?

Is it initial HTML, embedded data, or XHR?

Does it require authentication?

Does it require subscriber-specific information?

Does visible pagination cause network requests?

Are package IDs available?

Are prices represented in rial or toman?

How is data volume represented?

Are restricted allowances distinguishable?

Are SIM types exposed?

Are combined benefits exposed?

Can the source be collected without browser automation?
```

Unknown answers should remain explicit investigation tasks.

Do not guess them.

---

# 88. Prohibited Collector Patterns

Do not implement patterns equivalent to:

```rust
// captured developer token
const TOKEN: &str = "...";
```

```rust
// arbitrary user cookie copied from DevTools
.header("Cookie", "...")
```

```rust
// remote code execution
eval(operator_script);
```

```rust
// hides every failure
let packages = fetch().await.unwrap_or_default();
```

```rust
// one bad operator kills everything
join_all(...).await?;
```

```rust
// overwrites good cache with empty parser result
cache.save(Vec::new());
```

```rust
// UI concern inside collector
package.display_price = "۱۵۰ هزار تومان";
```

```rust
// recommendation concern inside collector
packages.sort_by(best_value);
```

```rust
// fake subscriber identity
mobile = "09120000000";
```

```rust
// blind browser emulation
copy_all_devtools_headers();
```

---

# 89. Preferred Collector Shape

A healthy collector should conceptually resemble:

```text
constants/config
      ↓
request functions
      ↓
bounded response handling
      ↓
response parser
      ↓
raw typed records
      ↓
collector metadata/warnings
```

Example conceptual Rust organization:

```rust
const SOURCE_URL: &str = "...";

pub async fn collect(
    client: &reqwest::Client,
) -> Result<RawCollection<RawPackage>, CollectorError> {
    let body = fetch(client).await?;
    let records = parse(&body)?;

    validate_collection_shape(&records)?;

    Ok(RawCollection {
        // ...
    })
}

async fn fetch(
    client: &reqwest::Client,
) -> Result<String, CollectorError> {
    // network only
}

fn parse(
    body: &str,
) -> Result<Vec<RawPackage>, CollectorError> {
    // parsing only
}
```

This is guidance, not mandatory boilerplate.

Use the simplest structure that preserves testability and boundaries.

---

# 90. Final Collector Principle

Collectors are adapters around unstable external systems.

Treat:

```text
Irancell JSON
Rightel API
MCI HTML/JavaScript
Samantel HTML/JavaScript
```

as volatile implementation details.

The rest of BastehYab should not care how an operator publishes its packages.

The intended isolation is:

```text
              Unstable External World
                       │
                       ▼
                  Collectors
                       │
                       ▼
                  Normalizers
                       │
════════════════ Stable Domain Boundary ════════════════
                       │
                       ▼
                InternetPackage
                       │
          ┌────────────┼────────────┐
          ▼            ▼            ▼
       Filters    Recommendations   UI
```

When upstream behavior changes, contain that change as close to the affected collector as possible.

Correctness, safety, minimal network traffic, failure isolation, and maintainability are more important than clever scraping techniques.
