# North Star Tooling Audit Findings

> [!INFO] INFO BLOCK — назначение
> **У нас сейчас:** этот отчёт собирает blocking findings по ToolRegistry и SuiteActionRegistry.
>
> **Technical details (EN):** generated from descriptors; CI/build preflight can fail on ERROR entries.

- Overall OK: `true`

## Tool Registry

No findings.

## Suite Action Registry

No findings.

## Required next behavior

```text
tools.validate fails on ERROR.
suite.doctor fails on ERROR.
Warnings are visible but do not block build unless promoted by policy.
```
