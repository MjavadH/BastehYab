# AGENTS.md

## Project Overview

**BastehYab** is an open-source, local-first desktop application for discovering, filtering, comparing, and recommending internet packages offered by Iranian mobile operators.

The application collects package information directly from official operator-owned sources, normalizes heterogeneous provider data into a common internal model, and performs all filtering, comparison, and recommendation logic locally on the user's device.

Initial supported operators:

* MCI (Hamrah-e Aval)
* Irancell
* Rightel
* Samantel

Packages containing internet/data must be included even when they also contain other services such as voice minutes, SMS, gifts, or other benefits.

BastehYab is licensed under the **MIT License**.

---

## Core Principles

All implementation decisions must preserve the following principles.

### 1. Local-First

BastehYab is a standalone desktop application.

It must not require:

* a BastehYab backend;
* a hosted database;
* cloud infrastructure;
* user accounts;
* remote configuration;
* telemetry infrastructure;
* analytics services;
* third-party scraping services.

Package collection, normalization, filtering, recommendation, and caching happen locally.

### 2. Direct Official Sources

Package data must be obtained directly from sources operated by the relevant mobile operator.

Do not introduce proxy APIs, scraping-as-a-service providers, mirrors, community datasets, or other intermediaries when collecting package information.

Network access must be intentional and minimal.

### 3. No Runtime Toolchain Requirements

End users must not need development tools or runtimes such as:

* Node.js;
* npm;
* pnpm;
* Yarn;
* Rust;
* Cargo;
* Python;
* Docker.

Development dependencies are acceptable during development and build time.

The distributed application must be usable as a normal desktop application.

### 4. Source Data Is Untrusted

All external responses must be treated as untrusted input.

Never assume that:

* fields exist;
* types remain stable;
* HTML structure remains unchanged;
* API responses are valid;
* prices are valid numbers;
* package identifiers are unique across operators;
* operator endpoints are available;
* a successful HTTP response contains usable package data.

Validate data before normalization and fail safely.

### 5. Explainable Recommendations

Recommendations must be deterministic and explainable.

Do not introduce opaque scoring rules that cannot be explained to the user.

A recommendation should be traceable to measurable properties such as:

* price;
* general internet volume;
* price per GB;
* validity duration;
* package restrictions;
* SIM type;
* included services.

Do not treat restricted, night-only, domestic-only, gift, or otherwise limited traffic as equivalent to unrestricted general internet unless the recommendation explicitly targets that category.

---

# Technology Direction

The primary desktop architecture is:

* **Tauri** for the desktop application shell;
* **Rust** for the application core;
* **React + TypeScript** for the user interface.

The exact versions, libraries, project structure, data contracts, and implementation details belong in `DESIGN.md` or the relevant skill documentation.

Do not replace major architectural choices without explicit approval.

---

# Architectural Boundaries

Maintain a strict separation between presentation and application logic.

## Rust Core

Rust owns business-critical and system-level behavior, including:

* operator communication;
* HTTP requests;
* collectors;
* parsing external responses;
* normalization;
* package validation;
* filtering logic;
* recommendation calculations;
* local cache management;
* concurrency;
* application-level error modeling.

Do not move these responsibilities into React merely because implementation would be convenient.

## Frontend

React/TypeScript is primarily responsible for:

* presentation;
* navigation;
* user interaction;
* filter controls;
* sorting controls;
* package visualization;
* recommendation visualization;
* comparison views;
* loading/error states;
* invoking approved Tauri commands.

The frontend must not independently scrape operator websites.

The frontend must not duplicate recommendation or normalization algorithms already owned by the Rust core.

## Tauri Boundary

Communication between the frontend and Rust must occur through explicit, typed Tauri commands/events.

Keep the exposed command surface small.

Do not expose generic filesystem, shell, arbitrary HTTP, or unrestricted system functionality to the frontend.

---

# Collector Architecture

Each operator must have an independent collector.

A failure in one operator must not prevent other operators from producing usable results.

Conceptually:

```text
MCI ─────────┐
Irancell ────┤
Rightel ─────┼──> Normalize ──> Validate ──> Package Dataset
Samantel ────┘
```

Collectors should execute concurrently where appropriate.

