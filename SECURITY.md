# Security Policy

eddacraft takes security reports seriously. This policy explains which versions
of kindling are supported and how to report a vulnerability responsibly.

## Supported Versions

kindling is an open-source local-first memory and continuity engine. Security
fixes are provided for the latest published package versions and the current
`main` branch.

| Version / Branch                 | Supported   |
| -------------------------------- | ----------- |
| Latest published packages        | Yes         |
| `main`                           | Yes         |
| Older releases                   | Best effort |
| Unreleased experimental branches | No          |

Users should upgrade to the latest published packages when a security update is
released.

## Reporting a Vulnerability

Please do not report security vulnerabilities through public GitHub issues,
discussions, or pull requests.

To report a vulnerability, use one of the following channels:

- GitHub private vulnerability reporting, if enabled for this repository
- Email: `security@eddacraft.ai`

Please include as much detail as you can safely provide:

- Affected kindling package, version, commit, or branch
- Affected CLI, hook, adapter, storage backend, server, or package
- Description of the vulnerability and likely impact
- Steps to reproduce or proof of concept
- Relevant operating system, Node.js version, package manager, configuration,
  logs, and database/storage details
- Whether the issue is already public or known to be exploited

Because kindling is open source, source-level details are welcome in private
reports. Please keep exploit details and patches private until we have triaged
the report and agreed a disclosure path.

## Scope

This policy covers vulnerabilities in the kindling source, packages, and
published artefacts, including:

- CLI commands, install paths, and agent hook integrations
- Local storage backends, database handling, search indexes, and migrations
- Adapters for agent tools and development workflows
- Redaction, truncation, and capture controls for sensitive development data
- Published crates, release binaries, the editor extension, scripts, and
  documentation that could cause unsafe use

kindling captures local development activity, which may include tool calls,
commands, diffs, errors, and logs. Reports involving accidental capture,
retention, disclosure, or inadequate redaction of sensitive data are in scope.

The following are generally out of scope unless they demonstrate a clear
security impact:

- Vulnerabilities in Node.js, SQLite, package managers, or agent tools that do
  not require a kindling-specific fix
- Dependency version reports without a reachable exploit path
- Denial-of-service claims without practical impact
- Social engineering or physical attacks
- Issues requiring compromised developer machines, leaked credentials, or
  malicious maintainers
- Automated scanner output without validation

## What To Expect

We aim to acknowledge valid reports within 3 business days.

After acknowledgement, we will triage the report and may ask for additional
information. For accepted vulnerabilities, we will work on a fix, publish
updated packages where appropriate, and credit the reporter if they want to be
credited.

For declined reports, we will explain the reason where it is safe and practical
to do so.

We aim to provide status updates at least every 14 days while an accepted report
remains unresolved.

## Coordinated Disclosure

Please give us a reasonable opportunity to investigate and fix the issue before
publishing details publicly.

We will not ask you to keep a vulnerability confidential forever, but we do ask
that disclosure timing be coordinated to reduce harm to users.

## Safe Harbour

We will not pursue legal action against good-faith security research that:

- Avoids privacy violations, data destruction, service disruption, or
  unauthorised access to third-party systems
- Uses only the minimum access necessary to demonstrate the issue
- Reports the vulnerability promptly and privately
- Does not use the vulnerability for extortion, persistence, or lateral
  movement

This safe harbour does not authorise testing against systems, accounts, data, or
infrastructure you do not own or do not have permission to test.

## Secrets and Sensitive Data

If you accidentally discover secrets, tokens, private keys, credentials, or
sensitive data, stop testing and report the issue immediately. Do not copy,
reuse, disclose, or retain sensitive material beyond what is necessary to
demonstrate the finding.

## Automated scanning

kindling is itself a secret-redaction engine. Automated scanners that do not
model the maskers as sanitizers will flag already-redacted flows. Known
dispositions:

### GitHub secret scanning

Synthetic credentials in `crates/kindling-service/tests/fixtures/**` exist only
to prove the redactor catches them. They are ignored via
`.github/secret_scanning.yml`. Nothing to rotate.

### CodeQL `rust/cleartext-logging`

The query treats `println!` / `assert!` format args as log sinks and does not
treat our maskers as barriers, so it reports taint _through_ redaction.

| Alert    | Location                                                                       | Disposition                           | Rationale                                                                                                                                             |
| -------- | ------------------------------------------------------------------------------ | ------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| #11, #12 | `crates/kindling/src/hook/filter.rs` (unit test `secret_assignment_is_masked`) | used in tests                         | Was assert-message interpolation of already-masked `filter_content` output. Test no longer interpolates that string.                                  |
| #13      | `crates/kindling/tests/capture_mapping.rs`                                     | used in tests                         | Was failure-message interpolation of mapped fixture content (`$ boom`). Test no longer interpolates it.                                               |
| #14      | `crates/kindling/src/commands/log.rs`                                          | false positive: redacted before write | CLI echoes `observation.content` after `KindlingService::append_observation`, which always runs `mask_secrets_with_evidence` (in-process and daemon). |

A CodeQL model pack at `.github/codeql/extensions/kindling-rust/` marks
`mask_secrets` / `mask_secrets_with_evidence` as `log-injection` barriers so
later scans should stop reporting those sanitized flows. If an alert remains
(especially #13, which never touches a masker), dismiss it with the rationale
above rather than adding inline suppressions.
