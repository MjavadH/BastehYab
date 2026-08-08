# Normalization Skill

## Purpose

This skill defines how BastehYab converts operator-specific raw package data into the stable shared domain model used by filtering, comparison, caching, recommendations, and the UI.

It applies primarily to:

```text
src-tauri/src/normalizers/
src-tauri/src/domain/
```

and normalization-specific tests and fixtures.

Read this skill before:

* implementing an operator normalizer;
* modifying package-domain fields;
* interpreting operator prices;
* interpreting traffic allowances;
* converting units;
* classifying restricted traffic;
* interpreting validity;
* interpreting SIM types;
* handling combined packages;
* changing unknown-value behavior;
* changing package identity rules.

This skill supplements:

```text
AGENTS.md
DESIGN.md
skills/collectors/SKILL.md
```

Repository-wide rules in `AGENTS.md` and architectural decisions in `DESIGN.md` take precedence.

---

# 1. Normalization Mission

Normalization converts heterogeneous operator data into one stable semantic representation.

Conceptually:

```text
Irancell Raw ──────┐
MCI Raw ───────────┤
Rightel Raw ───────┼──> Normalization ──> InternetPackage
Samantel Raw ──────┘
```

After normalization, downstream code must not need to know:

* which endpoint produced the package;
* which JSON property Irancell used;
* which MCI JavaScript variable contained it;
* which Rightel field represented validity;
* which Samantel string represented price.

The normalized domain model is the stable boundary.

---

# 2. Core Rule: Preserve Meaning

Normalization is semantic conversion, not data beautification.

The goal is:

> Preserve what the operator actually communicates while converting it into consistent BastehYab domain types.

Do not change meaning merely to make values easier to compare.

Examples:

```text
10 GB general + 20 GB night
```

must remain two distinct allowances.

It must not become:

```text
30 GB general
```

Likewise:

```text
30 days
```

must not become:

```text
1 month
```

unless the domain explicitly supports and requires that equivalence.

---

# 3. Never Invent Missing Facts

The normalizer must not guess.

If the source does not establish a value, represent it as unknown where the domain allows.

Never infer:

```text
missing SIM type      → prepaid
missing validity      → 30 days
missing availability  → available
missing SMS           → zero SMS
missing voice         → zero minutes
unknown traffic       → general traffic
unknown currency      → IRR
```

unless the relevant operator contract explicitly guarantees that interpretation.

Unknown is preferable to incorrect.

---

# 4. Conservative Interpretation

When multiple interpretations are possible, choose the interpretation that makes the fewest unsupported claims.

Example:

```text
"10 GB ویژه"
```

If the operator-specific context does not establish whether this means:

* general data;
* night data;
* domestic data;
* application-specific data;
* promotional data;

do not classify it as `General`.

Preserve it as an appropriate unknown/other allowance with the original description.

---

# 5. Normalization Boundary

The intended flow is:

```text
Raw Operator Model
        ↓
Operator Normalizer
        ↓
Normalized Candidate
        ↓
Domain Validation
        ↓
InternetPackage
```

Collectors should not perform recommendation-oriented normalization.

The UI should not repair incomplete normalization.

All cross-operator semantic consistency belongs here.

---

# 6. Operator-Specific Normalizers

Maintain independent normalizers.

Suggested structure:

```text
normalizers/
├── mod.rs
├── irancell.rs
├── mci.rs
├── rightel.rs
└── samantel.rs
```

Each normalizer understands its operator's raw semantics.

Example:

```rust
fn normalize_irancell(
    raw: RawIrancellPackage,
    context: &NormalizationContext,
) -> Result<InternetPackage, NormalizationError>
```

Exact signatures may differ.

Do not create one enormous function containing all operator-specific mapping rules.

---

# 7. Shared Helpers

Common transformations should be shared when semantics are genuinely identical.

Potential shared helpers:

```text
Persian/Arabic digit normalization
whitespace normalization
numeric parsing
byte conversion
money construction
time parsing
safe identifier construction
```

Do not move operator-specific semantic interpretation into a generic helper merely to reduce lines of code.

Bad abstraction:

```text
guess_data_type_from_any_operator_text()
```

Better:

```text
normalize_digits()
parse_numeric_amount()
checked_gib_to_bytes()
```

---

# 8. InternetPackage Contract

The normalized package conceptually contains:

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

The exact implementation may evolve through intentional design changes.

Normalizers must target the shared domain rather than exposing raw operator fields downstream.

---

# 9. Required vs Optional Information

Not every field has equal importance.

A package generally requires enough information to establish:

* operator;
* package identity;
* package name or meaningful source identity;
* internet/data presence;
* usable price semantics where price is advertised.

Optional information may include:

* voice;
* SMS;
* SIM type;
* purchase URL;
* USSD;
* upstream regulatory code;
* detailed description.

Missing optional metadata must not automatically invalidate an otherwise useful package.

---

# 10. Package Identity

Canonical package identity is based on:

```text
operator + external_id
```

Conceptually:

```text
irancell:<external_id>
mci:<external_id>
rightel:<external_id>
samantel:<external_id>
```

Do not assume IDs are globally unique across operators.

---

# 11. External IDs

Prefer an explicit stable operator-provided package identifier.

Possible sources:

* product ID;
* offer ID;
* package ID;
* bundle ID;
* tariff code;
* another stable source identifier.

Do not use the package's list position.

Bad:

```text
mci:17
```

where `17` merely means the 17th item returned today.

---

# 12. Missing Stable IDs

If an operator provides no stable ID, do not immediately invent a random UUID.

A random UUID would change between refreshes and destroy package identity.

Instead investigate whether a deterministic identity can safely be derived from stable source properties.

Possible last-resort deterministic identity inputs may include a carefully defined combination of:

```text
operator
source code
package name
price
validity
allowance characteristics
```

Such fallback identity must be:

* deterministic;
* operator-specific;
* documented;
* tested;
* clearly separated from true operator IDs.

Do not use Rust's unstable/default hash behavior for persistent identity.

