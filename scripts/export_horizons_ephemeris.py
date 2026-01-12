#!/usr/bin/env python3
"""Export 2D heliocentric ephemeris tables from JPL Horizons via the official SSD API.

This generates compact binary tables suitable for fast in-game interpolation.

Decisions (documented in docs/EPHEMERIS.md):
- Time axis: seconds since J2000 (t=0 at 2000-01-01 12:00:00)
- Frame: 2D heliocentric (Sun at origin), ecliptic-of-J2000
- Coverage: forward-only from J2000 for a fixed window

Output format (.bin)
--------------------
Little-endian:
- magic          [8]  b"DEOEPH1\0"
- version        u32  = 1
- body_id        u32  (stable numeric ID shared with the Rust loader)
- step_seconds   f64
- start_t0       f64  (seconds since J2000; typically 0.0 if START_TIME is J2000)
- count          u32  (#samples)
- reserved       u32  (0)
- samples[count] of:
    - x  f64 (meters)
    - y  f64 (meters)
    - vx f64 (m/s)
    - vy f64 (m/s)

Notes:
- Horizons returns state vectors in km and km/s; we convert to meters and m/s.
- Horizons "CSV_FORMAT=YES" for VECTORS uses the column order:
    JDTDB, Calendar Date (TDB), X, Y, Z, VX, VY, VZ, ...
  (i.e. JD comes first, not the calendar string.)
- This script is intended as an offline tool (not run in-game).

Requires: Python 3.9+ (uses only stdlib).

API docs:
- https://ssd-api.jpl.nasa.gov/doc/horizons.html
"""

from __future__ import annotations

import argparse
import dataclasses
import json
import os
import re
import struct
import sys
import time
import urllib.parse
import urllib.request
from typing import Iterable, List, Tuple

MAGIC = b"DEOEPH1\0"
VERSION = 1

KM_TO_M = 1000.0

# Base URL: the Horizons API is hosted under ssd.jpl.nasa.gov (not ssd-api.jpl.nasa.gov).
HORIZONS_API_BASE = "https://ssd.jpl.nasa.gov/api/horizons.api"

# NOTE: These body IDs are *not* Horizons COMMAND values.
# They should match the Rust enum discriminants you choose in the importer.
# For now we pin them manually to stay stable across refactors.
BODY_IDS = {
    "Sun": 0,
    "Mercury": 1,
    "Venus": 2,
    "Earth": 3,
    "Mars": 4,
    "Jupiter": 5,
    "Saturn": 6,
    "Uranus": 7,
    "Neptune": 8,
    "Moon": 9,
    "Io": 10,
    "Europa": 11,
    "Ganymede": 12,
    "Callisto": 13,
    "Titan": 14,
}

# Horizons COMMAND values (major bodies).
# For satellites, Horizons uses NAIF IDs; these are standard.
HORIZONS_COMMAND = {
    "Mercury": "199",
    "Venus": "299",
    "Earth": "399",
    "Mars": "499",
    "Jupiter": "599",
    "Saturn": "699",
    "Uranus": "799",
    "Neptune": "899",
    # Major moons
    "Moon": "301",
    "Io": "501",
    "Europa": "502",
    "Ganymede": "503",
    "Callisto": "504",
    "Titan": "606",
}


@dataclasses.dataclass(frozen=True)
class ExportSpec:
    name: str
    start_time: str
    stop_time: str
    step_size: str


def _http_get_text(url: str) -> str:
    with urllib.request.urlopen(url, timeout=120) as resp:
        raw = resp.read()
    return raw.decode("utf-8", errors="replace")


def _looks_like_horizons_line_limit_error(text: str) -> bool:
    # Example error observed:
    # "Projected output length (~219147) exceeds 90024 line max -- change step-size"
    return (
        "Projected output length" in text and "exceeds" in text and "line max" in text
    )


def _build_horizons_url(
    command: str, start_time: str, stop_time: str, step_size: str
) -> str:
    # We request VECTORS with center at the Sun, in the ecliptic plane.
    # Using J2000 ecliptic plane is important for stable 2D projection.
    params = {
        "format": "text",
        "COMMAND": f"'{command}'",
        "OBJ_DATA": "'NO'",
        "MAKE_EPHEM": "'YES'",
        "EPHEM_TYPE": "'VECTORS'",
        "CENTER": "'500@10'",  # Sun center
        "START_TIME": f"'{start_time}'",
        "STOP_TIME": f"'{stop_time}'",
        "STEP_SIZE": f"'{step_size}'",
        # Output settings
        "VEC_CORR": "'NONE'",
        "VEC_LABELS": "'NO'",
        "CSV_FORMAT": "'YES'",
        "REF_PLANE": "'ECLIPTIC'",
        "REF_SYSTEM": "'J2000'",
        "OUT_UNITS": "'KM-S'",
        "VEC_DELTA_T": "'NO'",
    }

    qs = urllib.parse.urlencode(params, safe="@' ")
    return f"{HORIZONS_API_BASE}?{qs}"