Prefer failure isolation patterns such as independent results rather than all-or-nothing collection.

Each collector must be responsible only for obtaining and interpreting operator-specific source data.

Shared recommendation logic must never depend on operator-specific raw response structures.

---

# Collection Strategy

Use the simplest reliable collection mechanism available from an official operator source.

Preferred order:

1. documented/public structured endpoint, when available;
2. structured endpoint used by the operator's own website;
3. structured data embedded in official HTML/JavaScript;
4. HTML parsing;
5. browser automation only as a last resort.

Do not introduce Playwright, Selenium, Chromium, or another embedded browser solely for scraping unless lighter approaches have been demonstrated to be insufficient and the change has been explicitly approved.

Current known strategies are documented separately and must not be blindly inferred from this file.

---

# Normalization

Operator-specific data must be converted into a shared domain model before being exposed to filtering, comparison, recommendation, or UI layers.

Never make the recommendation engine understand raw MCI, Irancell, Rightel, or Samantel structures.

Preserve distinctions between different traffic allowances.

Examples include:

* general internet;
* night traffic;
* domestic traffic;
* international traffic;
* social/application-specific traffic;
* promotional/gift traffic;
* unlimited traffic;
* time-restricted traffic.

Do not simply add fundamentally different traffic categories together to create an artificially larger package volume.

Combined packages containing internet plus voice/SMS/other services remain valid internet packages and must not be discarded.

---

# Pricing and Units

Use canonical internal units.

Avoid using localized display strings as business values.

For example, internal logic should operate on normalized numeric representations rather than strings such as:

```text
"10 GB"
"30 روز"
"120 هزار تومان"
```

Conversion and localization belong at defined boundaries.

Do not silently guess ambiguous units.

Price-per-GB and similar metrics must only be calculated when the required underlying values are known and semantically compatible.

Handle zero, unlimited, unknown, malformed, and missing values explicitly.

---

# Recommendation Engine

Recommendation logic must operate exclusively on normalized package data.

Filtering must happen before ranking when a recommendation is constrained by user criteria.

Examples of recommendation categories may include:

* best value;
* lowest price;
* most general data;
* best 30-day package;
* most data among 30-day packages;
* cheapest 30-day package;
* best package under a budget;
* best prepaid package;
* best postpaid package;
* best combined package;
* best night package;
* best long-term package.

Recommendation algorithms must be testable independently from collectors and UI.

Every recommendation type must define:

1. eligibility rules;
2. excluded or restricted traffic rules;
3. ranking metric;
4. deterministic tie-breaking behavior;
5. explanation metadata.

Do not label a package as globally "best" when it is only best according to a particular metric.

---

# Filtering

Filtering is part of the domain/application layer, not merely a UI concern.

Filters should be composable and reusable by both package browsing and recommendation logic.

Potential filter dimensions include:

* operator;
* SIM type;
* minimum/maximum price;
* minimum/maximum data;
* validity duration;
* package category;
* combined/internet-only packages;
* restricted/unrestricted data;
* night packages;
* included voice;
* included SMS.

Do not couple filter behavior to visual components.

---

# Freshness and Caching

BastehYab should remain useful when an operator is temporarily unavailable.

A successful collection may be cached locally.

On application startup, previously cached valid data may be displayed immediately while fresh collection runs in the background.

Fresh results replace cached results only after successful validation.

A failed, empty, malformed, or suspicious collection must not automatically destroy a previously valid cache.

Freshness must be tracked independently for each operator.

The UI must be capable of distinguishing fresh data from cached/stale data.

Do not present stale data as freshly collected.

---

# Networking

Keep HTTP behavior conservative.

Use:

* reasonable timeouts;
* bounded retries where appropriate;
* explicit response-size expectations where practical;
* HTTPS official endpoints;
* minimal required headers.

Do not copy complete browser request headers into collectors without evidence that they are required.

Do not persist temporary authentication tokens unless necessary.

When an operator exposes a website-scoped authentication flow, obtain credentials through the same legitimate public website flow rather than hard-coding temporary tokens.

Never commit:

* personal cookies;
* session identifiers;
* temporary bearer tokens;
* captured authentication credentials;
* user-specific secrets.

