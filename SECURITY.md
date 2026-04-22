# Security Policy

## Supported Versions

Only the latest release receives security fixes. No backports.

## Reporting a Vulnerability

Do not open a public issue for security vulnerabilities. Use GitHub's private
advisory mechanism instead:

**https://github.com/cunabli/dpaa2-controlplane/security/advisories/new**

Expect acknowledgement within 7 days. The standard disclosure timeline is
90 days from acknowledgement. Vulnerabilities will be disclosed publicly after
that window regardless of patch status.

## Scope

This software runs with elevated privileges and manipulates network resources
of the DPAA2 through its management complex. The following are in scope:

- Input handling from the program (network config parsing, option validation)
- Incorrect teardown leaving persistent network state

Dependency advisories that do not affect code paths exercised by this software
are out of scope. If filing such a report, demonstrate a realistic attack vector;
reports that amount to "a transitive dependency has an advisory" without a
concrete path will be triaged accordingly.