def _fetch_horizons_vectors_text_chunked(
    command: str,
    start_time: str,
    stop_time: str,
    step_size: str,
    *,
    max_stop_year: int | None,
) -> tuple[str, str]:
    """
    Fetch Horizons VECTORS output, automatically chunking the requested time range
    if Horizons refuses due to the output line limit.

    We keep the step-size fixed (accuracy/cadence choice) and split the time span
    into smaller windows until Horizons returns a normal $$SOE/$$EOE block.

    If `max_stop_year` is not None, we clamp STOP_TIME's year to that value to avoid
    Horizons hard-failing on unsupported far-future spans. The effective stop time
    is returned alongside the response text so callers can reflect reality in the manifest.

    The returned string is a concatenation of $$SOE/$$EOE blocks (headers removed),
    so downstream parsing can treat it as one longer SOE/EOE section.
    """

    def parse_year(s: str) -> int:
        return int(s.split("-", 1)[0])

    def with_year(s: str, year: int) -> str:
        # Keep month/day/time as-is; only clamp the year.
        rest = s.split("-", 1)[1]
        return f"{year}-{rest}"

    def clamp_stop_time_for_horizons(start_t: str, stop_t: str) -> str:
        """
        Horizons' underlying ephemeris coverage isn't infinite. In practice, the
        default source (often DE441 for major bodies) can refuse requests beyond
        a max year for some targets.

        For our game use-case, we optionally clamp the stop year to avoid hard failure
        when asking for far-future ranges.

        If you need a higher max year later, pass `--max_stop_year` or disable clamping
        with `--max_stop_year 0`.
        """
        if max_stop_year is None:
            return stop_t

        start_year = parse_year(start_t)
        stop_year = parse_year(stop_t)

        # If caller asks for an unreasonably high stop year, clamp.
        if stop_year > max_stop_year:
            # Ensure stop stays after start.
            clamped_year = max(start_year, max_stop_year)
            return with_year(stop_t, clamped_year)

        return stop_t

    effective_stop_time = clamp_stop_time_for_horizons(start_time, stop_time)

    # Initial request
    url = _build_horizons_url(command, start_time, effective_stop_time, step_size)
    text = _http_get_text(url)

    # If the response isn't a line-limit error, return it (success or any other error).
    if not _looks_like_horizons_line_limit_error(text):
        return text, effective_stop_time

    # If we hit the line limit, recursively bisect the time window by calendar year.
    # This keeps things simple and deterministic.
    #
    # Assumes our input dates are in the fixed form used by this script:
    #   "YYYY-MM-DD HH:MM"
    # and the span is within a few centuries.
    start_year = parse_year(start_time)
    stop_year = parse_year(effective_stop_time)

    if stop_year <= start_year:
        # Can't split further; surface the original error.
        return text

    mid_year = (start_year + stop_year) // 2
    mid_time = f"{mid_year}-01-01 12:00"

    left, left_stop = _fetch_horizons_vectors_text_chunked(
        command, start_time, mid_time, step_size, max_stop_year=max_stop_year
    )
    right, right_stop = _fetch_horizons_vectors_text_chunked(
        command, mid_time, effective_stop_time, step_size, max_stop_year=max_stop_year
    )

    # Merge by splicing SOE/EOE blocks together.
    # If a side still returns an error without $$SOE/$$EOE, downstream extraction
    # will fail with a clear message (including the response body).
    def extract_block_or_passthrough(t: str) -> str:
        i = t.find("$$SOE")
        j = t.find("$$EOE")
        if i == -1 or j == -1 or j <= i:
            return t  # let caller error out with context
        return "$$SOE\n" + t[i + len("$$SOE") : j].lstrip("\n") + "\n$$EOE\n"

    # Concatenate the two blocks; headers are irrelevant to parsing.
    # Effective stop time should match the right segment's stop (which is derived from the original
    # effective stop time), but return it explicitly for correctness.
    stitched = extract_block_or_passthrough(left) + extract_block_or_passthrough(right)
    return stitched, right_stop