Use a defined deterministic encoding/hash if fallback identity is necessary.

---

# 13. Package Names

Preserve the operator's meaningful package name.

Perform only safe syntactic cleanup such as:

* trimming leading/trailing whitespace;
* collapsing clearly accidental repeated whitespace;
* normalizing control characters when necessary.

Do not rewrite marketing names merely to make them prettier.

Do not translate package names inside the normalizer.

Example:

```text
بسته اینترنت ۳۰ روزه ویژه
```

should remain operator content.

---

# 14. Unicode

Persian operator data may contain visually similar Unicode characters.

Examples:

```text
ي / ی
ك / ک
```

Normalization may canonicalize known equivalent Persian/Arabic character variants when doing so improves parsing/search consistency without changing semantic content.

Keep this behavior centralized and tested.

Do not perform broad destructive Unicode transformations.

---

# 15. Digits

Inputs may use:

```text
Latin:
0123456789

Persian:
۰۱۲۳۴۵۶۷۸۹

Arabic:
٠١٢٣٤٥٦٧٨٩
```

Numeric parsing must support all relevant forms.

Use a shared digit-normalization helper.

Example:

```text
"۱۲٬۵۰۰"
        ↓
"12500"
        ↓
12500
```

Display formatting is separate.

---

# 16. Numeric Separators

Operator strings may contain:

```text
,
٬
.
spaces
non-breaking spaces
```

Determine whether a separator means:

* thousands separator;
* decimal separator;
* textual punctuation.

Do not remove every punctuation character blindly.

Parsing rules must be tested against actual operator representations.

---

# 17. Whitespace

External strings may contain:

* regular spaces;
* non-breaking spaces;
* zero-width characters;
* newlines;
* tabs.

Provide safe normalization for parsing purposes.

Preserve meaningful source text where appropriate.

Do not let invisible Unicode characters cause numeric parsing failures when they can be safely normalized.

---

# 18. Money

Canonical internal monetary representation uses integer IRR.

Conceptually:

```rust
struct Money {
    amount: u64,
    currency: Currency,
}
```

Canonical currency:

```text
IRR
```

No floating-point storage for money.

---

# 19. Rial and Toman

The normalizer must know the source unit before conversion.

If the source value is toman:

```text
1 toman = 10 IRR
```

Example:

```text
150,000 toman
        ↓
1,500,000 IRR
```

If the source already provides IRR:

```text
1,500,000 IRR
        ↓
1,500,000 IRR
```

Never multiply by ten merely because the UI commonly displays toman.

Currency interpretation must be operator/source-specific and documented.

---

# 20. Ambiguous Price Units

If a numeric source field is:

```text
150000
```

with no obvious unit, determine its semantics from the official source contract.

Do not decide based on whether the number "looks like" toman.

Once verified, encode the source-unit assumption in the relevant operator normalizer and test it.

If the unit genuinely cannot be established, fail or represent uncertainty rather than silently corrupting price comparisons.

---

# 21. Price Decorations

Price strings may contain:

```text
ریال
تومان
تومن
IRT
IRR
currency symbols
thousands separators
whitespace
```

Strip only known presentation decorations during parsing.

Do not store the decorated string as the canonical price.

---

# 22. Free Packages

A price of zero may be legitimate in promotional scenarios.

Do not automatically reject:

```text
price == 0
```

However, zero-priced packages require credible upstream evidence.

Recommendation algorithms decide whether such packages participate in metrics like price-per-GB.

Normalization merely preserves the verified price.

---

# 23. Overflow Safety

All conversions must use checked arithmetic where overflow is possible.

Examples:

```text
toman × 10
GB × 1024 × 1024 × 1024
```

Never allow malformed upstream numbers to wrap integer values.

Overflow should produce a structured normalization failure.

---

# 24. Canonical Data Unit

Store finite data allowances in bytes.

Conceptually:

```rust
amount_bytes: Option<u64>
```

All operators must eventually map finite traffic into the same unit.

---

# 25. Unit Conversion

Use one project-wide convention.

For BastehYab:

```text
1 KB = 1024 bytes
1 MB = 1024 KB
1 GB = 1024 MB
1 TB = 1024 GB
```

Therefore:

```text
1 GB = 1,073,741,824 bytes
```

Use checked integer arithmetic.

Do not let individual operators define different internal GB semantics.

---

# 26. Fractional Data Amounts

Sources may advertise:

```text
0.5 GB
1.5 GB
2.5 GB
```

Avoid unnecessary floating-point conversion.

Prefer parsing decimal quantities exactly and converting to the canonical byte unit.

For example:

```text
1.5 GB
    ↓
1536 MB
    ↓
1,610,612,736 bytes
```

Use an exact decimal/rational strategy or equivalent checked conversion.

Do not rely on imprecise floating-point equality for domain values.

---

# 27. Mixed Units

Operators may expose:

```text
500 MB
2 GB
1024 MB
1.5 GB
```

All become canonical bytes.

The UI later chooses the most readable display unit.

Do not retain different internal units merely because operators supplied different units.

---

# 28. Unlimited Data

Unlimited is a semantic state, not an enormous number.

Correct:

```rust
DataAllowance {
    amount_bytes: None,
    unlimited: true,
    ...
}
```

Incorrect:

```text
unlimited = 999999999 GB
```

Do not invent a numerical representation for unlimited traffic.

---

# 29. Unknown Amount

Unknown and unlimited are different.

```text
amount_bytes = None
unlimited = false
```

means:

> The amount is not known.

While:

```text
amount_bytes = None
unlimited = true
```

means:

> The source explicitly represents this allowance as unlimited.

Never confuse them.

---

# 30. DataAllowance

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

A package may contain multiple allowances.

Do not flatten them prematurely.

---

# 31. DataAllowanceKind

The shared semantic categories are:

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

Classification must be based on source evidence.

---

# 32. General Data

`General` means data that can reasonably be used as the package's normal unrestricted internet allowance.

