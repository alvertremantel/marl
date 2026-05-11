# Documentation Restructure and Update

**Date:** 2026-05-10
**Status:** draft

---

## Goal

Restructure the project documentation so the README becomes a concise project
landing page, detailed usage instructions move to a new `docs/USAGE.md`, a new
`docs/SCRIPTS.md` documents the project's Python scripts, and `docs/INFO.md` is
updated to reflect the current workspace structure and project state.

## Understanding

The repository currently has three documentation files: `README.md` (166 lines,
mixed overview + usage + viewer docs + citation), `docs/INFO.md` (288 lines,
deep technical characterization but with stale paths and outdated claims), and
no dedicated usage or scripts documentation.

Key staleness issues in `docs/INFO.md`:

- All source paths use `src/` instead of `crates/marl-engine/src/` (from the
  workspace decomposition of 2026-04-25).
- Line 256 claims the `half` dependency is "present but unused" — it was
  removed in the Unified Runtime Configuration work (2026-04-25).
- Line 267 claims "There are no tests yet" — there are 70+ tests across
  marl-engine, marl-format, and marl-viewer-rs.
- Line 266 says "Runtime configuration is limited; grid size is compile-time" —
  needs updating to note the TOML config system.
- The reading order uses stale `src/` paths.

The `README.md` currently mixes project identity content (description,
architecture, model choices) with operational content (build/run commands, CLI
flag tables, output format reference, full viewer GUI walkthrough). The
operational content should live in `docs/USAGE.md`.

There is only one script: `scripts/check_binary_dump.py` (a binary output
validator).

## Approach

1. **Rewrite README.md** as a project landing page: keep the title, epigraph,
   description, architecture summary, model choices, status summary, and
   citation; move all usage/operational content to `docs/USAGE.md`; add clear
   pointers to the three docs/ files.

2. **Create `docs/USAGE.md`** from the material extracted from the README,
   expanded into a comprehensive guide covering building, engine usage, viewer
   usage, output formats, TOML config reference, and common workflows.

3. **Update `docs/INFO.md`** by fixing stale paths, removing the `half` claim,
   updating the tests claim, updating the runtime config note, and revising the
   reading order.

4. **Create `docs/SCRIPTS.md`** documenting `check_binary_dump.py` (its purpose,
   usage, arguments, output, and examples).

5. **Update `.agents/context/MAP.md`** line 42 to reflect the new documentation
   structure.

No code is changed. No user-visible behavior changes. These are pure
documentation edits.

## Steps

### Phase 1: Create docs/USAGE.md (first — before removing README content)

1. **Create the usage documentation file**
   - **Location:** `docs/USAGE.md` (new file)
   - **Action:** Write a comprehensive usage guide. Extract all operational
     content from the current `README.md` (do not delete it yet) and expand
     into the following sections:
   - **Location:** `README.md`
   - **Action:** Replace the entire file. Keep and lightly edit sections:
     - Title, epigraph (lines 1–7, unchanged)
     - Project description (lines 9–11, unchanged)
     - "What The Simulation Does" (lines 26–34, keep as bullet list)
     - "Architecture At A Glance" (lines 36–49, condense; replace the inline
       crate summary with a shorter "key crates" list and point to INFO.md for
       depth)
     - "Important Model Choices" (lines 51–58, keep)
     - "Known In-Progress Or Partial Areas" (lines 60–65, keep)
     - "Status Summary" (lines 156–158, keep)
     - Citation (lines 160–166, keep)
   - Remove entirely (these move to USAGE.md):
     - "Running" section (lines 68–98)
     - "Outputs" section (lines 100–119)
     - "Standalone Viewer" section (lines 121–154)
   - Add a new "Documentation" section (after Status Summary, before Citation)
     linking to `docs/USAGE.md`, `docs/INFO.md`, and `docs/SCRIPTS.md`.
   - Add a "Quick Start" section (3–4 lines, minimal commands) that points to
     USAGE.md for full detail.
   - **Verification:** Read the rewritten README and confirm it no longer
     contains CLI flag tables, build commands, viewer flag documentation, or CSV
     output descriptions. Confirm the docs/ links are present and correct.

### Phase 2: Create docs/USAGE.md

