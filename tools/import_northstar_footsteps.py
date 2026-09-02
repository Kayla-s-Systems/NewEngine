#!/usr/bin/env python3
"""Recover the canonical NorthStar footstep source set from NorthStar Remastered Sony BNKs.

The importer never assumes ASCII string order equals Sony-BNK subsong order. It asks
vgmstream to enumerate every real stream index, matches exact native family names, then
decodes up to eight deterministic native variants selected by that index.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import re
import shutil
import subprocess
from pathlib import Path

BANK_SPECS: dict[str, tuple[str, dict[str, str]]] = {
    "dirt": ("srf-dirt-soft-damp.bnk", {
        "run": "mlb-ambi-heeltoe-fw-run-dirt-soft-damp-",
        "sprint": "mlb-ambi-heeltoe-fw-sprint-dirt-soft-damp-",
        "stealth": "mlb-stealth-heel-fw-vslow-dirt-soft-damp-",
        "land": "mlb-ambi-sland-hard-dirt-soft-damp-",
    }),
    "grass": ("srf-grass.bnk", {
        "run": "boot-grass-med-walk-heel-",
        "sprint": "boot-grass-fast-walk-heel-",
        "stealth": "boot-grass-slow-walk-heel-",
        "land": "boot-grass-land-a-",
    }),
    "metal": ("srf-metal-dense.bnk", {
        "run": "mlb-ambi-heeltoe-fw-run-metal-dense-",
        "sprint": "mlb-ambi-heeltoe-fw-sprint-metal-dense-",
        "stealth": "mlb-stealth-heel-fw-vslow-metal-dense-",
        "land": "mlb-ambi-sland-hard-metal-dense-",
    }),
    "stone": ("srf-stone-asphalt.bnk", {
        "run": "mlb-ambi-heeltoe-run-asphalt-",
        "sprint": "mlb-ambi-heeltoe-sprint-asphalt-",
        "stealth": "mlb-ambi-stealth-heel-fw-slow-asphalt-",
        "land": "mlb-ambi-dland-hard-a-asphalt-",
    }),
    "wood": ("srf-wood-deck.bnk", {
        "run": "mlb-ambi-heeltoe-fw-run-wood-deck-",
        "sprint": "mlb-ambi-heeltoe-fw-sprint-wood-deck-",
        "stealth": "mlb-stealth-heel-fw-vslow-wood-deck-",
        "land": "mlb-ambi-sland-hard-wood-deck-",
    }),
    "mud": ("srf-mud-thick.bnk", {
        "run": "mlb-ambi-heeltoe-fw-run-mud-thick-",
        "sprint": "mlb-ambi-heeltoe-fw-sprint-mud-thick-",
        "stealth": "mlb-stealth-heel-fw-vslow-mud-thick-",
        "land": "mlb-ambi-sland-hard-mud-thick-",
    }),
    "water": ("srf-water-wetness.bnk", {
        "run": "mlb-ambi-heeltoe-fw-run-wet-srf-sweetener-",
        "sprint": "mlb-ambi-heeltoe-fw-sprint-wet-srf-sweetener-",
        "stealth": "mlb-stealth-heel-vslow-wet-srf-sweetener-",
        "land": "mlb-ambi-sland-hard-wet-srf-sweetener-",
    }),
    "snow": ("srf-snow.bnk", {
        "run": "mlb-ambi-heel-run-snow-packed-",
        "stealth": "mlb-stealth-heel-slow-snow-packed-",
        "land": "mlb-ambi-sland-snow-packed-",
    }),
}
STREAM_RE = re.compile(r"stream index:\s*(\d+).*?stream name:\s*([^\r\n]+)", re.S)


MIN_VARIATIONS = 4
MAX_VARIATIONS = 8


EXTRA_FAMILIES: dict[str, dict[str, str]] = {
    "dirt": {
        "walk": "mlb-ambi-heel-fw-vslow-dirt-soft-damp-",
        "walk_toe": "mlb-ambi-toe-fw-vslow-dirt-soft-damp-",
        "stealth_toe": "mlb-stealth-toe-fw-vslow-dirt-soft-damp-",
        "scuff": "mlb-ambi-scuff-hard-dirt-soft-damp-",
        "lift": "mlb-ambi-lift-sprint-dirt-soft-damp-",
    },
    "grass": {
        "walk": "boot-grass-med-walk-heel-",
        "walk_toe": "boot-grass-med-walk-toe-",
        "stealth_toe": "boot-grass-slow-walk-toe-",
        "run_toe": "boot-grass-med-walk-toe-",
        "sprint_toe": "boot-grass-fast-walk-toe-",
        "scuff": "boot-grass-scuff-",
    },
    "metal": {
        "stealth_toe": "mlb-stealth-toe-fw-vslow-metal-dense-",
        "scuff": "mlb-ambi-scuff-hard-metal-dense-",
        "lift": "mlb-ambi-lift-sprint-metal-dense-",
    },
    "stone": {
        "walk": "mlb-ambi-heel-vslow-stone-asphalt-v2-",
        "walk_toe": "mlb-ambi-toe-vslow-stone-asphalt-v2-",
        "stealth_toe": "mlb-ambi-stealth-toe-fw-slow-asphalt-",
        "scuff": "mlb-ambi-scuff-hard-asphalt-",
        "lift": "mlb-ambi-lift-sprint-asphalt-",
    },
    "wood": {
        "walk": "mlb-ambi-heel-fw-vslow-wood-deck-",
        "walk_toe": "mlb-ambi-toe-fw-vslow-wood-deck-",
        "stealth_toe": "mlb-stealth-toe-fw-vslow-wood-deck-",
        "scuff": "mlb-ambi-scuff-hard-wood-deck-",
        "lift": "mlb-ambi-lift-sprint-wood-deck-",
    },
    "mud": {
        "walk": "mlb-ambi-heel-fw-vslow-mud-thick-",
        "walk_toe": "mlb-ambi-toe-fw-vslow-mud-thick-",
        "stealth_toe": "mlb-stealth-toe-fw-vslow-mud-thick-",
        "scuff": "mlb-ambi-scuff-soft-mud-thick-",
    },
    "water": {
        "walk": "mlb-ambi-heel-fw-vslow-wet-srf-sweetener-",
        "walk_toe": "mlb-ambi-toe-fw-vslow-wet-srf-sweetener-",
        "stealth_toe": "mlb-stealth-toe-vslow-wet-srf-sweetener-",
        "scuff": "mlb-ambi-scuff-hard-wet-srf-sweetener-",
        "lift": "mlb-ambi-lift-fw-sprint-wet-srf-sweetener-",
    },
    "snow": {
        "walk": "mlb-ambi-heel-med-snow-packed-",
        "stealth_toe": "mlb-stealth-toe-slow-snow-packed-",
        "scuff": "mlb-ambi-scuff-snow-packed-",
    },
}


def run_checked(command: list[str], timeout: int = 90) -> subprocess.CompletedProcess[str]:
    result = subprocess.run(command, capture_output=True, text=True, encoding="utf-8", errors="replace", timeout=timeout)
    if result.returncode != 0:
        raise RuntimeError(f"command failed ({result.returncode}): {subprocess.list2cmdline(command)}\n{result.stderr[-2000:]}")
    return result


def index_bank(vgmstream: Path, bank: Path) -> list[tuple[int, str]]:
    result = run_checked([str(vgmstream), "-m", "-s", "1", "-S", "0", str(bank)])
    rows = [(int(index), name.strip()) for index, name in STREAM_RE.findall(result.stdout + result.stderr)]
    if not rows:
        raise RuntimeError(f"vgmstream enumerated no streams: {bank}")
    return rows


def decode(vgmstream: Path, bank: Path, subsong: int, output: Path) -> None:
    output.parent.mkdir(parents=True, exist_ok=True)
    run_checked([str(vgmstream), "-i", "-s", str(subsong), "-o", str(output), str(bank)], timeout=30)
    if not output.is_file() or output.stat().st_size <= 44:
        raise RuntimeError(f"decoded WAV is missing/truncated: {output}")


def source_label(game_root: Path) -> str:
    parts = list(game_root.parts)
    for index, part in enumerate(parts):
        if part.lower().startswith("the.last.of.us.part.ii.remastered"):
            return "/".join(parts[index:])
    return "The.Last.of.Us.Part.II.Remastered/build/pc/main"


def record_for(output_root: Path, output: Path, game_root: Path, bank: Path, surface: str,
               mode: str, variation: int, subsong: int, stream_name: str, **extra: str) -> dict[str, object]:
    record: dict[str, object] = {
        "surface": surface,
        "mode": mode,
        "variation": variation,
        "canonical": output.relative_to(output_root).as_posix(),
        "bank": bank.relative_to(game_root).as_posix(),
        "subsong": subsong,
        "stream_name": stream_name,
        "bytes": output.stat().st_size,
        "sha256": hashlib.sha256(output.read_bytes()).hexdigest(),
    }
    record.update(extra)
    return record


def import_footsteps(game_root: Path, vgmstream: Path, output_root: Path) -> list[dict[str, object]]:
    banks = game_root / "core_Unpacked" / "soundbank4"
    records: list[dict[str, object]] = []
    selected: dict[str, dict[str, list[tuple[int, str]]]] = {}
    cache: dict[Path, list[tuple[int, str]]] = {}

    def indexed(bank: Path) -> list[tuple[int, str]]:
        if bank not in cache:
            cache[bank] = index_bank(vgmstream, bank)
        return cache[bank]

    for surface, (bank_name, families) in BANK_SPECS.items():
        bank = banks / bank_name
        if not bank.is_file():
            raise FileNotFoundError(f"missing NorthStar footstep bank: {bank}")
        rows = indexed(bank)
        selected[surface] = {}
        for mode, pattern in families.items():
            hits = [(index, name) for index, name in rows if pattern in name.lower()]
            if len(hits) < MIN_VARIATIONS:
                raise RuntimeError(f"{surface}/{mode}: exact family '{pattern}' has only {len(hits)} streams")
            selected[surface][mode] = hits[:MAX_VARIATIONS]
            for variation, (subsong, stream_name) in enumerate(selected[surface][mode], 1):
                output = output_root / surface / f"{mode}_{variation:02d}.wav"
                decode(vgmstream, bank, subsong, output)
                records.append(record_for(output_root, output, game_root, bank, surface, mode,
                                          variation, subsong, stream_name))

    # Secondary native foot-contact families used by the runtime contact-phase state machine.
    for surface, families in EXTRA_FAMILIES.items():
        bank_name = BANK_SPECS[surface][0]
        bank = banks / bank_name
        rows = indexed(bank)
        for mode, pattern in families.items():
            hits = [(index, name) for index, name in rows if pattern in name.lower()]
            if len(hits) < MIN_VARIATIONS:
                raise RuntimeError(f"{surface}/{mode}: exact family '{pattern}' has only {len(hits)} streams")
            for variation, (subsong, stream_name) in enumerate(hits[:MAX_VARIATIONS], 1):
                output = output_root / surface / f"{mode}_{variation:02d}.wav"
                decode(vgmstream, bank, subsong, output)
                records.append(record_for(output_root, output, game_root, bank, surface, mode,
                                          variation, subsong, stream_name))

    # NorthStar's snow bank has no dedicated sprint-snow-packed family. Preserve that fact
    # explicitly rather than manufacturing a synthetic sound source.
    snow_bank = banks / "srf-snow.bnk"
    for variation, (subsong, stream_name) in enumerate(selected["snow"]["run"], 1):
        source = output_root / "snow" / f"run_{variation:02d}.wav"
        output = output_root / "snow" / f"sprint_{variation:02d}.wav"
        shutil.copyfile(source, output)
        records.append(record_for(
            output_root, output, game_root, snow_bank, "snow", "sprint", variation,
            subsong, stream_name, fallback_from="run",
            reason="NorthStar srf-snow.bnk has no dedicated sprint-snow-packed family",
        ))

    puddle = banks / "srf-water-puddle.bnk"
    puddle_hits = [(index, name) for index, name in indexed(puddle)
                   if "footstep-heel-water-splish-" in name.lower()]
    if len(puddle_hits) < MIN_VARIATIONS:
        raise RuntimeError(f"water splish family has only {len(puddle_hits)} streams")
    for variation, (subsong, stream_name) in enumerate(puddle_hits[:MAX_VARIATIONS], 1):
        output = output_root / "water" / f"splish_{variation:02d}.wav"
        decode(vgmstream, puddle, subsong, output)
        records.append(record_for(output_root, output, game_root, puddle, "water", "splish",
                                  variation, subsong, stream_name))
    return records



def write_ysncd_manifest(output_root: Path, records: list[dict[str, object]]) -> None:
    surfaces = tuple(BANK_SPECS)
    available = {(str(record["surface"]), str(record["mode"])) for record in records}

    def clips_for(surface: str, mode: str, *, cue_mode: str | None = None) -> list[dict[str, object]]:
        actual_mode = mode
        # Metal has no matching ambient forward heel family in this bank; authored walk
        # intentionally reuses the complete forward-run contact at reduced cue pitch/gain.
        if (surface, actual_mode) not in available and surface == "metal" and mode == "walk":
            actual_mode = "run"
        rows = sorted(
            (record for record in records
             if str(record["surface"]) == surface and str(record["mode"]) == actual_mode),
            key=lambda record: int(record["variation"]),
        )
        if len(rows) < MIN_VARIATIONS:
            raise RuntimeError(f"{surface}/{mode}: only {len(rows)} canonical source variations")
        clips = []
        for record in rows:
            variation = int(record["variation"])
            clips.append({
                "name": f"{surface}_{cue_mode or mode}_{variation:02d}",
                "source": str(record["canonical"]),
                "weight": 1.0,
                "gain": 1.0,
                "pitch": 1.0,
            })
        return clips

    def descriptor(name: str, surface: str, mode: str, clips: list[dict[str, object]]) -> dict[str, object]:
        gain_ranges = {
            "walk": [0.78, 0.92], "run": [0.94, 1.03], "sprint": [0.98, 1.06],
            "stealth": [0.68, 0.84], "land": [0.96, 1.06], "toe": [0.62, 0.82],
            "lift": [0.55, 0.76], "scuff": [0.74, 0.96],
        }
        pitch_ranges = {
            "walk": [0.96, 1.02], "run": [0.97, 1.03], "sprint": [0.99, 1.05],
            "stealth": [0.96, 1.02], "land": [0.97, 1.03], "toe": [0.98, 1.04],
            "lift": [0.98, 1.05], "scuff": [0.95, 1.04],
        }
        semantic = "toe" if mode.endswith("_toe") else mode
        surface_gain = {
            "dirt": 0.98, "grass": 0.94, "metal": 1.05, "stone": 1.06,
            "wood": 1.00, "mud": 0.92, "water": 0.90, "snow": 0.93,
        }[surface]
        surface_pitch = {
            "dirt": 0.995, "grass": 0.990, "metal": 1.018, "stone": 1.020,
            "wood": 1.004, "mud": 0.975, "water": 0.968, "snow": 0.982,
        }[surface]
        authored_gain = [round(value * surface_gain, 5) for value in gain_ranges[semantic]]
        authored_pitch = [round(value * surface_pitch, 5) for value in pitch_ranges[semantic]]
        base_distance = {
            "dirt": 23.0, "grass": 21.0, "metal": 29.0, "stone": 30.0,
            "wood": 26.0, "mud": 19.0, "water": 21.0, "snow": 20.0,
        }[surface]
        # AudioRuntime concurrency groups are exclusive/replace-old. Ordinary foot phases
        # must overlap naturally (left/right tails, toe/lift layers), so only landing uses
        # an exclusive group; the global voice arbiter owns the remaining budget.
        concurrency = "player.footsteps.land" if semantic == "land" else ""
        priority = {"land": 70, "sprint": 58, "run": 52, "walk": 46, "stealth": 42,
                    "scuff": 40, "toe": 36, "lift": 34}.get(semantic, 40)
        layers: list[dict[str, object]] = []
        cue_clips = clips
        # Wet-surface main contacts keep the separately sourced puddle sweetener as a second
        # audio-domain layer. Toe/lift/scuff phases do not re-trigger it.
        if surface == "water" and semantic in {"walk", "run", "sprint", "stealth", "land"}:
            sweeteners = clips_for("water", "splish", cue_mode=f"{mode}_splish")
            cue_clips = clips + sweeteners
            layers = [
                {"name": "body", "role": "body", "clip_names": [c["name"] for c in clips], "gain": 1.0, "pitch": 1.0},
                {"name": "splish", "role": "sweetener", "clip_names": [c["name"] for c in sweeteners],
                 "gain": {"stealth": 0.30, "walk": 0.40, "run": 0.52, "sprint": 0.64, "land": 0.74}[semantic],
                 "pitch": 1.0},
            ]
        return {
            "name": name,
            "bus": "sfx",
            "looping": False,
            "concurrency_group": concurrency,
            "priority": priority,
            "repeat_avoidance": 2 if len(clips) >= 6 else 1,
            "spatial_policy": "spatial",
            "gain_range": authored_gain,
            "pitch_range": authored_pitch,
            "attenuation": {"min_distance": 0.35, "max_distance": base_distance + (8.0 if semantic == "land" else 0.0),
                            "curve": "inverse", "rolloff": 1.12 if surface in {"metal", "stone"} else 1.20,
                            "curve_points": []},
            "clips": cue_clips,
            "layers": layers,
        }

    cues: list[dict[str, object]] = []
    for surface in surfaces:
        for mode in ("walk", "run", "sprint", "stealth", "land"):
            if mode == "sprint" and surface == "snow":
                # import_footsteps materializes snow/sprint_XX from the native run family.
                pass
            cues.append(descriptor(f"{surface}_{mode}", surface, mode, clips_for(surface, mode)))
        for phase_mode in ("walk_toe", "stealth_toe", "run_toe", "sprint_toe", "lift", "scuff"):
            if (surface, phase_mode) in available:
                cues.append(descriptor(
                    f"{surface}_{phase_mode}", surface, phase_mode,
                    clips_for(surface, phase_mode),
                ))

    manifest = {
        "schema": "newengine.ysncd.manifest.v1",
        "version": 1,
        "source_contract": "northstar.northstar.footsteps.source.v1",
        "policy": [
            "physics owns contact/surface identity only; FPS gameplay owns gait/contact-phase semantics",
            "left/right foot, heel/toe delay, scuff and lift scheduling are runtime semantics rather than WAV naming conventions",
            "YSNCD owns weighted randomization and repeat avoidance; gameplay never selects an individual variation",
            "each source family retains up to eight native NorthStar variations; pools with six or more avoid the last two selections",
            "surface-specific gain/pitch/attenuation preserve hard/soft/wet material character before dynamic contact physics modulation",
            "water contact cues compose a wet-surface body with a separately sourced puddle splish sweetener",
            "snow sprint intentionally reuses native snow run recordings because the source bank has no sprint-snow-packed family",
        ],
        "cues": cues,
    }
    (output_root / "footsteps.ysncd.json").write_text(
        json.dumps(manifest, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--game-root", type=Path, required=True,
                        help="NorthStar build/pc/main directory")
    parser.add_argument("--vgmstream", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    game_root = args.game_root.resolve()
    vgmstream = args.vgmstream.resolve()
    output = args.output.resolve()
    records = import_footsteps(game_root, vgmstream, output)
    write_ysncd_manifest(output, records)
    provenance = {
        "schema": "northstar.northstar.footsteps.source.v1",
        "source_title": "The Last of Us Part II Remastered",
        "source_root": source_label(game_root),
        "decoder": "vgmstream-cli",
        "selection_policy": (
            "up to eight exact vgmstream-indexed native streams per canonical family (minimum four); "
            "snow sprint intentionally reuses snow run because no dedicated sprint family exists; "
            "secondary walk/toe/scuff/lift families are extracted for runtime contact-phase composition"
        ),
        "records": records,
    }
    (output / "source.northstar.json").write_text(
        json.dumps(provenance, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
    )
    print(f"NorthStar footsteps recovered output='{output}' records={len(records)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