_CSV_LINE_RE = re.compile(
    # Example (CSV) actual Horizons VECTORS output begins with JD:
    # 2451545.000000000, A.D. 2000-Jan-01 12:00:00.0000, X, Y, Z, VX, VY, VZ, ...
    #
    # Horizons output is not strictly machine-CSV; it often includes:
    # - spaces after commas
    # - extra trailing columns (LT, RG, RR, etc.) and a trailing comma
    #
    # Make the parser robust: allow whitespace around separators and ignore trailing columns.
    r"^\s*"
    r"(?P<jd>[-0-9.]+)\s*,\s*"
    r"(?P<cal>[^,]+?)\s*,\s*"
    r"(?P<x>[-0-9.Ee+]+)\s*,\s*"
    r"(?P<y>[-0-9.Ee+]+)\s*,\s*"
    r"(?P<z>[-0-9.Ee+]+)\s*,\s*"
    r"(?P<vx>[-0-9.Ee+]+)\s*,\s*"
    r"(?P<vy>[-0-9.Ee+]+)\s*,\s*"
    r"(?P<vz>[-0-9.Ee+]+)"
    r"(?:\s*,.*)?\s*$"
)


def _extract_csv_block(text: str) -> List[str]:
    start = text.find("$$SOE")
    end = text.find("$$EOE")
    if start == -1 or end == -1 or end <= start:
        # Provide actionable diagnostics: Horizons often returns plain-text errors.
        preview = text.strip()
        if len(preview) > 500:
            preview = preview[:500] + "\n...(truncated)..."
        raise RuntimeError(
            "Horizons response missing $$SOE/$$EOE block.\n"
            "This usually means Horizons returned an error message instead of ephemeris data.\n"
            f"Response preview:\n{preview}\n"
        )
    block = text[start + len("$$SOE") : end]
    # Horizons often includes blank lines.
    return [ln.strip() for ln in block.splitlines() if ln.strip()]


def _parse_samples(
    lines: Iterable[str],
) -> List[Tuple[float, float, float, float, float]]:
    samples: List[Tuple[float, float, float, float, float]] = []
    for ln in lines:
        m = _CSV_LINE_RE.match(ln)
        if not m:
            # Skip non-data lines defensively.
            continue
        jd = float(m.group("jd"))
        x_km = float(m.group("x"))
        y_km = float(m.group("y"))
        vx_km_s = float(m.group("vx"))
        vy_km_s = float(m.group("vy"))
        samples.append(
            (jd, x_km * KM_TO_M, y_km * KM_TO_M, vx_km_s * KM_TO_M, vy_km_s * KM_TO_M)
        )

    if not samples:
        raise RuntimeError("No vector samples parsed (check Horizons settings / regex)")
    return samples


def _jd_to_j2000_seconds(jd: float) -> float:
    # J2000 epoch in Julian Date (TT) is 2451545.0
    # We treat this as our time origin and ignore TT/UTC offsets.
    return (jd - 2451545.0) * 86400.0


def _write_bin(
    path: str,
    body_id: int,
    step_seconds: float,
    samples: List[Tuple[float, float, float, float, float]],
) -> None:
    # samples: (jd, x, y, vx, vy) but we store only x,y,vx,vy
    if len(samples) < 2:
        raise RuntimeError("Need at least 2 samples for interpolation")

    # Derive start_t0 from the first JD.
    start_t0 = _jd_to_j2000_seconds(samples[0][0])

    with open(path, "wb") as f:
        f.write(MAGIC)
        f.write(struct.pack("<I", VERSION))
        f.write(struct.pack("<I", body_id))
        f.write(struct.pack("<d", step_seconds))
        f.write(struct.pack("<d", start_t0))
        f.write(struct.pack("<I", len(samples)))
        f.write(struct.pack("<I", 0))

        for _, x, y, vx, vy in samples:
            f.write(struct.pack("<dddd", x, y, vx, vy))


def _infer_step_seconds(
    samples: List[Tuple[float, float, float, float, float]],
) -> float:
    # Use first gap.
    dt_days = samples[1][0] - samples[0][0]
    return dt_days * 86400.0


def export_one(spec: ExportSpec, out_dir: str, *, max_stop_year: int | None) -> dict:
    if spec.name == "Sun":
        raise ValueError("Sun is not exported (it is always the origin)")

    command = HORIZONS_COMMAND.get(spec.name)
    if not command:
        raise ValueError(f"No Horizons COMMAND configured for {spec.name}")

    # Ensure output directory exists so file writes don't fail mid-export.
    os.makedirs(out_dir, exist_ok=True)

    url = _build_horizons_url(command, spec.start_time, spec.stop_time, spec.step_size)
    print(f"Fetching {spec.name}: {url}")

    # Large spans (e.g. 600 years @ 1d cadence) can exceed Horizons line limits.
    # Auto-chunk by calendar year to keep step-size stable.
    text, effective_stop_time = _fetch_horizons_vectors_text_chunked(
        command=command,
        start_time=spec.start_time,
        stop_time=spec.stop_time,
        step_size=spec.step_size,
        max_stop_year=max_stop_year,
    )

    # If chunking returned concatenated blocks, just parse each SOE/EOE block and merge.
    if text.count("$$SOE") > 1:
        all_samples: List[Tuple[float, float, float, float, float]] = []
        parts = text.split("$$SOE")
        for part in parts:
            if "$$EOE" not in part:
                continue
            chunk_text = "$$SOE" + part
            lines = _extract_csv_block(chunk_text)
            all_samples.extend(_parse_samples(lines))
        samples = all_samples
    else:
        lines = _extract_csv_block(text)
        samples = _parse_samples(lines)

    step_seconds = _infer_step_seconds(samples)

    out_path = f"{out_dir}/{spec.name.lower()}.bin"
    body_id = BODY_IDS[spec.name]
    _write_bin(out_path, body_id, step_seconds, samples)

    start_t0 = _jd_to_j2000_seconds(samples[0][0])
    stop_t = _jd_to_j2000_seconds(samples[-1][0])
    return {
        "name": spec.name,
        "body_id": body_id,
        "path": out_path,
        "start_time": spec.start_time,
        "requested_stop_time": spec.stop_time,
        "effective_stop_time": effective_stop_time,
        "start_t0": start_t0,
        "stop_t": stop_t,
        "step_seconds": step_seconds,
        "count": len(samples),
    }


