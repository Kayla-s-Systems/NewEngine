#!/usr/bin/env python3
"""P8 reference-module completeness scanner for North Star Engine.

The uploaded archives are treated as behavioral reference implementations. This
scanner does not compare or import their concrete code. It verifies that every
reference module is represented by a North Star domain/gateway/capability path,
that the required crates are actually part of the Cargo workspace, and that the
source tree has the minimum DTO/provider/runtime seams needed to continue toward
reference parity without violating Domains & Gateways.
"""
from __future__ import annotations

import argparse
import json
import pathlib
import sys
from dataclasses import dataclass

try:
    import tomllib  # py3.11+
except ModuleNotFoundError:  # pragma: no cover
    tomllib = None  # type: ignore[assignment]

SCRIPT_ROOT = pathlib.Path(__file__).resolve().parent
ENGINE_ROOT = SCRIPT_ROOT.parent
REPO_ROOT = ENGINE_ROOT.parents[1]
CRATES = ENGINE_ROOT / "crates"
WORKSPACE = ENGINE_ROOT / "Cargo.toml"
REFERENCE_MATRIX = ENGINE_ROOT / "config" / "reference" / "module_completeness_matrix.v1.json"
CAPABILITY_MATRIX = ENGINE_ROOT / "config" / "capabilities" / "engine_capability_matrix.v1.json"
CONFORMANCE_MATRIX = ENGINE_ROOT / "config" / "conformance" / "provider_conformance_matrix.v1.json"
AUDIT_DOC = REPO_ROOT / "docs" / "audits" / "P8_REFERENCE_MODULE_COMPLETENESS_20260531.md"
TAKESOME_INVARIANTS = REPO_ROOT / "tools" / "scripts" / "takesome" / "tools" / "invariants.py"
TAKESOME_VALIDATION = REPO_ROOT / "tools" / "scripts" / "takesome" / "tools" / "validation.py"
TAKESOME_TOOLS_RUN = REPO_ROOT / "tools" / "scripts" / "takesome" / "tools" / "run.py"
TAKESOME_CLI = REPO_ROOT / "tools" / "scripts" / "takesome" / "cli.py"
SUITE_REGISTRY = REPO_ROOT / "tools" / "scripts" / "takesome" / "suite" / "registry.py"


@dataclass(frozen=True)
class Finding:
    severity: str
    check: str
    path: pathlib.Path
    message: str
    excerpt: str = ""

    def render(self) -> str:
        suffix = f": {self.excerpt.strip()}" if self.excerpt.strip() else ""
        return f"[{self.severity}] {self.check}: {self.path}: {self.message}{suffix}"


def rel(path: pathlib.Path) -> pathlib.Path:
    try:
        return path.relative_to(REPO_ROOT)
    except ValueError:
        return path


def read(path: pathlib.Path) -> str:
    return path.read_text(encoding="utf-8", errors="replace") if path.exists() else ""


def read_json(path: pathlib.Path) -> tuple[dict, list[Finding]]:
    if not path.exists():
        return {}, [Finding("ERROR", "p8-json", rel(path), "required JSON file is missing")]
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except Exception as exc:
        return {}, [Finding("ERROR", "p8-json", rel(path), f"invalid JSON: {exc}")]
    if not isinstance(value, dict):
        return {}, [Finding("ERROR", "p8-json", rel(path), "root must be a JSON object")]
    return value, []


def workspace_members() -> tuple[set[str], list[Finding]]:
    if not WORKSPACE.exists():
        return set(), [Finding("ERROR", "workspace", rel(WORKSPACE), "Cargo workspace manifest is missing")]
    if tomllib is None:
        return set(), [Finding("ERROR", "workspace", rel(WORKSPACE), "tomllib is unavailable; use Python 3.11+")]
    try:
        data = tomllib.loads(WORKSPACE.read_text(encoding="utf-8"))
    except Exception as exc:
        return set(), [Finding("ERROR", "workspace", rel(WORKSPACE), f"invalid Cargo.toml: {exc}")]
    members = data.get("workspace", {}).get("members", [])
    if not isinstance(members, list):
        return set(), [Finding("ERROR", "workspace", rel(WORKSPACE), "workspace.members must be a list")]
    return {str(item).replace("\\", "/").rstrip("/") for item in members}, []


