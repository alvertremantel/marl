# MARL Utility Scripts

This document describes utility scripts in the `scripts/` directory.

---

## `check_binary_dump.py`

**Purpose:** Sanity-checks MARL binary output files (field dump, cell dump, and
metadata) for a given tick.

**Requirements:** Python 3.6+ (standard library only — `argparse`, `json`,
`math`, `struct`, `pathlib`).

### Usage

```bash
python scripts/check_binary_dump.py <run_dir> <tick>
```

### Arguments

| Argument | Description |
|----------|-------------|
| `run_dir` | Path to the engine output directory containing `run_meta.json` and `tick_<N>.field.bin` / `tick_<N>.cells.bin` |
| `tick` | Tick number to inspect (e.g., `0`, `500`, `1000`) |

### Checks Performed

1. **Metadata:** Reads `run_meta.json` and extracts `field_byte_len` and
   `cell_record_stride`.
2. **Field file size:** Verifies that `tick_<T>.field.bin` on disk has exactly
   `field_byte_len` bytes.
3. **First field value:** Reads the first `f32` (little-endian) from the field
   file and confirms it is a finite number (not `NaN` or `inf`).
4. **Cell file integrity:** Verifies that `tick_<T>.cells.bin` size is evenly
   divisible by `cell_record_stride` (the size of one packed cell record).

### Output

On success, prints a summary line and exits with code 0:

```
ok: first_f32=<value>, field_bytes=<n>, cell_count=<n>
```

On failure, exits non-zero with an error message describing the mismatch.

### Example

```bash
$ python scripts/check_binary_dump.py output/run_128x128x64 0
ok: first_f32=0, field_bytes=50331648, cell_count=90

$ python scripts/check_binary_dump.py output/run_128x128x64 500
ok: first_f32=0.0034521, field_bytes=50331648, cell_count=127

# Error example — tick without snapshot
$ python scripts/check_binary_dump.py output/run_128x128x64 100
FileNotFoundError: [Errno 2] No such file or directory: 'output/run_128x128x64/tick_100.field.bin'
```

### Typical Use

Validate output after an engine run, or while a run is in progress to confirm
data integrity:

```bash
# Check the latest snapshot
ls -t output/run_128x128x64/tick_*.field.bin | head -1
python scripts/check_binary_dump.py output/run_128x128x64 5000
```