It must not have restrictions that materially prevent it from being comparable with ordinary internet volume.

Examples likely to qualify:

```text
10 GB internet
20 GB general internet
5 GB monthly data
```

provided no additional restrictions exist.

---

# 33. Night Data

Use `Night` when the allowance is explicitly limited to a night/time window and is marketed as such.

Example:

```text
30 GB
01:00–07:00
```

should not become general data.

Represent:

```text
kind: Night
time_window: 01:00–07:00
```

when the source establishes the window.

---

# 34. Time Restriction vs Kind

Time restriction and allowance category are related but distinct.

A source may contain general-looking traffic that is available only during specific hours.

Preserve both facts.

Do not remove the `time_window` simply because `kind` already communicates a restriction.

---

# 35. Domestic Data

Use `Domestic` when the source explicitly restricts the allowance to domestic/national traffic.

Examples may include operator terminology equivalent to:

```text
اینترنت داخلی
ترافیک داخلی
سایت‌های داخلی
```

Do not count domestic-only traffic as unrestricted general traffic.

---

# 36. International Data

Use `International` only when the source explicitly distinguishes such traffic.

Do not automatically classify ordinary internet as `International`.

`General` is the normal unrestricted category unless the operator's semantics establish a more specific distinction.

---

# 37. Social Data

Use `Social` when an allowance is restricted to social-network usage as a category.

If it is limited to a specific named application, `ApplicationSpecific` may be more accurate.

Preserve relevant source description.

---

# 38. Application-Specific Data

Examples:

```text
10 GB for a specific video service
5 GB for a messaging application
```

must not become general internet.

Use:

```text
ApplicationSpecific
```

and preserve the relevant service description where available.

---

# 39. Gift Data

Use `Gift` when the operator explicitly describes the allowance as a bonus/gift/promotion and preserving that distinction is meaningful.

Gift data may still have additional restrictions.

Do not assume gift traffic is general merely because it has a volume.

---

# 40. Other Data

`Other` exists for allowances that are known to contain internet traffic but cannot be safely mapped into the more specific shared categories.

Use it instead of guessing.

Preserve a source description when useful.

---

# 41. Multiple Allowances

Example:

```text
Package:
12 GB general
+ 20 GB night
+ 5 GB domestic
```

must become three allowances.

Do not create:

```text
37 GB
```

as a canonical package amount.

Downstream domain helpers may calculate specific totals according to explicit semantics.

---

# 42. Duplicate Allowances

Do not merge allowances merely because their numeric amounts match.

Two 5 GB allowances may have different:

* time windows;
* categories;
* application restrictions;
* promotional semantics.

Merge only when the source clearly represents duplicated fragments of the same allowance and the rule is operator-specific and tested.

---

# 43. Original Descriptions

Use `description` to preserve important restrictions or source semantics that are not fully captured by structured fields.

Do not copy enormous marketing paragraphs into every allowance.

Preserve concise information useful for understanding the allowance.

---

# 44. Validity

Conceptual model:

```rust
enum Validity {
    Hours(u32),
    Days(u32),
    Unknown,
}
```

Normalize explicit source duration into this representation.

---

# 45. Validity Examples

```text
24 hours
    ↓
Hours(24)
```

or, if the project establishes canonical equivalence:

```text
1 day
    ↓
Days(1)
```

Use one consistent policy.

For normal package validity, prefer preserving the unit semantics actually provided when meaningful.

Examples:

```text
7 روز
    ↓
Days(7)

30 روز
    ↓
Days(30)

72 ساعت
    ↓
Hours(72)
```

---

# 46. Month Is Not Automatically 30 Days

Do not blindly normalize:

```text
1 month
```

to:

```text
30 days
```

unless the operator's contract establishes that a "month" means exactly 30 days.

Calendar months and fixed 30-day periods are not inherently identical.

If month-based validity appears and the current domain cannot represent it accurately, update the domain intentionally or preserve it as unknown rather than fabricating precision.

---

# 47. Validity Parsing

Possible Persian representations include:

```text
روزه
روز
ساعته
ساعت
ماه
ماهه
```

Use operator-aware parsing where necessary.

Avoid broad substring logic that could misinterpret package marketing text.

Prefer explicit source fields over extracting validity from the package name.

---

# 48. Name-Derived Validity

Parsing validity from a package name is a fallback, not the preferred source.

Preferred:

```text
raw.validity = 30
raw.validity_unit = day
```

Fallback only if necessary:

```text
"بسته اینترنت ۳۰ روزه ۱۰ گیگ"
```

If name parsing is required:

* make the rule operator-specific;
* test it;
* avoid interpreting unrelated numbers;
* prefer returning unknown over a weak guess.

---

# 49. TimeWindow

Conceptual representation:

```rust
struct TimeWindow {
    start: LocalTime,
    end: LocalTime,
}
```

Examples:

```text
01:00–07:00
02:00–11:00
```

Use a structured local time representation.

Do not attach an arbitrary timezone conversion to daily operator windows.

These windows describe local service eligibility, not UTC timestamps.

---

# 50. Overnight Windows

A range such as:

```text
23:00–07:00
```

crosses midnight.

Do not reject it because:

```text
end < start
```

The time-window model must support overnight semantics.

---

# 51. Unknown Time Window

If a package is clearly night/restricted traffic but the exact hours are unavailable:

```text
kind: Night
time_window: None
```

is preferable to inventing:

```text
01:00–07:00
```

from assumptions about common operator behavior.

---

# 52. Voice Allowance

Conceptual model:

```rust
struct VoiceAllowance {
    minutes: Option<u32>,
    unlimited: bool,
}
```

Examples:

```text
100 minutes
    ↓
minutes: Some(100)
unlimited: false
```

```text
unlimited calls
    ↓
minutes: None
unlimited: true
```

Unknown is distinct from unlimited.

---

# 53. SMS Allowance

Conceptual model:

```rust
struct SmsAllowance {
    count: Option<u32>,
    unlimited: bool,
}
```