def all_crate_dirs() -> set[str]:
    if not CRATES.exists():
        return set()
    return {
        f"crates/{path.name}"
        for path in CRATES.iterdir()
        if path.is_dir() and (path / "Cargo.toml").exists()
    }


def scan_workspace_coverage(members: set[str]) -> list[Finding]:
    findings: list[Finding] = []
    missing = sorted(all_crate_dirs() - members)
    for crate in missing:
        findings.append(Finding(
            "ERROR",
            "workspace-member-coverage",
            rel(WORKSPACE),
            "crate has Cargo.toml but is not compiled by the workspace",
            crate,
        ))
    return findings


def scan_reference_matrix_shape(matrix: dict) -> list[Finding]:
    findings: list[Finding] = []
    if matrix.get("schema") != "northstar.reference_module_completeness.v1":
        findings.append(Finding("ERROR", "reference-matrix", rel(REFERENCE_MATRIX), "unexpected schema id"))
    policy = matrix.get("policy") or {}
    for key in (
        "reference_archives_are_behavioral_etalon",
        "engine_must_preserve_domains_and_gateways",
        "no_direct_backend_dependency_on_reference_code",
        "providers_receive_dtos_not_world",
        "runtime_apply_stages_own_mutation",
        "production_gaps_must_be_visible",
        "all_dataset_archives_must_be_mapped",
    ):
        if policy.get(key) is not True:
            findings.append(Finding("ERROR", "reference-policy", rel(REFERENCE_MATRIX), f"policy.{key} must be true"))
    domains = matrix.get("domains")
    if not isinstance(domains, list) or not domains:
        findings.append(Finding("ERROR", "reference-matrix", rel(REFERENCE_MATRIX), "domains must be a non-empty list"))
    archive_coverage = matrix.get("archive_coverage")
    if not isinstance(archive_coverage, list) or not archive_coverage:
        findings.append(Finding("ERROR", "reference-matrix", rel(REFERENCE_MATRIX), "archive_coverage must list every uploaded dataSet archive"))
    return findings


def scan_archive_coverage(matrix: dict) -> list[Finding]:
    """Require every known dataSet archive to have an explicit parity status.

    The source tree must not claim reference/dataSet completeness by checking only
    a hand-picked subset. This gate keeps all uploaded archives visible while
    still allowing honest `visible_gap` or `missing_gateway` states for systems
    that have not reached production parity.
    """

    findings: list[Finding] = []
    coverage = matrix.get("archive_coverage") or []
    if not isinstance(coverage, list):
        return findings

    seen: set[str] = set()
    valid_status = {"covered", "visible_gap", "missing_gateway", "external_reference"}
    domain_archives = discover_reference_archives(matrix)

    for idx, item in enumerate(coverage):
        if not isinstance(item, dict):
            findings.append(Finding("ERROR", "dataset-archive-coverage", rel(REFERENCE_MATRIX), f"archive_coverage[{idx}] must be an object"))
            continue
        archive = str(item.get("reference_archive", "")).strip()
        if not archive:
            findings.append(Finding("ERROR", "dataset-archive-coverage", rel(REFERENCE_MATRIX), f"archive_coverage[{idx}] missing reference_archive"))
            continue
        if archive in seen:
            findings.append(Finding("ERROR", "dataset-archive-coverage", rel(REFERENCE_MATRIX), "duplicate archive coverage record", archive))
        seen.add(archive)

        systems = item.get("mapped_systems")
        if not isinstance(systems, list) or not systems:
            findings.append(Finding("ERROR", "dataset-archive-coverage", rel(REFERENCE_MATRIX), f"{archive} must name mapped_systems"))
        status = str(item.get("northstar_status", "")).strip()
        if status not in valid_status:
            findings.append(Finding("ERROR", "dataset-archive-coverage", rel(REFERENCE_MATRIX), f"{archive} has invalid northstar_status", status))
        if item.get("production_gaps_visible") is not True:
            findings.append(Finding("ERROR", "dataset-archive-coverage", rel(REFERENCE_MATRIX), f"{archive} must keep production_gaps_visible=true"))
        if status == "covered" and archive not in domain_archives:
            findings.append(Finding("ERROR", "dataset-archive-coverage", rel(REFERENCE_MATRIX), f"{archive} cannot be 'covered' without a detailed domains[] contract record"))

    for archive in sorted(domain_archives - seen):
        findings.append(Finding("ERROR", "dataset-archive-coverage", rel(REFERENCE_MATRIX), "detailed domain archive missing from archive_coverage", archive))
    return findings