def main(argv: List[str]) -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--out", default="assets/ephemeris")
    p.add_argument("--years", type=int, default=600)
    p.add_argument("--planet_step", default="1 d")
    p.add_argument("--moon_step", default="2 h")
    p.add_argument(
        "--max_stop_year",
        type=int,
        default=2500,
        help=(
            "Optional clamp on STOP_TIME year to avoid Horizons hard-failing on far-future ranges. "
            "Set to 0 to disable clamping entirely."
        ),
    )
    args = p.parse_args(argv)

    # Ensure output directory exists before any per-body export begins.
    os.makedirs(args.out, exist_ok=True)

    # Coverage: [J2000, J2000 + years] (requested).
    start_time = "2000-01-01 12:00"
    requested_stop_time = f"{2000 + args.years}-01-01 12:00"

    max_stop_year = None if args.max_stop_year == 0 else args.max_stop_year

    planet_names = [
        "Mercury",
        "Venus",
        "Earth",
        "Mars",
        "Jupiter",
        "Saturn",
        "Uranus",
        "Neptune",
    ]
    moon_names = ["Moon", "Io", "Europa", "Ganymede", "Callisto", "Titan"]

    specs: List[ExportSpec] = []
    for name in planet_names:
        specs.append(
            ExportSpec(
                name=name,
                start_time=start_time,
                stop_time=requested_stop_time,
                step_size=args.planet_step,
            )
        )
    for name in moon_names:
        specs.append(
            ExportSpec(
                name=name,
                start_time=start_time,
                stop_time=requested_stop_time,
                step_size=args.moon_step,
            )
        )

    manifest = {
        "generated_at_unix": int(time.time()),
        "start_time": start_time,
        "requested_stop_time": requested_stop_time,
        "years_requested": args.years,
        "max_stop_year": args.max_stop_year,
        "planet_step": args.planet_step,
        "moon_step": args.moon_step,
        "files": [],
        "coverage_summary": {
            "min_start_t0": None,
            "max_start_t0": None,
            "min_stop_t": None,
            "max_stop_t": None,
            "effective_stop_time_min": None,
            "effective_stop_time_max": None,
        },
    }

    for spec in specs:
        entry = export_one(spec, args.out, max_stop_year=max_stop_year)
        manifest["files"].append(entry)

        cov = manifest["coverage_summary"]

        cov["min_start_t0"] = (
            entry["start_t0"]
            if cov["min_start_t0"] is None
            else min(cov["min_start_t0"], entry["start_t0"])
        )
        cov["max_start_t0"] = (
            entry["start_t0"]
            if cov["max_start_t0"] is None
            else max(cov["max_start_t0"], entry["start_t0"])
        )
        cov["min_stop_t"] = (
            entry["stop_t"]
            if cov["min_stop_t"] is None
            else min(cov["min_stop_t"], entry["stop_t"])
        )
        cov["max_stop_t"] = (
            entry["stop_t"]
            if cov["max_stop_t"] is None
            else max(cov["max_stop_t"], entry["stop_t"])
        )

        cov["effective_stop_time_min"] = (
            entry["effective_stop_time"]
            if cov["effective_stop_time_min"] is None
            else min(cov["effective_stop_time_min"], entry["effective_stop_time"])
        )
        cov["effective_stop_time_max"] = (
            entry["effective_stop_time"]
            if cov["effective_stop_time_max"] is None
            else max(cov["effective_stop_time_max"], entry["effective_stop_time"])
        )

    manifest_path = f"{args.out}/manifest.json"
    with open(manifest_path, "w", encoding="utf-8") as f:
        json.dump(manifest, f, indent=2)

    print(f"Wrote manifest: {manifest_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