Apply the same unknown/unlimited distinction.

---

# 54. Missing Voice or SMS

If the package is clearly internet-only:

```text
voice: None
sms: None
```

is appropriate.

If raw data simply omits these fields and the package composition is ambiguous, do not infer benefits.

`None` means no known allowance object, not necessarily a claim that the operator explicitly states zero.

---

# 55. Combined Package

Conceptual classification:

```rust
enum PackageKind {
    InternetOnly,
    Combined,
}
```

A package is `Combined` when source evidence establishes internet plus another meaningful telecom service/benefit.

Examples:

```text
Internet + voice
Internet + SMS
Internet + voice + SMS
```

---

# 56. Combined Does Not Mean Restricted

These concepts are independent.

Example:

```text
10 GB night-only internet
```

may still be:

```text
PackageKind::InternetOnly
```

with a `Night` allowance.

While:

```text
10 GB general + 100 minutes
```

is:

```text
PackageKind::Combined
```

with a `General` allowance.

---

# 57. Promotional Extras

Not every marketing bonus necessarily makes a package `Combined`.

Examples such as:

```text
lottery entry
discount voucher
membership badge
```

should not automatically make a package combined unless the domain explicitly decides to model such benefits.

The initial `Combined` concept focuses on telecom-service composition.

---

# 58. SIM Types

Conceptual values:

```rust
enum SimType {
    Prepaid,
    Postpaid,
    Tdlte,
    DataSim,
    Other,
}
```

Map only when source semantics support the classification.

---

# 59. SIM Type Mapping

Examples:

```text
اعتباری
    ↓
Prepaid

دائمی
    ↓
Postpaid

TD-LTE
    ↓
Tdlte
```

Operator-specific synonyms should be explicitly mapped.

Do not infer SIM type from price or package duration.

---

# 60. Multiple SIM Types

If the same offer explicitly applies to:

```text
prepaid + postpaid
```

represent:

```text
[Prepaid, Postpaid]
```

Do not duplicate the package unless the upstream source represents distinct offers with distinct identities or semantics.

---

# 61. Unknown SIM Type

If no SIM type is established:

```text
sim_types: []
```

may represent unknown/not specified according to the final domain implementation.

Do not default to prepaid.

If this distinction becomes ambiguous in code, introduce a more explicit representation rather than relying on hidden conventions.

---

# 62. Availability

Conceptual model:

```rust
enum Availability {
    Available,
    Unavailable,
    Unknown,
}
```

Map explicit operator states.

Examples:

```text
active / purchasable
    ↓
Available

disabled / expired / unavailable
    ↓
Unavailable
```

Do not map missing state to `Available`.

---

# 63. Availability vs Listing

A package appearing on an official page is evidence that it is listed.

It is not always proof that every subscriber can purchase it.

If the source does not explicitly establish purchase availability:

```text
Unknown
```

may be more accurate.

---

# 64. Subscriber Eligibility

Some packages may be available only to:

* selected subscribers;
* new subscribers;
* specific regions;
* specific SIM categories;
* targeted campaigns.

Do not flatten such restrictions into global availability.

Preserve relevant eligibility information if the domain supports it.

Otherwise preserve concise source description/metadata and avoid overclaiming.

---

# 65. Purchase Information

Conceptual model:

```rust
struct PurchaseInfo {
    official_url: Option<String>,
    ussd_code: Option<String>,
}
```

Only normalize purchase information actually provided or safely derivable from an explicit official source contract.

Do not guess USSD codes.

Do not construct undocumented purchase URLs.

---

# 66. Purchase URLs

If a raw package contains an official URL:

* require HTTPS where applicable;
* validate expected operator ownership;
* reject obviously malformed URLs;
* do not turn arbitrary external source text into a clickable purchase URL.

External-link security is enforced again at the application/UI boundary.

---

# 67. USSD Codes

Preserve exact functional characters:

```text
*
#
digits
```

Do not run USSD through generic numeric normalization that removes `*` or `#`.

Display directionality can be handled in the UI.

---

# 68. Metadata

Metadata may contain:

```rust
struct PackageMetadata {
    fetched_at: DateTime<Utc>,
    source_url: String,
    regulatory_code: Option<String>,
    offer_code: Option<String>,
    original_description: Option<String>,
}
```

Only place cross-operator meaningful information into shared metadata.

Do not turn metadata into a dumping ground for every upstream field.

---

# 69. fetched_at

`fetched_at` comes from collection context.

It must not be generated independently for every package during a normalization loop.

Packages originating from the same collection should normally share the collection timestamp.

---

# 70. source_url

The source URL should identify the official source from which the package dataset was obtained.

Do not use an unrelated marketing URL merely because it is more user-friendly.

Purchase links and collection source URLs are separate concepts.

---

# 71. Original Description

Preserve a useful original package description when it contains semantics not represented elsewhere.

Avoid storing:

* huge HTML blocks;
* scripts;
* tracking text;
* unrelated page content.

Store text, not executable markup.

---

# 72. HTML Removal

If descriptions originate as HTML:

```text
<p>10 گیگ اینترنت</p>
```

convert safely to text where needed.

Do not preserve executable markup.

Do not execute or render operator HTML directly.

---

# 73. Package Validation

Normalization and validation are related but distinct.

The normalizer constructs a candidate.

Domain validation determines whether the candidate is internally credible.

Conceptually:

```text
Raw
 ↓
Normalize
 ↓
Candidate
 ↓
Validate
 ↓
InternetPackage
```

---

# 74. Fatal Validation Failures

Examples that may make a package unusable:

```text
no stable/deterministic identity
empty meaningful name
no internet allowance at all
invalid/overflowed monetary representation
contradictory impossible allowance state
structurally impossible duration
```

Fatal validation should reject that package, not crash the collector.

---

# 75. Warnings

Examples that may remain usable:

```text
unknown SIM type
unknown availability
missing purchase URL
missing voice details
unrecognized optional metadata
unknown exact time window
```

Warnings should not unnecessarily discard useful package records.