def scan_domain_routes(matrix: dict, members: set[str]) -> list[Finding]:
    findings: list[Finding] = []
    cap_data, cap_findings = read_json(CAPABILITY_MATRIX)
    conf_data, conf_findings = read_json(CONFORMANCE_MATRIX)
    findings.extend(cap_findings)
    findings.extend(conf_findings)
    cap_records = cap_data.get("records") if isinstance(cap_data, dict) else []
    conf_families = conf_data.get("families") if isinstance(conf_data, dict) else []
    gateways = {str(r.get("engine_gateway", "")) for r in cap_records or [] if isinstance(r, dict)}
    capabilities = {str(r.get("capability_id", "")) for r in cap_records or [] if isinstance(r, dict)}
    families = {str(f.get("family", "")) for f in conf_families or [] if isinstance(f, dict)}

    seen_archives: set[str] = set()
    for idx, domain in enumerate(matrix.get("domains") or []):
        if not isinstance(domain, dict):
            findings.append(Finding("ERROR", "reference-domain", rel(REFERENCE_MATRIX), f"domains[{idx}] must be an object"))
            continue
        archive = str(domain.get("reference_archive", "")).strip()
        if not archive:
            findings.append(Finding("ERROR", "reference-domain", rel(REFERENCE_MATRIX), f"domains[{idx}] missing reference_archive"))
        if archive in seen_archives:
            findings.append(Finding("ERROR", "reference-domain", rel(REFERENCE_MATRIX), "duplicate reference archive", archive))
        seen_archives.add(archive)

        for gateway in domain.get("northstar_gateways") or []:
            if gateway not in gateways:
                findings.append(Finding("ERROR", "reference-gateway", rel(CAPABILITY_MATRIX), f"{archive} target gateway is not declared", str(gateway)))
        for capability in domain.get("required_capabilities") or []:
            if capability not in capabilities:
                findings.append(Finding("ERROR", "reference-capability", rel(CAPABILITY_MATRIX), f"{archive} capability is not declared", str(capability)))
        for family in domain.get("required_conformance_families") or []:
            if family not in families:
                findings.append(Finding("ERROR", "reference-conformance", rel(CONFORMANCE_MATRIX), f"{archive} conformance family is not declared", str(family)))
        for crate in domain.get("required_workspace_crates") or []:
            crate = str(crate).replace("\\", "/").rstrip("/")
            if not (ENGINE_ROOT / crate / "Cargo.toml").exists():
                findings.append(Finding("ERROR", "reference-crate", pathlib.Path(crate), f"{archive} required crate is missing"))
            elif crate not in members:
                findings.append(Finding("ERROR", "reference-crate", rel(WORKSPACE), f"{archive} required crate is not a workspace member", crate))
    return findings


def discover_reference_archives(matrix: dict) -> set[str]:
    return {
        str(domain.get("reference_archive", "")).strip()
        for domain in matrix.get("domains") or []
        if isinstance(domain, dict) and str(domain.get("reference_archive", "")).strip()
    }


