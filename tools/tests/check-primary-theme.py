#!/usr/bin/env python3
"""Build-free primary contrast and generated color checks (doc 04 §7–§8).

Checks authored color roles against the literal adapters emitted by tokens_cmd.
This is intentionally narrower than `cargo xtask check` / full token freshness:
it does not run the typed generator or validate non-color adapter families.
"""

import json
from pathlib import Path
import re
import tomllib

ROOT = Path(__file__).resolve().parents[2]
SCHEMES = {
    "light": "LIGHT",
    "dark": "DARK",
    "hc-light": "HIGH_CONTRAST_LIGHT",
    "hc-dark": "HIGH_CONTRAST_DARK",
}
KOTLIN = "apps/mobile/shared/src/commonMain/kotlin/com/loveoverflow/tabula/mobile/design/TabulaTokens.kt"


def require(condition, message):
    if not condition:
        raise ValueError(message)


def camel(name):
    head, *tail = name.split("-")
    return head + "".join(part.title() for part in tail)


def rgb(value):
    require(re.fullmatch(r"#[0-9A-Fa-f]{6}", value), f"invalid color: {value}")
    return tuple(int(value[i:i + 2], 16) for i in (1, 3, 5))


def luminance(color):
    channels = [value / 255 for value in color]
    linear = [v / 12.92 if v <= 0.04045 else ((v + 0.055) / 1.055) ** 2.4 for v in channels]
    return sum(v * weight for v, weight in zip(linear, (0.2126, 0.7152, 0.0722)))


def contrast(a, b):
    a, b = luminance(a), luminance(b)
    return (max(a, b) + 0.05) / (min(a, b) + 0.05)


def block(text, pattern):
    matches = list(re.finditer(pattern, text, re.S))
    require(len(matches) == 1, f"expected one adapter scheme block: {pattern}")
    return matches[0].group(1)


def check(root):
    source = tomllib.loads((root / "tokens.toml").read_text())
    export = json.loads((root / "docs/ui/tokens.json").read_text())
    require(source["meta"]["schemes"] == list(SCHEMES), "scheme order drift")
    require(source["schemes"] == export["schemes"], "JSON color scheme drift")
    require(source["ref"] == export["reference"], "JSON reference palette drift")
    rust = (root / "crates/tabula-design/src/generated.rs").read_text()
    css = (root / "apps/web/style/tokens.css").read_text()
    kotlin = (root / KOTLIN).read_text()
    mappings = 0
    for name, constant in SCHEMES.items():
        colors = source["schemes"][name]
        rs = block(rust, rf"pub const {constant}: Theme = theme\(.*?ColorTokens \{{\n(.*?)    \}},\n    GameArtTokens")
        selector = ":root" if name == "light" else f':root[data-theme="{name}"]'
        web = block(css, re.escape(selector) + r" \{\n(.*?)\n\}")
        kt = block(kotlin, rf"    val {camel(name)} = TabulaColors\(\n(.*?)    \)")
        for role, value in colors.items():
            values = value if isinstance(value, list) else [value]
            encoded = ["Color::rgb(" + ", ".join(map(str, rgb(v))) + ")" for v in values]
            rs_role = {"container": "surface_container", "container-high": "surface_container_high"}.get(role, role.replace("-", "_"))
            if isinstance(value, list):
                entries = re.findall(r"Color::rgb\([^)]*\)", block(rs, rf"        {rs_role}: \[\n(.*?)        \],"))
                require(entries == encoded, f"Rust {name}/{role} drift")
                kt_value = "listOf(" + ", ".join(f"Color(0xFF{v[1:].upper()})" for v in values) + ")"
            else:
                require(rs.count(f"        {rs_role}: {encoded[0]},\n") == 1, f"Rust {name}/{role} drift")
                kt_value = f"Color(0xFF{value[1:].upper()})"
            require(kt.count(f"        {camel(role)} = {kt_value},\n") == 1, f"Kotlin {name}/{role} drift")
            for index, color in enumerate(values, 1):
                key = f"{role}-{index}" if isinstance(value, list) else role
                require(web.count(f"  --sys-color-{key}: {color};\n") == 1, f"CSS {name}/{key} drift")
            mappings += 3 * len(values)

        primary, on_primary = rgb(colors["primary"]), rgb(colors["on-primary"])
        require(colors["selected"] == colors["primary"], f"{name}: selected family drift")
        ratios = [contrast(primary, on_primary)]
        require(ratios[0] >= (7 if name.startswith("hc-") else 4.5), f"{name}: filled label")
        for state in ("hover", "focus", "press"):
            alpha = round(source["sys"]["state"][state] * 100)
            # The same rounded sRGB on-color composition as design-crate tests.
            mixed = tuple((front * alpha + back * (100 - alpha) + 50) // 100 for front, back in zip(on_primary, primary))
            ratios.append(contrast(on_primary, mixed))
            require(ratios[-1] >= 4.5, f"{name}: {state} label")
        for surface in ("surface", "container", "container-high", "shell-canvas", "shell-paper", "shell-note"):
            require(contrast(primary, rgb(colors[surface])) >= 4.5, f"{name}: primary text/focus on {surface}")
        print(f"{name}: base/hover/focus/press " + " / ".join(f"{value:.2f}:1" for value in ratios))

    require(source["sys"]["focus"]["ring-color"] == "primary", "authored focus role drift")
    require("ring_color: color.primary," in rust, "Rust focus role drift")
    require(css.count("--sys-focus-ring-color: var(--sys-color-primary);") == 4, "CSS focus role drift")
    require('focusRingColorRole: String = "primary"' in kotlin, "Kotlin focus role drift")
    print(f"PASS: {mappings} scalar color mappings; four schemes; 16 filled state pairs; 24 primary surface pairs")


if __name__ == "__main__":
    check(ROOT)