---

# 76. Internet Presence

BastehYab's primary dataset contains packages with internet/data.

A normalized package should normally contain at least one `DataAllowance`.

A package containing only:

```text
voice
SMS
```

should not enter the internet-package dataset.

---

# 77. Zero Data

A finite allowance of zero bytes is usually not a meaningful internet allowance.

Treat zero carefully.

It may indicate:

* malformed upstream data;
* placeholder;
* exhausted/current subscriber state;
* incorrect parsing.

Do not automatically accept it as a normal internet allowance.

Use operator context and validation rules.

---

# 78. Contradictory Unlimited State

Invalid conceptual state:

```text
amount_bytes: Some(10 GB)
unlimited: true
```

unless the domain explicitly defines why both are meaningful.

Prefer one clear representation.

If upstream says something like:

```text
10 GB high-speed, then unlimited throttled
```

this is a richer semantic model and must not be incorrectly flattened into either ordinary 10 GB or unrestricted unlimited traffic.

Preserve such a package conservatively until the domain explicitly supports the distinction.

---

# 79. Fair Usage / Throttled Unlimited

"Unlimited" packages may have fair-use limits or post-threshold throttling.

Do not normalize:

```text
unlimited with 100 GB fair-use policy
```

as equivalent to genuinely unrestricted unlimited traffic.

If the current domain cannot accurately represent the policy:

* preserve the description;
* classify conservatively;
* exclude it from calculations that would become misleading.

Do not assign infinite value to such packages.

---

# 80. Traffic Multipliers

Some operator packages may advertise concepts equivalent to:

```text
X GB international
2X GB domestic
```

Do not simply add both values.

If domestic traffic is charged at a different multiplier or effectively consumes quota differently, preserve the semantic distinction.

The normalizer should model actual allowances rather than marketing arithmetic.

---

# 81. Shared Quotas

A package may expose multiple usage categories drawing from the same underlying quota.

Do not create separate additive allowances if doing so would double-count the same quota.

Example conceptually:

```text
10 GB total,
domestic usage charged at half rate
```

is not necessarily:

```text
10 GB general + 20 GB domestic
```

unless the operator truly provides independent quotas.

Operator-specific semantics must determine the representation.

---

# 82. Bonus Quotas

If the package contains:

```text
10 GB main
+ 5 GB bonus
```

preserve separate allowances when the bonus has distinct eligibility or restrictions.

If the source explicitly establishes that the bonus is identical unrestricted data and immediately available, it may still be preserved as `Gift` rather than silently merged, allowing recommendation logic to decide how to value it.

---

# 83. Price Per GB Is Not Normalization

Do not store calculated ranking values such as:

```text
price_per_gb
value_score
recommendation_score
```

as authoritative source fields in the normalized package.

These are derived domain/recommendation metrics.

Normalization provides the facts needed to calculate them.

---

# 84. General Data Total Is Derived

Do not introduce a canonical field such as:

```text
total_data = sum(all allowances)
```

because different allowance categories are not interchangeable.

Instead, domain helpers calculate explicit metrics such as:

```text
general_data_bytes(package)
night_data_bytes(package)
domestic_data_bytes(package)
```

according to documented semantics.

---

# 85. Search Strings Are Not Domain Values

Do not modify normalized names/descriptions solely for frontend search.

Search-specific normalization may happen in a dedicated search/index helper.

Domain values should preserve meaningful operator content.

---

# 86. Normalization Errors

Use structured errors.

Conceptually:

```rust
enum NormalizationError {
    MissingRequiredField,
    InvalidIdentifier,
    InvalidPrice,
    UnknownPriceUnit,
    InvalidDataAmount,
    UnsupportedDataUnit,
    InvalidValidity,
    InvalidTimeWindow,
    Overflow,
    NoInternetAllowance,
    ContradictoryData,
}
```

Include safe context such as:

```text
operator
external package identifier
field
category
```

Do not include secrets or enormous raw payloads.

---

# 87. Field-Level Context

Useful error:

```text
operator: mci
package_id: ABC123
field: price
error: invalid numeric representation
```

Less useful:

```text
parse failed
```

Preserve enough context for fixture-driven debugging.

---

# 88. Batch Normalization

One invalid raw package should not necessarily invalidate the entire operator dataset.

Conceptually:

```text
Raw records: 40
      ↓
Normalized: 38
Rejected: 2
      ↓
warnings + usable dataset
```

However, widespread normalization failure may indicate an upstream semantic change.

Example:

```text
40 raw
2 normalized
38 rejected
```

should be treated as suspicious rather than silently returning two packages as a healthy refresh.

Threshold policy belongs to collection/validation orchestration and should remain conservative.

---

# 89. No Silent Defaults

Avoid patterns like:

```rust
let price = parse_price(raw.price).unwrap_or(0);
```

or:

```rust
let validity = parse_validity(raw.validity)
    .unwrap_or(Validity::Days(30));
```

or:

```rust
let kind = classify_allowance(raw)
    .unwrap_or(DataAllowanceKind::General);
```

These convert uncertainty into false facts.

Return explicit errors or unknown states.

---

# 90. No `unwrap()` for External Data

Operator input is untrusted.

Do not use `unwrap()` or `expect()` on values derived from upstream data in production normalization paths.

Use:

```text
Result
Option
explicit matching
validated constructors
```

as appropriate.

---

# 91. Validated Constructors

Where domain invariants matter, prefer constructors that enforce them.

Conceptually:

```rust
Money::new_irr(amount)
DataAmount::from_mebibytes(value)
PackageId::new(operator, external_id)
```

rather than scattering invariant checks throughout operator modules.

Keep constructors simple and domain-focused.

---

# 92. Strong Domain Types

Avoid passing ambiguous primitive values across boundaries.

Less desirable:

```rust
price: u64
data: u64
duration: u32
```

Better:

```rust
price: Money
data_allowances: Vec<DataAllowance>
validity: Validity
```

Strong types reduce accidental unit mixing.