def scan_source_tokens(matrix: dict) -> list[Finding]:
    findings: list[Finding] = []
    for domain in matrix.get("domains") or []:
        if not isinstance(domain, dict):
            continue
        archive = str(domain.get("reference_archive", "reference"))
        for item in domain.get("required_source_tokens") or []:
            if not isinstance(item, dict):
                findings.append(Finding("ERROR", "reference-token", rel(REFERENCE_MATRIX), f"{archive} source token record must be an object"))
                continue
            path = ENGINE_ROOT / str(item.get("path", ""))
            if not path.exists():
                findings.append(Finding("ERROR", "reference-token", rel(path), f"{archive} required source file is missing"))
                continue
            text = read(path)
            for token in item.get("tokens") or []:
                if str(token) not in text:
                    findings.append(Finding("ERROR", "reference-token", rel(path), f"{archive} missing source token", str(token)))
    return findings


def scan_tooling_wiring() -> list[Finding]:
    required = {
        TAKESOME_INVARIANTS: ["run_p8_reference_module_completeness_scan", "p8_reference_module_completeness_scan.py"],
        TAKESOME_VALIDATION: ["run_p8_reference_module_completeness_scan", "reference_code"],
        TAKESOME_TOOLS_RUN: ["reference-completeness", "Run P8 reference module completeness scan"],
        TAKESOME_CLI: ["reference-completeness"],
        SUITE_REGISTRY: ["diag.reference.completeness", "run_p8_reference_module_completeness_scan"],
        AUDIT_DOC: ["P8", "ai.zip", "renderer.zip", "Domains & Gateways"],
    }
    findings: list[Finding] = []
    for path, tokens in required.items():
        if not path.exists():
            findings.append(Finding("ERROR", "p8-tooling", rel(path), "required tooling/audit file missing"))
            continue
        text = read(path)
        for token in tokens:
            if token not in text:
                findings.append(Finding("ERROR", "p8-tooling", rel(path), f"missing token {token}"))
    return findings


def run_checks(*, strict_reference_parity: bool = False) -> list[Finding]:
    findings: list[Finding] = []
    members, member_findings = workspace_members()
    findings.extend(member_findings)
    findings.extend(scan_workspace_coverage(members))
    matrix, matrix_findings = read_json(REFERENCE_MATRIX)
    findings.extend(matrix_findings)
    if matrix:
        findings.extend(scan_reference_matrix_shape(matrix))
        findings.extend(scan_archive_coverage(matrix))
        findings.extend(scan_domain_routes(matrix, members))
        findings.extend(scan_source_tokens(matrix))
    findings.extend(scan_tooling_wiring())
    if strict_reference_parity and matrix:
        for domain in matrix.get("domains") or []:
            if isinstance(domain, dict) and domain.get("production_gaps"):
                findings.append(Finding(
                    "ERROR",
                    "reference-parity-gap",
                    rel(REFERENCE_MATRIX),
                    f"{domain.get('reference_archive', 'reference')} declares production gaps",
                    "; ".join(map(str, (domain.get("production_gaps") or [])[:4])),
                ))
        for item in matrix.get("archive_coverage") or []:
            if isinstance(item, dict) and item.get("northstar_status") != "covered":
                findings.append(Finding(
                    "ERROR",
                    "dataset-parity-gap",
                    rel(REFERENCE_MATRIX),
                    f"{item.get('reference_archive', 'reference')} is not covered",
                    str(item.get("northstar_status", "")),
                ))
    return findings


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(prog="p8_reference_module_completeness_scan.py")
    parser.add_argument("--summary-only", action="store_true")
    parser.add_argument("--strict-reference-parity", action="store_true", help="Treat declared production gaps as errors.")
    ns = parser.parse_args(argv)
    findings = run_checks(strict_reference_parity=bool(ns.strict_reference_parity))
    errors = [f for f in findings if f.severity == "ERROR"]
    warnings = [f for f in findings if f.severity == "WARN"]
    if not ns.summary_only:
        for finding in findings:
            print(finding.render())
    print(f"p8 reference contract coverage scan: errors={len(errors)} warnings={len(warnings)}")
    return 1 if errors else 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
