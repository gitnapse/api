# Security Policy

## Supported Versions

The GitNapse API is under active development (`0.1.x`). Security fixes are
backported to the latest published release when one exists.

## Reporting a Vulnerability

Do **not** open a public issue for security problems. Report privately by
email to the maintainer (see commit history / repo owner) or via GitHub's
private vulnerability reporting on the affected repository
(`gitnapse/gitnapse`, `gitnapse/api`).

Please include:

- Which server version / commit you observed it on
- A minimal reproduction (endpoint, headers, payload)
- Impact assessment if you have one

## Scope

This repository hosts a local-first HTTP server that proxies the GitHub token
of the local user. Anything that could leak that token, bypass the loopback
restriction, or perform unauthorized actions through `/api/*` is in scope.

## Disclosure expectations

- Initial acknowledgement: within 3 business days.
- Plan + fix window: depends on severity, typically ≤ 30 days for high.
