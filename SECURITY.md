# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |

## Reporting a Vulnerability

We take the security of `selection-capture` seriously. If you believe you have found a security vulnerability, please report it to us as described below.

**Please do NOT report security vulnerabilities through public GitHub issues.**

Instead, please report them via email to [zaob.ogn@gmail.com](mailto:zaob.ogn@gmail.com) with the subject line "Security Vulnerability Report".

You should receive a response within 48 hours acknowledging your report. After the initial reply to your report, we will send you a more detailed list of information about the next steps in the response process.

We ask that you give us reasonable time to investigate and mitigate the issue before making it public. We request a minimum of 90 days from the initial report before any public disclosure.

## What to Include

To help us triage your report quickly, please include the following information:

* Type of issue (e.g., buffer overflow, SQL injection, cross-site scripting, etc.)
* Full paths of source file(s) related to the manifestation of the issue
* The location of the affected source code (tag/branch/commit or direct URL)
* Any special configuration required to reproduce the issue
* Step-by-step instructions to reproduce the issue
* Proof-of-concept or exploit code (if possible)
* Impact of the issue, including how an attacker might exploit the issue

## Preferred Languages

We prefer all communications to be in English.

## Security Best Practices

When using `selection-capture`, please consider the following security best practices:

1. **Accessibility Permissions**: Be aware that this library requires Accessibility permissions on macOS. Only grant these permissions to applications you trust.

2. **Input Validation**: When processing captured text, ensure proper validation before using it in sensitive contexts (e.g., command execution, file operations).

3. **Dependencies**: Regularly update dependencies to incorporate security patches. Use tools like `cargo audit` to check for known vulnerabilities.

4. **Clipboard Security**: Be mindful that clipboard operations can potentially expose sensitive data. Ensure your application handles clipboard data appropriately.

## Response Process

Our response process includes:

1. Acknowledgment of your report within 48 hours
2. Investigation and assessment of the reported issue
3. Development and testing of a fix
4. Release of a patched version
5. Public disclosure (coordinated with you)

We will keep you informed of our progress throughout this process.

## Recognition

We believe in recognizing security researchers who help improve our security. Unless you prefer to remain anonymous, we will acknowledge your contribution in our security advisories and release notes.

Thank you for helping keep `selection-capture` and our users safe!