---

# 93. Parsing vs Semantic Mapping

Keep these concepts separable.

Example:

```text
"۱۵۰,۰۰۰ تومان"
       ↓
syntactic parsing
       ↓
150000 + TOMAN
       ↓
semantic conversion
       ↓
1,500,000 IRR
```

Likewise:

```text
"۱۰ گیگ شبانه"
       ↓
syntactic extraction
       ↓
10 + GB + "شبانه"
       ↓
semantic classification
       ↓
10 GiB + Night
```

This makes tests easier and failures clearer.

---

# 94. Operator-Specific Knowledge

Rules such as:

```text
Rightel field X is measured in MB
Irancell field Y is expressed in rial
MCI category Z means prepaid
```

belong close to the relevant normalizer and should be tested.

Do not place undocumented operator-specific assumptions in generic domain helpers.

---

# 95. Documentation of Assumptions

When a source contract is not self-evident, document the verified assumption.

Example:

```rust
// The current official Irancell response expresses this field in IRR.
// Verified against the displayed price on the official package page.
```

Avoid comments that merely restate code.

Document why a conversion exists.

---

# 96. Tests

Normalization tests must be deterministic and offline.

Test:

```text
money conversion
digit normalization
unit conversion
fractional data
allowance classification
validity parsing
time windows
SIM mapping
combined packages
unknown values
overflow
malformed numeric strings
missing fields
multiple allowances
```

---

# 97. Shared Unit Tests

Shared helpers should have table-driven tests where practical.

Example cases for numeric parsing:

```text
"1000"
"1,000"
"۱۰۰۰"
"۱٬۰۰۰"
"١٠٠٠"
```

Expected:

```text
1000
```

Include actual formats encountered in operator fixtures.

---

# 98. Money Tests

At minimum test:

```text
100,000 toman → 1,000,000 IRR
1,000,000 IRR → 1,000,000 IRR
zero price
large valid price
overflow
Persian digits
thousands separators
```

Do not rely only on ideal Latin-number inputs.

---

# 99. Data Unit Tests

At minimum:

```text
1 KB
1 MB
1 GB
1 TB
500 MB
1.5 GB
Persian-digit quantities
unsupported unit
overflow
```

Verify exact byte outputs.

---

# 100. Allowance Classification Tests

Test representative cases for:

```text
General
Night
Domestic
International
Social
ApplicationSpecific
Gift
Other
```

Tests should be operator-specific where classification depends on operator terminology.

Do not create a universal Persian keyword classifier and assume it works for every operator.

---

# 101. Validity Tests

Test:

```text
1 day
7 days
30 days
90 days
24 hours
72 hours
Persian digits
missing validity
ambiguous month
malformed duration
```

Do not force ambiguous month semantics merely to make a test pass.

---

# 102. Time Window Tests

Include:

```text
01:00–07:00
23:00–07:00
00:00–08:00
missing time
malformed time
```

Ensure overnight ranges remain valid.

---

# 103. Combined Package Tests

Verify:

```text
internet only
internet + voice
internet + SMS
internet + voice + SMS
night-only internet
internet + non-telecom marketing bonus
```

Classification must follow documented package composition semantics.

---

# 104. Unknown-State Tests

Explicitly test that missing values remain unknown.

Examples:

```text
missing availability != Available
missing SIM type != Prepaid
missing traffic kind != General
missing amount != Unlimited
```

These are important regression tests.

---

# 105. Fixture-Based Normalization

Collector fixtures should feed normalization tests where practical.

Pipeline test:

```text
Fixture
  ↓
Collector parser
  ↓
Raw model
  ↓
Normalizer
  ↓
InternetPackage
```

Assert meaningful semantic results rather than every irrelevant raw property.

---

# 106. Snapshot Testing

Snapshot tests may be useful for normalized fixture outputs if they remain readable and intentional.

Do not blindly accept large snapshot updates after upstream changes.

Review semantic differences.

A changed snapshot can represent a real data-model regression.

---

# 107. Property Tests

Property-based testing may be introduced for high-value conversion helpers if it provides clear benefit.

Good candidates:

```text
digit normalization
checked unit conversion
money conversion
round-trip-safe deterministic IDs
```

Do not add a property-testing dependency merely for trivial coverage.

---

# 108. Recommendation Independence

Normalization tests must not assert:

```text
this package is best
```

That belongs to recommendation tests.

Normalization tests assert facts:

```text
price == X
general allowance == Y
night allowance == Z
validity == 30 days
SIM type == prepaid
```

---

# 109. UI Independence

Normalization must not depend on:

```text
React
TypeScript
Persian UI labels
Tailwind
Tauri window state
```

The Rust domain should remain independently testable.

---

# 110. Cache Compatibility

The normalized model is persisted in local cache.

Therefore changing serialized domain representation may affect existing users.

Before changing serialized structures, consider:

```text
cache schema version
backward compatibility
migration
safe cache invalidation
```

Do not casually rename serialized enum values or fields after release.

Cache-specific migration rules belong to the cache skill.

---

# 111. Serialization Stability

Prefer explicit serialized enum names.

Example:

```text
general
night
domestic
international
social
application_specific
gift
other
```

Avoid serialization that depends on Rust debug formatting.

Use stable `serde` representation.

---

# 112. API Stability Toward Frontend

Normalized domain objects may cross the Tauri boundary.

Changing them can affect TypeScript contracts.

When changing shared fields:

1. update Rust model;
2. update serialization;
3. update TypeScript representation;
4. update relevant tests;
5. update UI consumers;
6. consider cache compatibility.

Do not create separate subtly inconsistent definitions without reason.

---

# 113. Derived Frontend Types

Where practical, generate or carefully mirror TypeScript contracts from Rust serialization semantics.

If manual mirroring is used, keep naming and nullability exact.

Rust:

```rust
Option<T>
```

must not casually become a required TypeScript field.

Unknown-state semantics must survive IPC.

---

# 114. Null vs Missing

Choose a consistent serialization policy for optional domain values.