---

# Concurrency

Operator collection should normally happen concurrently to minimize refresh latency.

Concurrency must remain bounded and intentional.

Do not aggressively parallelize requests against an operator.

A collector should make only the requests necessary to retrieve the relevant package dataset.

Avoid request patterns that could place unnecessary load on operator infrastructure.

---

# Error Handling

Expected failures must not cause application crashes.

Examples:

* timeout;
* DNS failure;
* TLS/network error;
* non-success HTTP status;
* authentication failure;
* invalid JSON;
* unexpected HTML;
* missing fields;
* changed upstream structure;
* normalization failure;
* cache read/write failure.

Use structured errors.

Errors should retain enough context for diagnostics without exposing secrets.

Operator-specific failures must be identifiable.

Prefer partial success over total failure when independent operators are involved.

Avoid `unwrap()` and `expect()` in production Rust paths where failure can reasonably occur.

---

# Observability

Local diagnostic logging is allowed and encouraged where useful.

Logs must:

* remain local;
* avoid secrets;
* avoid full authentication tokens;
* avoid unnecessary raw response dumps;
* provide enough context to identify collector failures.

Do not add remote logging, crash reporting, analytics, or telemetry without explicit approval.

---

# Privacy

BastehYab should collect no user data unless a future feature explicitly requires it.

Do not introduce:

* account identifiers;
* device fingerprinting;
* behavioral analytics;
* advertising identifiers;
* hidden telemetry.

User preferences stored locally should remain local.

---

# Security

Follow least privilege throughout the application.

Tauri capabilities and permissions must be narrowly scoped.

The frontend must not receive unrestricted access to:

* filesystem;
* shell;
* process execution;
* arbitrary URLs;
* operating-system APIs.

Avoid dynamically executing JavaScript obtained from operator websites.

If structured data is embedded inside JavaScript, parse/extract the required data safely rather than evaluating arbitrary remote scripts.

Do not use `eval`-equivalent behavior for scraped content.

External strings rendered in the UI must be treated as untrusted content.

---

# Dependencies

Prefer the standard library and already-approved dependencies where practical.

Before adding a dependency, consider:

* whether it is actually necessary;
* maintenance status;
* security history;
* binary-size impact;
* transitive dependency cost;
* whether existing dependencies already solve the problem.

Do not add large frameworks to solve small problems.

Do not introduce an embedded browser automation stack without explicit approval.

Lockfiles must be committed.

---

# Code Quality

Prefer:

* small focused modules;
* explicit types;
* meaningful names;
* pure functions for domain calculations;
* deterministic behavior;
* clear ownership boundaries;
* reusable domain logic;
* minimal hidden state.

Avoid:

* giant modules;
* duplicated operator-independent logic;
* premature abstractions;
* speculative extensibility;
* unnecessary design patterns;
* magic numbers;
* silent fallbacks;
* catch-all error swallowing.

Comments should explain **why**, not restate obvious code.

---

# Rust Guidelines

Use idiomatic stable Rust.

Prefer:

* explicit domain types;
* enums for finite domain states;
* `Result` for fallible operations;
* structured error types;
* `Option` for genuinely optional values;
* immutable data by default;
* safe Rust.

Avoid `unsafe` unless absolutely necessary and explicitly justified.

Domain calculations should not depend on UI or Tauri-specific types.

Keep Tauri command handlers thin.

---

# TypeScript Guidelines

Use strict TypeScript.

Avoid `any` unless integration constraints make it unavoidable and the reason is documented.

Frontend domain types exposed through Tauri should correspond clearly to serialized Rust contracts.

Do not recreate backend/core business rules in TypeScript.

Components should focus on presentation and interaction rather than domain calculations.

---

# UI Principles

BastehYab targets ordinary desktop users.

The UI should make package discovery understandable without requiring technical knowledge.

Prioritize:

* fast startup;
* clear package information;
* obvious filters;
* understandable recommendations;
* visible freshness;
* useful loading states;
* useful partial-failure states;
* readable comparison views;
* Persian/RTL usability.

Do not expose raw collector/API terminology to normal users.

Recommendation explanations should use understandable metrics such as:

