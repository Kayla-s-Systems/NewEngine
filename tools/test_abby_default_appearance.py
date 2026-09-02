from __future__ import annotations

import json
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PROFILE = ROOT / "Shared/Source/models/characters/abby/appearance/abby_wlf_default.json"
VARIANTS = ROOT / "Shared/Source/models/characters/abby/variants/abby_variants.json"
MATERIAL = ROOT / "Shared/Source/materials/character_abby.nemat.xml"


class AbbyDefaultAppearanceTests(unittest.TestCase):
    def setUp(self) -> None:
        self.profile = json.loads(PROFILE.read_text("utf-8"))
        self.variants = json.loads(VARIANTS.read_text("utf-8"))
        self.forbidden = [str(v).lower() for v in self.profile["forbidden_default_tokens"]]

    def assert_clean_reference(self, value: str) -> None:
        lower = value.lower()
        hits = [token for token in self.forbidden if token in lower]
        self.assertEqual(hits, [], f"default Abby reference contains damage tokens: {value}")

    def test_default_head_uses_clean_nude_base(self) -> None:
        head = self.profile["head"]
        self.assertEqual(self.profile["profile"], "clean_default")
        self.assertEqual(head["base_color"]["semantic_source"], "abby-nude-head-color.tga")
        self.assertEqual(head["normal"]["semantic_source"], "abby-nude-head-normal.tga")
        self.assertEqual(head["roughness"]["semantic_source"], "abby-head-roughness.tga")
        for section in ("base_color", "normal", "roughness"):
            cfg = head[section]
            self.assert_clean_reference(str(cfg.get("semantic_source", "")))
            self.assert_clean_reference(str(cfg.get("source_path", "")))

    def test_default_variant_has_no_damage_overlays(self) -> None:
        self.assertEqual(self.profile["damage_overlays"], [])
        variant = next(v for v in self.variants["variants"] if v["id"] == "abby_wlf_default_709")
        self.assertEqual(variant.get("damage_overlays"), [])
        self.assertEqual(
            variant.get("appearance_profile"),
            "models/characters/abby/appearance/abby_wlf_default.json",
        )

    def test_head_material_slots_resolve_only_canonical_ytd_entries(self) -> None:
        material = MATERIAL.read_text("utf-8").lower()
        for entry in ("m00_base", "m01_base", "m00_normal", "m01_normal", "m00_roughness", "m01_roughness"):
            self.assertIn(f"textures/characters/abby.ytd@{entry}", material)
        for token in self.forbidden:
            self.assertNotIn(token, material)


if __name__ == "__main__":
    unittest.main(verbosity=2)