Do not let:

```text
missing property
null
empty string
0
false
```

all ambiguously mean unknown.

The contract should be explicit and tested.

---

# 115. Empty Strings

Treat source strings containing only whitespace as absent where appropriate.

Do not create:

```text
Some("")
```

for fields whose semantic meaning requires actual content.

Use validated optional-string helpers.

---

# 116. Boolean Parsing

External values may encode booleans as:

```text
true / false
1 / 0
"true" / "false"
"yes" / "no"
operator-specific values
```

Do not use generic truthiness.

Map only known source representations.

Unknown values should not silently become false.

---

# 117. Enumerated Source Values

For operator enums/categories:

```text
known source value
      ↓
known domain mapping
```

Unknown new source value should generally produce:

* `Other`;
* unknown;
* warning;

depending on the domain field.

Do not crash the entire dataset merely because an operator introduced a new optional category.

But do not map an unknown category to a known semantic value without evidence.

---

# 118. Raw Codes vs Display Labels

Prefer machine-readable source codes over labels when both exist.

Example:

```text
typeCode: PREPAID
label: اعتباری
```

Prefer the stable code for semantic mapping.

Use display text as fallback or supporting evidence.

This reduces fragility when labels change cosmetically.

---

# 119. Package Descriptions as Fallback Evidence

Description/name parsing is weaker than structured fields.

Priority:

```text
explicit structured field
        ↓
explicit source code
        ↓
dedicated label
        ↓
package description
        ↓
package name
```

Use weaker sources only when stronger sources are unavailable.

---

# 120. Conflicting Source Fields

If two upstream fields conflict, do not arbitrarily choose one.

Example:

```text
structured validity: 7 days
package name: "30-day package"
```

This may indicate:

* stale marketing text;
* parsing error;
* upstream inconsistency.

Apply an operator-specific documented precedence rule if one source is known to be authoritative.

Otherwise emit a warning or reject the package if the conflict affects correctness materially.

---

# 121. Source Precedence

Each normalizer may define explicit precedence for conflicting representations.

Example conceptually:

```text
API numeric price
    >
formatted display price
```

if verified.

Document and test such precedence.

Do not let precedence emerge accidentally from code order.

---

# 122. Data Provenance

When classification is difficult, preserve enough provenance to diagnose why a normalized value was produced.

This does not require storing every raw field in production.

Tests and normalizer code should make mapping traceable.

Example:

```text
raw category "NIGHT"
    ↓
DataAllowanceKind::Night
```

should be easy to identify in code.

---

# 123. Determinism

Given identical:

```text
raw package
collection context
normalization rules
```

the normalizer must produce identical semantic output.

Do not use:

* random IDs;
* current local time for package identity;
* unordered unstable hashing;
* environment-dependent parsing.

`fetched_at` is supplied by collection context and is intentionally variable across collections.

---

# 124. Locale Independence

Normalization behavior must not depend on the operating system locale.

The same source should normalize identically on:

```text
Persian Windows
English Windows
German Windows
```

Do not rely on locale-sensitive numeric/date parsing without explicit configuration.

---

# 125. Timezone Independence

Daily package time windows represent operator-local usage windows and should not shift based on the user's current system timezone.

Do not convert:

```text
01:00–07:00
```

into another timezone merely because the computer is outside Iran.

Package validity duration is also not a timezone conversion problem.

---

# 126. Iran-Specific Calendar Text

If operators provide Persian/Jalali dates for campaign metadata, do not casually interpret them as package validity.

Campaign start/end dates and package duration are separate concepts.

If future requirements model campaign periods, introduce explicit domain fields.

---

# 127. Tax and Display Price

If operators expose both:

```text
base price
final payable price
tax-inclusive price
```

the domain must intentionally choose which amount represents `price`.

Do not add/subtract tax based on assumptions.

For comparison, prefer the actual advertised payable package price when the official source clearly provides it.

If ambiguity exists, preserve/source-document it and resolve per operator.

---

# 128. Discounts

If a package has:

```text
original price
discounted price
```

do not discard the distinction if it matters.

The current domain may initially use the effective/current payable price.

If original-price display becomes a product requirement, extend the domain intentionally.

Do not overload `price` with two meanings.

---

# 129. Temporary Promotions

Promotional packages remain valid packages when currently advertised.

Do not exclude them simply because they are temporary.

If expiry/eligibility information is available, preserve it separately where supported.

Recommendations may later decide how to present promotional eligibility.

---

# 130. Personalized Packages

A personalized subscriber offer is not automatically equivalent to a public package.

The initial BastehYab dataset targets general official package discovery.

Do not normalize subscriber-specific responses into globally available packages unless the product explicitly introduces personalized collection.

---

# 131. Operator-Specific Volume Semantics

Before mapping an upstream numeric field, establish whether it represents:

```text
bytes
KB
MB
GB
quota units
marketing units
traffic multiplier
```

Never infer solely from field name.

Example:

```text
volume: 10240
```

is meaningless without knowing whether it means:

```text
10240 MB
10240 KB
10240 bytes
```

Encode verified semantics in the operator normalizer.

---

# 132. Operator-Specific Price Semantics

Likewise:

```text
price: 100000
```

requires knowing:

```text
rial?
toman?
pre-tax?
post-tax?
discounted?
```

Do not normalize until semantics are established.

---

# 133. Normalizer Review Checklist

When reviewing a normalizer, verify:

```text
Are all units explicit?

Are all money conversions explicit?

Are missing values preserved?

Are restricted allowances separated?

Are combined packages retained?

Are IDs deterministic?

Are operator assumptions documented?

Are overflow paths checked?

Are malformed values handled?

Are recommendations absent?

Are UI strings absent?
```

---

# 134. Adding a New Normalizer

For a new operator:

```text
Understand raw model
      ↓
Document units
      ↓
Document price semantics
      ↓
Document validity semantics
      ↓
Document allowance semantics
      ↓
Document SIM/category mappings
      ↓
Implement field parsers
      ↓
Implement semantic mapping
      ↓
Validate candidate
      ↓
Add tests
      ↓
Integrate
```