2. **Create the usage documentation file**
   - **Location:** `docs/USAGE.md` (new file)
   - **Action:** Write a comprehensive usage guide with these sections:
     1. **Prerequisites** — Rust toolchain, optional GPU drivers for viewer/GPGPU.
     2. **Building** — `cargo build --release --workspace`, feature flags
        (`gpu`), crate-specific builds.
     3. **Running the Engine** — basic `cargo run -p marl-engine` invocation
        with common flags; the full CLI flag table (migrated from README lines
        90–97); grid size note (compile-time).
     4. **Runtime Configuration (TOML)** — moved from README lines 76–99;
        sample `marl.toml` reference; how partial files fall back to defaults;
        key parameter groups (simulation physics, seeding, output). Mention
        `marl.toml` in the repo root as the reference.
     5. **Output Files** — moved from README lines 100–119; binary outputs
        (field.bin, cells.bin, run_meta.json); legacy CSV outputs and their
        toggles; PPM image outputs and their toggles; `summary.md`.
     6. **Running the Viewer** — moved from README lines 121–154; CLI flags
        table (`--tick`, `--species`, `--view`, `--cells`, `--cell-alpha`,
        `--scale`, `--exposure`, `--steps`); legacy top-down mode example;
        microbe coloring note.
     7. **Viewer GUI** — the GUI shell features (directory picker, tick
        navigation, view settings panel, Apply/Reset, Reload); how to point the
        viewer at a running simulation's output directory.
     8. **Common Workflows** — parameter sweep (copy TOML, edit one value,
        rerun), running and viewing simultaneously, validating output with
        `check_binary_dump.py`, generating images for quick inspection.
     9. **Troubleshooting** — common issues: `wgpu` adapter not found, snapshot
        file not found, stale snapshot list, `rfd` folder picker returns
        nothing, grid size mismatch when switching compile-time dimensions.
   - **Verification:** Read the file and confirm every usage detail from the
     current README is preserved here. Confirm there are no broken references.

### Phase 3: Update docs/INFO.md

3. **Fix stale source paths throughout**
   - **Location:** `docs/INFO.md`
   - **Action:** Replace all occurrences of `src/config.rs`, `src/field.rs`,
     `src/cell.rs`, `src/light.rs`, `src/main.rs`, `src/data.rs`,
     `src/snapshot.rs`, `src/hgt.rs`, `src/binary_dump.rs`, and generic `src/`
     references with the correct `crates/marl-engine/src/` paths. This includes
     lines ~13–19 (architecture bullet list), ~36 (field reference), ~184
     (starter factory mention), and ~275–282 (reading order list).
   - **Verification:** `grep "src/" docs/INFO.md` returns no results (or only
     results that are not stale, e.g., references to `src/` in non-path
     contexts). `grep "crates/marl-engine/src/" docs/INFO.md` confirms paths
     are correct.

4. **Remove stale `half` dependency claim**
   - **Location:** `docs/INFO.md`, line 256: `the \`half\` dependency is present but unused`
   - **Action:** Delete line 256 ("- the `half` dependency is present but unused").
   - **Verification:** `grep -n "half" docs/INFO.md` returns no results.

5. **Update "no tests" claim**
   - **Location:** `docs/INFO.md`, line 267: `- There are no tests yet.`
   - **Action:** Replace with: "Unit and integration tests exist for engine
     field diffusion, binary dump layout, GPU diffusion equivalence, viewer
     CLI/IO/camera/renderer/GUI, and shared format crate. Run with `cargo test
     --workspace`."
   - **Verification:** Read the updated line; confirm it accurately reflects
     the current test landscape (no false claims).

6. **Update runtime config note**
   - **Location:** `docs/INFO.md`, line 266: `- Runtime configuration is limited; grid size is compile-time.`
   - **Action:** Replace with: "Most physics, chemistry, and output parameters
     are runtime-configurable via TOML + CLI. Grid dimensions and species
     counts remain compile-time constants (they determine array sizes)."
   - **Verification:** Read the updated line; confirm it matches the current
     `config.rs` reality.

7. **Update suggested reading order paths**
   - **Location:** `docs/INFO.md`, lines 275–282 (the numbered reading list)
   - **Action:** Replace the 8 numbered items with correct crate paths:
     1. `crates/marl-engine/src/config.rs`
     2. `crates/marl-engine/src/field.rs`
     3. `crates/marl-engine/src/cell.rs`
     4. `crates/marl-engine/src/main.rs`
     5. `crates/marl-engine/src/light.rs`
     6. `crates/marl-engine/src/data.rs`
     7. `crates/marl-engine/src/snapshot.rs`
     8. `crates/marl-engine/src/hgt.rs`
   - Also consider adding `crates/marl-format/src/lib.rs` and
     `crates/marl-engine/src/binary_dump.rs` to the list (now relevant with the
     viewer data pipeline). Add them after `hgt.rs` if desired.
   - **Verification:** Confirm the reading order section has correct, existing
     paths.

8. **Revise the "Overview" opening sentence**
   - **Location:** `docs/INFO.md`, line 5
   - **Action:** The opening says "written in Rust" and "CPU prototype."
     Optionally update to mention the workspace structure and GPU prototype
     status. Change: "MARL is a 3D reaction-diffusion cellular automaton
     written in Rust. It is aimed at open-ended microbial evolution..." to
     include a note about the Cargo workspace and optional GPU diffusion path.
     This is a nice-to-have clarity improvement.
   - **Verification:** Read the updated opening paragraph.

