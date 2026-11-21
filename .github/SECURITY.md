# Security Policy

## Supported versions

Security fixes are made for the latest release of StarDash only. Please update to the
newest version before reporting a problem.

## Reporting a vulnerability

Please do **not** open a public issue for security problems.

Report them privately through GitHub instead:
[Report a vulnerability](https://github.com/crazmoe/StarDash/security/advisories/new).
Reports in English or German are welcome.

Helpful details:

- StarDash version, Linux distribution and database
- What an attacker could do, and the steps to reproduce it

You will get an answer as soon as possible. Once a fix is released, the advisory is
published and you are credited, unless you prefer not to be.

## Scope

In scope is the StarDash code in this repository and the packages built from it
(the Debian packages under Releases and in the package repository). Of particular interest:
access to reports of another company, bypassing the sign-in of the web interface, and personal
data that the scrubbing in the clients lets through.

Vulnerabilities in the clients belong to the repository of the client concerned;
vulnerabilities in the STARFACE phone system itself should be reported to STARFACE GmbH.