Do not begin by copying another operator's semantic mappings.

---

# 135. New Operator Normalization Questions

Before declaring a new normalizer complete, answer:

```text
What uniquely identifies a package?

What currency/unit is price expressed in?

Does price include tax?

What unit is traffic expressed in?

Can a package have multiple allowances?

How is unrestricted data distinguished?

How is night data distinguished?

How is domestic traffic represented?

Are there application-specific quotas?

How is unlimited represented?

How is validity represented?

How are SIM types represented?

How are combined benefits represented?

How is availability represented?

Are any fields subscriber-specific?

Can unknown values be distinguished from zero?
```

If an answer is unknown, do not silently invent it.

---

# 136. Prohibited Normalization Patterns

Do not implement equivalents of:

```rust
let sim_type = raw.sim_type.unwrap_or("prepaid");
```

```rust
let validity = parse(raw.validity)
    .unwrap_or(Validity::Days(30));
```

```rust
let data = general + night + domestic;
```

```rust
let unlimited_bytes = u64::MAX;
```

```rust
let price = raw.price.parse().unwrap_or(0);
```

```rust
let kind = unknown_kind.unwrap_or(DataAllowanceKind::General);
```

```rust
let id = Uuid::new_v4();
```

for persistent package identity.

Also avoid:

```text
unknown = zero
unknown = false
unknown = general
unknown = prepaid
unknown = available
```

unless an explicit verified source contract defines that mapping.

---

# 137. Preferred Normalizer Shape

Conceptually:

```rust
pub fn normalize(
    raw: RawPackage,
    context: &NormalizationContext,
) -> Result<InternetPackage, NormalizationError> {
    let external_id = normalize_id(&raw)?;
    let name = normalize_name(&raw)?;

    let price = normalize_price(&raw)?;
    let validity = normalize_validity(&raw)?;

    let data_allowances = normalize_allowances(&raw)?;

    let voice = normalize_voice(&raw)?;
    let sms = normalize_sms(&raw)?;

    let sim_types = normalize_sim_types(&raw)?;
    let package_kind =
        determine_package_kind(&data_allowances, &voice, &sms, &raw)?;

    let availability = normalize_availability(&raw)?;

    let package = InternetPackage {
        // ...
    };

    validate(package)
}
```

This is conceptual guidance, not required boilerplate.

Prefer functions that expose semantic decisions clearly.

---

# 138. Separation of Syntax and Semantics

A healthy implementation should make this path visible:

```text
External representation
        ↓
Syntactic parsing
        ↓
Typed raw value
        ↓
Semantic interpretation
        ↓
Canonical domain value
```

Example:

```text
"۱۲ گیگابایت شبانه"
        ↓
12 + "GB" + "night"
        ↓
Data amount + category
        ↓
12 GiB + DataAllowanceKind::Night
```

Avoid giant string-manipulation functions that combine every step invisibly.

---

# 139. Stability Goal

Operator formats are unstable.

The normalized domain should be comparatively stable.

Expected containment:

```text
Operator changes "trafficAmount"
to "volume"
        ↓
Raw model/parser changes
        ↓
Normalizer input adapts
        ↓
InternetPackage unchanged
```

or:

```text
Operator introduces a new traffic category
        ↓
Normalizer investigates semantics
        ↓
Domain changes only if existing categories
cannot represent the real meaning safely
```

Do not modify the shared domain merely because an upstream field was renamed.

---

# 140. Domain Extension Rule

Extend the shared domain only when:

1. the information has real product value;
2. existing fields cannot represent it without losing important semantics;
3. it is not merely an operator implementation detail;
4. downstream behavior can benefit from the distinction.

Do not add:

```text
irancell_special_field
rightel_internal_type
mci_css_category
```

to `InternetPackage`.

Keep operator-specific details at the boundary.

---

# 141. Semantic Loss

Some source information may not fit the initial domain.

When loss is unavoidable:

* preserve critical human-readable description;
* avoid incorrect structured claims;
* document the limitation;
* consider domain extension if it affects filtering/recommendations materially.

Incorrect precision is worse than explicit limitation.

---

# 142. Recommendation Safety

Normalization quality directly affects recommendations.

A mistake such as:

```text
20 GB night
        ↓ incorrectly
20 GB general
```

can make BastehYab recommend a materially worse package as "best value."

Therefore traffic classification, money units, and validity semantics are correctness-critical.

Treat changes to these rules as business-logic changes requiring tests.

---

# 143. Comparison Safety

Cross-operator comparisons are valid only because normalization establishes common semantics.

Before comparing:

```text
Irancell 10 GB
MCI 10 GB
Rightel 10 GB
```

BastehYab must know those values represent semantically comparable traffic.

Equal numbers do not guarantee equal allowances.

The normalizer must preserve restrictions so downstream logic can make that decision.

---

# 144. Final Principle

Normalization is the trust boundary between volatile operator data and BastehYab's stable understanding of a package.

The central rule is:

```text
Preserve facts.
Normalize units.
Preserve restrictions.
Preserve uncertainty.
Never manufacture certainty.
```

The intended architecture is:

```text
          Untrusted / Heterogeneous Sources
                       │
                       ▼
                   Collectors
                       │
                       ▼
                  Raw Models
                       │
                       ▼
                 Normalizers
                       │
             ┌─────────┴─────────┐
             │                   │
       Semantic Mapping      Validation
             │                   │
             └─────────┬─────────┘
                       ▼
══════════════ Stable Domain Boundary ══════════════
                       │
                       ▼
                InternetPackage
                       │
          ┌────────────┼────────────┐
          ▼            ▼            ▼
       Filters      Compare    Recommendations
                       │
                       ▼
                      UI
```

If the source does not provide enough evidence to make a trustworthy semantic claim, preserve that uncertainty instead of guessing.

Correct semantic representation is more important than maximizing the number of populated fields.
