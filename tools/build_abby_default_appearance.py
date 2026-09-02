from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path

import numpy as np
from PIL import Image

ROOT = Path(__file__).resolve().parents[2]
PROFILE = ROOT / "Shared/Source/models/characters/abby/appearance/abby_wlf_default.json"
TEXTURE_DIR = ROOT / "Shared/Source/textures/characters/abby"
REPORT = ROOT / "Intermediate/abby_default_appearance_report.json"


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def resolve_shared(path: str) -> Path:
    return ROOT / "Shared/Source" / path


def forbidden_hits(value: str, forbidden: list[str]) -> list[str]:
    lower = value.lower()
    return [token for token in forbidden if token.lower() in lower]


def reconstruct_clean_head(clean_low: Image.Image, detail_donor: Image.Image) -> tuple[Image.Image, float]:
    clean_low = clean_low.convert("RGB")
    detail_donor = detail_donor.convert("RGB")
    donor_low = detail_donor.resize(clean_low.size, Image.Resampling.LANCZOS)
    donor_low_up = donor_low.resize(detail_donor.size, Image.Resampling.LANCZOS)
    clean_up = clean_low.resize(detail_donor.size, Image.Resampling.LANCZOS)

    clean_np = np.asarray(clean_up, dtype=np.int16)
    donor_np = np.asarray(detail_donor, dtype=np.int16)
    donor_low_np = np.asarray(donor_low_up, dtype=np.int16)
    reconstructed = np.clip(clean_np + (donor_np - donor_low_np), 0, 255).astype(np.uint8)
    image = Image.fromarray(reconstructed, mode="RGB")

    validation = image.resize(clean_low.size, Image.Resampling.LANCZOS)
    a = np.asarray(validation, dtype=np.int16)
    b = np.asarray(clean_low, dtype=np.int16)
    mae = float(np.abs(a - b).mean())
    return image, mae


def validate_profile(profile: dict) -> None:
    forbidden = [str(v).lower() for v in profile.get("forbidden_default_tokens", [])]
    if profile.get("profile") != "clean_default":
        raise SystemExit("Abby default appearance must use profile='clean_default'")
    overlays = profile.get("damage_overlays")
    if overlays != []:
        raise SystemExit(f"Abby default appearance must disable damage overlays, got {overlays!r}")

    head = profile.get("head", {})
    for section in ("base_color", "normal", "roughness"):
        cfg = head.get(section) or {}
        semantic = str(cfg.get("semantic_source", ""))
        source_path = str(cfg.get("source_path", ""))
        for value_name, value in (("semantic_source", semantic), ("source_path", source_path)):
            hits = forbidden_hits(value, forbidden)
            if hits:
                raise SystemExit(
                    f"Abby default head {section}.{value_name} resolves to forbidden damage tokens {hits}: {value!r}"
                )

    semantic = str(head.get("base_color", {}).get("semantic_source", "")).lower()
    if semantic != "abby-nude-head-color.tga":
        raise SystemExit(f"Abby default head base must resolve to abby-nude-head-color.tga, got {semantic!r}")


def main() -> int:
    ap = argparse.ArgumentParser(description="Build clean Abby WLF-default head appearance")
    ap.add_argument("--apply", action="store_true", help="write m00/m01 base-color outputs")
    args = ap.parse_args()

    profile = json.loads(PROFILE.read_text("utf-8"))
    validate_profile(profile)
    head = profile["head"]

    clean_path = resolve_shared(head["base_color"]["source_path"])
    donor_path = resolve_shared(profile["detail_donor"]["path"])
    if not clean_path.is_file():
        raise SystemExit(f"missing clean Abby head source: {clean_path}")
    if not donor_path.is_file():
        raise SystemExit(f"missing Abby head detail donor: {donor_path}")

    clean_low = Image.open(clean_path)
    donor = Image.open(donor_path)
    clean_head, low_frequency_mae = reconstruct_clean_head(clean_low, donor)
    if low_frequency_mae > 2.0:
        raise SystemExit(
            f"clean-head reconstruction failed low-frequency gate mae={low_frequency_mae:.4f} > 2.0"
        )

    output_slots = [str(v) for v in head["base_color"].get("output_slots", [])]
    if output_slots != ["m00_base", "m01_base"]:
        raise SystemExit(f"unexpected Abby default head output slots: {output_slots}")

    outputs: dict[str, dict[str, object]] = {}
    if args.apply:
        for slot in output_slots:
            output = TEXTURE_DIR / f"{slot}.png"
            clean_head.save(output)
            outputs[slot] = {
                "path": str(output),
                "sha256": sha256(output),
                "size": list(clean_head.size),
            }
    else:
        for slot in output_slots:
            output = TEXTURE_DIR / f"{slot}.png"
            outputs[slot] = {
                "path": str(output),
                "exists": output.is_file(),
                "sha256": sha256(output) if output.is_file() else None,
            }

    for slot in ("m00_normal", "m01_normal", "m00_roughness", "m01_roughness"):
        if not (TEXTURE_DIR / f"{slot}.png").is_file():
            raise SystemExit(f"missing matching Abby default head data texture: {slot}.png")

    report = {
        "schema": "northstar.abby.default-appearance-build.v1",
        "profile": profile["profile"],
        "variant": profile["variant"],
        "semantic_head_base": head["base_color"]["semantic_source"],
        "damage_overlays": profile["damage_overlays"],
        "forbidden_default_tokens": profile["forbidden_default_tokens"],
        "clean_source": str(clean_path),
        "clean_source_sha256": sha256(clean_path),
        "detail_donor": str(donor_path),
        "detail_policy": profile["detail_donor"]["policy"],
        "reconstruction_low_frequency_mae": low_frequency_mae,
        "outputs": outputs,
        "applied": args.apply,
    }
    REPORT.parent.mkdir(parents=True, exist_ok=True)
    REPORT.write_text(json.dumps(report, indent=2) + "\n", "utf-8")
    print(
        "abby-default-appearance: PASS "
        f"profile={profile['profile']} damage_overlays=0 low_frequency_mae={low_frequency_mae:.4f} "
        f"apply={args.apply} report={REPORT}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