9. **Final pass: verify all INFO.md content is current**
   - **Location:** `docs/INFO.md`
   - **Action:** Re-read the entire file and spot-check any remaining stale
     references to old architecture (the file pre-dates the workspace
     decomposition, binary dump pipeline, viewer, etc.). The "High-Level
     Architecture" section (lines 13–19) bullet list references individual
     source files — these should be updated (handled in step 3). The
     "Conceptually, the simulation loop" block (lines 21–28) is still accurate.
     - Line 35: "Each voxel stores `S_EXT = 12` external species" — check if
       `S_EXT` changed (it hasn't, per STATUS.md).
     - The section "Stale Or Aspirational Elements" (lines 252–257) — after
       removing the `half` bullet, the remaining items should still be
       accurate. Verify the "older descriptions of much larger grid sizes" and
       "older GPU-facing intent" bullets still apply (they describe pre-2026
       project notes, still relevant as context).
   - **Verification:** Manual read-back of the entire file.

### Phase 4: Create docs/SCRIPTS.md

10. **Create the scripts documentation file**
    - **Location:** `docs/SCRIPTS.md` (new file)
    - **Action:** Write documentation for all scripts under `scripts/`.
      Currently only `scripts/check_binary_dump.py` exists. Document:
      - **Script:** `scripts/check_binary_dump.py`
      - **Purpose:** Sanity-checks MARL binary viewer output files (field dump,
        cell dump, and metadata) for a given tick.
      - **Requirements:** Python 3.x (uses `argparse`, `json`, `struct`,
        `pathlib` — all stdlib).
      - **Usage:** `python scripts/check_binary_dump.py <run_dir> <tick>`
      - **Arguments:**
        - `run_dir` — path to the engine output directory (contains
          `run_meta.json` and `tick_<N>.field.bin` / `tick_<N>.cells.bin`)
        - `tick` — tick number to inspect
      - **Checks performed:**
        1. Reads `run_meta.json` and validates `field_byte_len` against actual
           file size.
        2. Reads the first `f32` from the field file and checks it is finite.
        3. Validates that cell file size is divisible by `cell_record_stride`
           from metadata.
      - **Output:** On success, prints `ok: first_f32=<value>, field_bytes=<n>,
        cell_count=<n>`. On failure, exits non-zero with an error message.
      - **Example:** `python scripts/check_binary_dump.py output/run_128x128x64 0`
    - **Verification:** Run `python scripts/check_binary_dump.py --help` and
      confirm the documented arguments match. Run the example command against a
      valid output directory and confirm the documented output format matches.

### Phase 5: Update project context

11. **Update .agents/context/MAP.md**
    - **Location:** `.agents/context/MAP.md`, line 42
    - **Action:** Change line 42 from:
      `- \`README.md\`: user-facing overview, run commands, config and output documentation`
      to:
      `- \`README.md\`: project landing page (overview, architecture, model choices, status, citation)`
      Add new lines after it:
      `- \`docs/USAGE.md\`: comprehensive usage guide (build, engine, viewer, outputs, workflows, troubleshooting)`
      `- \`docs/INFO.md\`: deep technical characterization and architecture reference`
      `- \`docs/SCRIPTS.md\`: documentation for project utility scripts`
    - **Verification:** Re-read the MAP.md Documentation section.

## Risks

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| README rewrite accidentally drops important information not captured in USAGE.md | Low | Medium | Use explicit checklist: every removed section from README must appear in USAGE.md. Cross-verify after writing. |
| INFO.md updates miss some stale `src/` paths | Medium | Low | Use `grep` verification step (step 3 verification) and manual final-pass read (step 9). |
| USAGE.md becomes too long and the user wanted even more detail | Low | Low | The user explicitly asked for a "long USAGE.md" — err on the side of thoroughness. |
| MAP.md goes stale if future documentation changes aren't tracked | Low | Low | This plan's final step updates MAP.md. Future documentation plans should do the same. |

## Verification

Overall verification strategy:

1. **Structural check:** Confirm `docs/USAGE.md` and `docs/SCRIPTS.md` exist.
   Confirm `docs/INFO.md` is modified. Confirm `README.md` is substantially
   shorter and no longer contains CLI flag details.

2. **Staleness check:** Run `grep -n "src/" docs/INFO.md` — only non-path
   matches should remain. Run `grep -n "half" docs/INFO.md` — should return
   nothing. Run `grep -n "no tests" docs/INFO.md` — should return nothing.

3. **Link check:** All cross-document links (`README.md` → docs files,
   USAGE.md → other docs) should be verified by manual reading.

4. **Command check:** Run `python scripts/check_binary_dump.py --help` to
   confirm the documented interface matches.

5. **Read-back:** A human (or LLM agent) re-reads all four documentation files
   end-to-end and confirms they are self-consistent and free of contradictions.
