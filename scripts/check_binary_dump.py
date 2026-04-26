#!/usr/bin/env python3
"""Sanity-check MARL binary viewer output files."""

from __future__ import annotations

import argparse
import json
import math
import struct
from pathlib import Path


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("run_dir", type=Path, help="Output run directory")
    parser.add_argument("tick", type=int, help="Tick number to inspect")
    args = parser.parse_args()

    meta_path = args.run_dir / "run_meta.json"
    field_path = args.run_dir / f"tick_{args.tick}.field.bin"
    cells_path = args.run_dir / f"tick_{args.tick}.cells.bin"

    meta = json.loads(meta_path.read_text(encoding="utf-8"))
    expected_field_bytes = int(meta["field_byte_len"])
    field_bytes = field_path.stat().st_size
    if field_bytes != expected_field_bytes:
        raise SystemExit(
            f"field size mismatch: got {field_bytes}, expected {expected_field_bytes}"
        )

    with field_path.open("rb") as handle:
        first = struct.unpack("<f", handle.read(4))[0]
    if not math.isfinite(first):
        raise SystemExit(f"first field value is not finite: {first!r}")

    stride = int(meta["cell_record_stride"])
    cell_bytes = cells_path.stat().st_size
    if cell_bytes % stride != 0:
        raise SystemExit(f"cell file size {cell_bytes} is not divisible by stride {stride}")

    print(
        f"ok: first_f32={first:.6g}, field_bytes={field_bytes}, "
        f"cell_count={cell_bytes // stride}"
    )


if __name__ == "__main__":
    main()