```text
قیمت هر گیگابایت
حجم اینترنت عمومی
مدت اعتبار
محدودیت زمانی
```

instead of internal score values.

---

# Testing

Business-critical behavior must be testable without contacting live operator services.

Tests should cover at minimum:

* normalization;
* unit conversion;
* price conversion;
* filtering;
* recommendation ranking;
* tie-breaking;
* restricted traffic handling;
* combined packages;
* malformed upstream data;
* cache validation.

Collector parsers should use sanitized fixtures representing realistic upstream responses.

Do not make the normal test suite depend on live operator availability.

Live integration checks, if introduced, must be separate from deterministic tests.

When fixing a parser bug caused by an upstream format, add or update a regression fixture where practical.

---

# Upstream Changes

Operator websites and APIs are external systems and may change without notice.

When a collector breaks:

1. identify the actual upstream change;
2. update only the affected collector/parser where possible;
3. preserve the normalized domain contract;
4. add/update regression coverage;
5. avoid unrelated architectural changes.

Do not weaken validation merely to make changed upstream data pass.

---

# Scope Discipline

Do not introduce unrelated product features while implementing a requested task.

In particular, do not add without explicit approval:

* accounts;
* cloud synchronization;
* hosted APIs;
* analytics;
* advertising;
* automatic purchasing;
* payment handling;
* SIM/account login;
* operator account management;
* browser automation;
* background system services;
* auto-update infrastructure.

Implement the smallest complete solution consistent with the existing design.

---

# Documentation Hierarchy

Before making architectural or domain-level changes, consult the relevant project documentation.

Use the following hierarchy:

```text
AGENTS.md
    ↓
DESIGN.md
    ↓
Relevant skills/*/SKILL.md
    ↓
Implementation
```

`AGENTS.md` defines repository-wide invariants and engineering rules.

`DESIGN.md` defines the concrete architecture, domain model, data flow, UX structure, and major technical decisions.

`skills/*/SKILL.md` files provide specialized implementation guidance for particular areas.

Do not duplicate large sections of documentation across these files.

If implementation and documentation disagree, do not silently choose one. Determine whether the implementation is incorrect or the documentation requires an intentional update.

---

# Agent Workflow

Before modifying code:

1. understand the requested task;
2. read `DESIGN.md` when the task touches architecture or domain behavior;
3. read the relevant `SKILL.md`;
4. inspect existing implementation before proposing replacements;
5. identify affected tests and contracts.

While implementing:

1. keep changes focused;
2. preserve architectural boundaries;
3. avoid unrelated refactors;
4. add/update tests alongside behavioral changes;
5. handle external failures explicitly.

Before completing:

1. run relevant formatting;
2. run lint/static checks;
3. run relevant tests;
4. verify Rust compilation;
5. verify frontend TypeScript/build checks when frontend changed;
6. report any checks that could not be run.

Never claim a check passed unless it was actually executed successfully.

---

# Definition of Done

A change is complete when:

* requested behavior is implemented;
* architectural boundaries remain intact;
* relevant tests pass;
* formatting/static checks pass;
* failure paths are handled;
* no secrets or captured credentials were introduced;
* documentation is updated when contracts or architecture changed;
* no unnecessary dependency was introduced;
* the application remains standalone and local-first.

When upstream operator behavior is involved, completion also requires verifying that parsing assumptions are represented by tests or fixtures where practical.

---

# Non-Negotiable Invariants

Unless explicitly changed by project maintainers:

1. BastehYab remains a standalone desktop application.
2. There is no required BastehYab backend or cloud service.
3. Package information comes directly from official operator sources.
4. Core collection and recommendation logic remains local.
5. Operator collectors remain isolated from each other.
6. Raw operator formats never become the shared domain model.
7. Restricted traffic is not silently treated as unrestricted traffic.
8. Combined packages containing internet remain eligible packages.
9. One failed operator must not invalidate successful results from others.
10. Previously valid cache is not destroyed by a failed refresh.
11. Recommendations remain deterministic and explainable.
12. The frontend does not independently scrape operator websites.
13. Temporary captured credentials are never hard-coded.
14. Remote operator JavaScript is never blindly executed.
15. End users are not required to install development dependencies.
