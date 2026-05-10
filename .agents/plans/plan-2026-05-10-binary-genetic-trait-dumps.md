# Binary Genetic and Trait Dumps

**Date:** 2026-05-10
**Status:** draft

---

## Goal

Add optional binary outputs that expose per-cell trait summaries and exact genetic/ruleset state alongside the existing binary field and viewer cell dumps. Preserve the current `tick_<T>.field.bin` and `tick_<T>.cells.bin` formats so the existing viewer and scripts continue to work, and document the per-snapshot storage cost clearly enough for users to choose an appropriate output mode.

## Understanding

- Existing binary output is implemented in `crates/marl-engine/src/binary_dump.rs`:
  - `write_field_dump()` writes `tick_<T>.field.bin` as raw little-endian `f32` values in `[z][y][x][species]` order.
  - `write_cell_dump()` writes `tick_<T>.cells.bin` as a headerless array of 25-byte `marl_format::ViewerCellRecord` records: `pos:f32[3],lineage_id:u64,starter_type:u8,energy:f32`.
  - `write_run_meta()` manually writes `run_meta.json` with field/cell layout metadata.
- Shared format definitions live in `crates/marl-format/src/lib.rs`; `RunMeta` is loaded by the viewer in `crates/marl-viewer-rs/src/io.rs`. Any new `RunMeta` fields must use serde defaults or `Option` so old `run_meta.json` files remain readable.
- The main snapshot loop in `crates/marl-engine/src/main.rs:289-307` writes field/cell binary files when their toggles are enabled. Legacy CSV cell/reaction dumps are only written when `OutputConfig::write_csv_snapshots` is true (`main.rs:309-329`).
- Evolvable cell data is in `crates/marl-engine/src/cell.rs`:
  - `CellState`: `pos`, `lineage_id`, `age`, `internal: [f32; M_INT]`, `ruleset`, `quiescent`, `starter_type`, `prep_remaining`.
  - `Ruleset`: `receptors`, `transport`, `reactions`, `effectors`, `fate`, `hgt_propensity`, `mutation_rate`.
  - Current compile-time sizes in `crates/marl-engine/src/config.rs`: `M_INT=16`, `R_MAX=16`, `S_RECEPTORS=8`, `S_TRANSPORTERS=8`, `S_EFFECTORS=8`.
- Current default field dump size is fixed at `128 * 128 * 64 * 12 * 4 = 50,331,648` bytes, as noted in `.agents/context/NOTES.md`. Current cell dump size is `25 * live_cell_count` bytes.
- Existing viewer rendering only needs the 25-byte cell records. Changing `CELL_RECORD_STRIDE` or the `tick_<T>.cells.bin` layout would break `marl-viewer-rs`, so additional data must be emitted as sidecar files.

## Approach

Implement two independent sidecar dump types:

1. **Trait summary dump**: `tick_<T>.traits.bin`, enabled by default via `write_binary_traits = true`. This is compact enough for routine analysis and includes current cell state/phenotype: position, lineage, deterministic genotype hash, age, flags, active counts, full internal concentrations, fate thresholds, HGT propensity, and mutation rate.
2. **Full genome dump**: `tick_<T>.genomes.bin`, disabled by default via `write_binary_genomes = false`. This stores the exact serialized `Ruleset` for every living cell and is intended for selected runs or coarser snapshot intervals because it can dominate storage at high occupancy.

Do not alter `tick_<T>.cells.bin`; keep it as the stable viewer contract. Add metadata fields that describe the new sidecars, and make all new metadata backward-compatible for old runs.

### Proposed binary layouts and sizes

All integers and floats are little-endian. Serialize manually with `to_le_bytes()` rather than transmuting packed structs; this avoids unaligned references and allows stride helpers to depend on compile-time species/reaction counts.

`tick_<T>.traits.bin` record layout:

| Field | Type | Bytes |
|---|---:|---:|
| `pos` | `u16[3]` | 6 |
| `lineage_id` | `u64` | 8 |
| `genotype_hash` | `u64` | 8 |
| `age` | `u32` | 4 |
| `flags` | `u8` (`bit0 = quiescent`; other bits 0) | 1 |
| `starter_type` | `u8` | 1 |
| `prep_remaining` | `u16` | 2 |
| `active_reactions` | `u8` | 1 |
| `active_transporters` | `u8` | 1 |
| `active_effectors` | `u8` | 1 |
| `reserved` | `u8` (`0`) | 1 |
| `internal` | `f32[M_INT]` | `4 * M_INT` |
| `fate` | `f32[4]` (`division_energy`, `death_energy`, `quiescence_energy`, `division_prep_ticks`) | 16 |
| `hgt_propensity` | `f32` | 4 |
| `mutation_rate` | `f32` | 4 |

Formula: `trait_record_stride = 58 + 4 * M_INT`. With current `M_INT=16`, this is `122` bytes per living cell.

`tick_<T>.genomes.bin` record layout:

| Field | Type | Bytes |
|---|---:|---:|
| `pos` | `u16[3]` | 6 |
| `lineage_id` | `u64` | 8 |
| `genotype_hash` | `u64` | 8 |
| `receptors` | repeated `k_half:f32,n_hill:f32,gain:f32` | `12 * S_RECEPTORS` |
| `transport` | repeated `uptake_rate:f32,secrete_rate:f32,ext_species:u8,int_species:u8` | `10 * S_TRANSPORTERS` |
| `reactions` | repeated `substrate:u8,product:u8,catalyst:u8,cofactor:u8,k_m:f32,v_max:f32,k_cat:f32` | `16 * R_MAX` |
| `effectors` | repeated `threshold:f32,rate:f32,int_species:u8,ext_species:u8` | `10 * S_EFFECTORS` |
| `fate` | `f32[4]` | 16 |
| `hgt_propensity` | `f32` | 4 |
| `mutation_rate` | `f32` | 4 |

Formula: `genome_record_stride = 46 + 12*S_RECEPTORS + 10*S_TRANSPORTERS + 16*R_MAX + 10*S_EFFECTORS`. With current constants, this is `558` bytes per living cell.

### Storage impact per dump

Let `N = live_cell_count` at the snapshot tick.

- Current dump: `50,331,648 + 25N` bytes.
- With trait sidecar only: `50,331,648 + (25 + 122)N` bytes.
- With genome sidecar only: `50,331,648 + (25 + 558)N` bytes.
- With both sidecars: `50,331,648 + (25 + 122 + 558)N` bytes.

Approximate decimal MB totals for the default grid:

| Live cells | Occupancy | Current | + traits | + genomes | + both |
|---:|---:|---:|---:|---:|---:|
| 90 | initial default seed | 50.334 MB | 50.345 MB | 50.384 MB | 50.395 MB |
| 10,486 | 1% | 50.594 MB | 51.873 MB | 56.445 MB | 57.724 MB |
| 104,858 | 10% | 52.953 MB | 65.746 MB | 111.464 MB | 124.257 MB |
| 1,048,576 | 100% | 76.546 MB | 204.472 MB | 661.651 MB | 789.578 MB |

Recommended defaults: keep `write_binary_field = true` and `write_binary_cells = true`; set `write_binary_traits = true` for routine trait analysis; set `write_binary_genomes = false` because full genomes can more than double a dump around 10% occupancy and can add ~585 MB at full occupancy.

### Ordering and parallelization

- Phase 1 must land before engine writers because it defines the metadata contract and stride helpers.
- Phase 2 metadata/config and Phase 3 engine serialization both depend on Phase 1, but can be implemented by separate agents after agreeing on the exact helper/function names listed below.
- Phase 5 documentation and checker updates can proceed in parallel after the metadata field names and file names are fixed; they should not modify engine serialization code.
- Viewer code should only need test updates caused by `RunMeta::new(...)` signature changes; do not add rendering dependencies on trait/genome sidecars in this plan.

## Steps

### Phase 1: Extend shared format metadata without breaking old dumps

1. **Add sidecar schema helpers**
   - **Location:** `crates/marl-format/src/lib.rs`
   - **Action:** Add constants for file patterns and layout strings:
     - `TRAIT_FILE_PATTERN = "tick_<T>.traits.bin"`
     - `GENOME_FILE_PATTERN = "tick_<T>.genomes.bin"`
     - `TRAIT_LAYOUT = "pos:u16[3],lineage_id:u64,genotype_hash:u64,age:u32,flags:u8,starter_type:u8,prep_remaining:u16,active_reactions:u8,active_transporters:u8,active_effectors:u8,reserved:u8,internal:f32[m_int],fate:f32[4],hgt_propensity:f32,mutation_rate:f32"`
     - `GENOME_LAYOUT = "pos:u16[3],lineage_id:u64,genotype_hash:u64,receptors:{k_half:f32,n_hill:f32,gain:f32}[s_receptors],transport:{uptake_rate:f32,secrete_rate:f32,ext_species:u8,int_species:u8}[s_transporters],reactions:{substrate:u8,product:u8,catalyst:u8,cofactor:u8,k_m:f32,v_max:f32,k_cat:f32}[r_max],effectors:{threshold:f32,rate:f32,int_species:u8,ext_species:u8}[s_effectors],fate:f32[4],hgt_propensity:f32,mutation_rate:f32"`
     - Add checked helper functions:
       - `trait_record_stride(m_int: u32) -> Option<u32>` returning `58 + 4*m_int`.
       - `genome_record_stride(s_receptors: u32, s_transporters: u32, r_max: u32, s_effectors: u32) -> Option<u32>` returning `46 + 12*s_receptors + 10*s_transporters + 16*r_max + 10*s_effectors`.
   - **Verification:** Add `marl-format` unit tests asserting `trait_record_stride(16) == Some(122)`, `genome_record_stride(8, 8, 16, 8) == Some(558)`, zero/overflow cases return `None`, and layout strings contain the expected file/field names. Run `cargo test -p marl-format`.

2. **Add backward-compatible fields to `RunMeta`**
   - **Location:** `crates/marl-format/src/lib.rs:71` (`RunMeta`) and `RunMeta::new()` / `RunMeta::validate()`.
   - **Action:** Add serde-defaulted fields:
     - compile-time counts: `r_max`, `s_receptors`, `s_transporters`, `s_effectors` (`u32`, default `0` for old metadata).
     - toggles: `write_binary_traits`, `write_binary_genomes` (`bool`, default `false`).
     - strides: `trait_record_stride`, `genome_record_stride` (`u32`, default `0`).
   - Update `RunMeta::new(...)` to accept the new counts and toggles, compute strides through the new helpers, and preserve existing field names.
   - Update `RunMeta::validate()` so old metadata remains valid when the new fields are absent/zero. Only enforce trait/genome stride equality when the corresponding `write_binary_*` flag is true or the stride value is nonzero.
   - **Verification:** Extend existing serde tests so:
     - new `RunMeta::new(...)` emits expected new fields and validates;
     - old JSON without new fields deserializes and validates;
     - an incorrect nonzero trait/genome stride fails validation. Run `cargo test -p marl-format` and `cargo test -p marl-viewer-rs` to confirm viewer metadata loading remains compatible.

### Phase 2: Add output configuration and metadata writing in the engine

1. **Add output toggles**
   - **Location:** `crates/marl-engine/src/config.rs:168` (`OutputConfig`) and `impl Default for OutputConfig`; `marl.toml`; `README.md` output/config sections.
   - **Action:** Add `pub write_binary_traits: bool` and `pub write_binary_genomes: bool` to `OutputConfig`. Set defaults to `true` for traits and `false` for genomes. Update `marl.toml` with:
     - `write_binary_traits = true`
     - `write_binary_genomes = false`
     - comments explaining storage costs and recommending genomes only for selected/coarser snapshots.
   - **Verification:** Add or update config tests if present; otherwise run `cargo check -p marl-engine` and a parse smoke test using the sample `marl.toml` via `cargo run -p marl-engine -- --config marl.toml --ticks 1 --snapshot 1 --stats 1 --output /tmp/opencode/marl_config_parse_smoke`.

2. **Write new metadata fields**
   - **Location:** `crates/marl-engine/src/binary_dump.rs:50` (`write_run_meta`).
   - **Action:** Update `write_run_meta()` to include:
     - `r_max`, `s_receptors`, `s_transporters`, `s_effectors`.
     - `write_binary_traits`, `write_binary_genomes`.
     - `trait_file_pattern`, `genome_file_pattern`.
     - `trait_record_stride`, `genome_record_stride`.
     - `trait_record_layout`, `genome_record_layout`.
   - Use `marl_format::trait_record_stride()` and `marl_format::genome_record_stride()` rather than duplicating formulas.
   - **Verification:** Add a unit test that writes metadata to a temporary directory under `/tmp/opencode`, parses it as JSON, verifies the new values and that `marl_format::RunMeta` deserializes/validates it. Run `cargo test -p marl-engine binary_dump`.

### Phase 3: Implement deterministic trait/genome serialization

1. **Add byte-writing primitives and deterministic genotype hashing**
   - **Location:** `crates/marl-engine/src/binary_dump.rs`
   - **Action:** Add small private helpers `write_u8`, `write_u16_le`, `write_u32_le`, `write_u64_le`, `write_f32_le`, plus helpers that append the same bytes to a `Vec<u8>` for hashing.
   - Implement `genotype_hash(ruleset: &Ruleset) -> u64` using FNV-1a 64-bit over the exact serialized ruleset payload bytes in genome layout order. Do not use `DefaultHasher`, because its output is not a stable file-format contract.
   - **Verification:** Add tests proving identical starter rulesets produce identical hashes, a one-field mutation changes the hash, and repeated calls in the same process return the same hash.

2. **Serialize trait records**
   - **Location:** `crates/marl-engine/src/binary_dump.rs`
   - **Action:** Add `pub fn write_trait_dump(cells: &[CellState], tick: u64, out_dir: &str, sim: &SimulationConfig) -> std::io::Result<()>` that creates `tick_<T>.traits.bin` and writes records in the same `cells` slice order as `write_cell_dump()`.
   - Active count rules:
     - `active_reactions`: reactions where `abs(v_max) > sim.active_reaction_threshold` and `substrate != product`.
     - `active_transporters`: transporters where `abs(uptake_rate) > threshold || abs(secrete_rate) > threshold`.
     - `active_effectors`: effectors where `abs(rate) > threshold`.
   - Use `u8::try_from(count).unwrap()` only because counts are bounded by current compile-time constants; if constants grow beyond `u8::MAX`, replace with a checked error.
   - **Verification:** Unit-test a synthetic `CellState` or starter metabolism cell to assert exact file length `122 * N`, exact first bytes for `pos`, `lineage_id`, flags, and finite little-endian `internal[0]`; run `cargo test -p marl-engine binary_dump`.

3. **Serialize full genome records**
   - **Location:** `crates/marl-engine/src/binary_dump.rs`
   - **Action:** Add `pub fn write_genome_dump(cells: &[CellState], tick: u64, out_dir: &str) -> std::io::Result<()>` that creates `tick_<T>.genomes.bin` and writes full ruleset records in the layout defined above. Reuse the exact same ruleset serialization bytes for `genotype_hash()` and for the on-disk genome payload so the hash is reproducible by external tools.
   - **Verification:** Unit-test exact file length `558 * N`, verify known offsets in the first record (`pos`, `lineage_id`, `genotype_hash`, first receptor `k_half`, first reaction topology), and verify the hash embedded in the trait and genome records matches for the same cell.

### Phase 4: Wire sidecar dumps into the snapshot loop

1. **Include new toggles in metadata creation**
   - **Location:** `crates/marl-engine/src/main.rs:35-38`
   - **Action:** Change `writes_binary` to include `cfg.output.write_binary_traits || cfg.output.write_binary_genomes` so `run_meta.json` is written even when only trait/genome sidecars are enabled.
   - **Verification:** Run a short config with field/cells disabled and traits enabled; verify `run_meta.json` and `tick_0.traits.bin` are created.

2. **Write sidecars at snapshot ticks**
   - **Location:** `crates/marl-engine/src/main.rs:289-307`
   - **Action:** After `write_cell_dump()`, add guarded calls:
     - `if cfg.output.write_binary_traits { binary_dump::write_trait_dump(&cells, t, &cfg.output.output_dir, sim) }`
     - `if cfg.output.write_binary_genomes { binary_dump::write_genome_dump(&cells, t, &cfg.output.output_dir) }`
   - Log warnings consistent with the existing field/cell warning style.
   - **Verification:** Run `cargo run -p marl-engine -- --ticks 1 --snapshot 1 --stats 1 --output /tmp/opencode/marl_trait_smoke` and confirm `run_meta.json`, field/cell files, and `tick_0.traits.bin` exist while `tick_0.genomes.bin` does not by default. Then run with a temporary TOML enabling `write_binary_genomes = true` and verify `tick_0.genomes.bin` exists.

### Phase 5: Update validation tooling and docs

1. **Enhance the binary dump checker**
   - **Location:** `scripts/check_binary_dump.py`
   - **Action:** Teach the script to read optional `write_binary_traits`, `write_binary_genomes`, `trait_record_stride`, and `genome_record_stride` metadata. If the toggles are true, require corresponding files and verify file size divisibility by stride. Add optional CLI flags `--require-traits` and `--require-genomes` to force validation even if metadata says false.
   - **Verification:** Run the checker against a default trait-enabled smoke run and a genome-enabled smoke run:
     - `python scripts/check_binary_dump.py /tmp/opencode/marl_trait_smoke 0 --require-traits`
     - `python scripts/check_binary_dump.py /tmp/opencode/marl_genome_smoke 0 --require-traits --require-genomes`

2. **Document formats and storage costs**
   - **Location:** `README.md`; optionally `INFO.md` if it has a binary-output section.
   - **Action:** Document the two new sidecar files, toggles, record sizes/formulas, and the storage table from this plan. Emphasize that `tick_<T>.cells.bin` remains the viewer-focused 25-byte record and that full genome dumps are disabled by default because of population-dependent storage.
   - **Verification:** Run `cargo test --workspace` to ensure doc changes did not affect code; manually review README output section for consistency with `run_meta.json` field names.

3. **Update durable project context after implementation**
   - **Location:** `.agents/context/STATUS.md` and `.agents/context/NOTES.md`
   - **Action:** After code and docs are implemented and verified, add a concise status entry describing trait/genome binary sidecars, default toggles, record strides, and storage implications. Add a durable note that existing `cells.bin` remains stable and sidecars are the extension mechanism.
   - **Verification:** Confirm no local runtime outputs under `output/` or `/tmp/opencode` are committed accidentally; run `git status --short` before handing off.

## Risks

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Breaking existing viewer/cell dump consumers | Medium | High | Do not change `tick_<T>.cells.bin` or `CELL_RECORD_STRIDE`; add sidecar files only; keep `RunMeta` new fields serde-defaulted. |
| Full genome files dominate storage on dense runs | High | Medium/High | Keep `write_binary_genomes = false` by default; document per-dump formulas and table; recommend coarser snapshot intervals for genome output. |
| Metadata drift from actual byte layout | Medium | High | Centralize stride formulas in `marl-format`; unit-test exact record lengths and known offsets in engine writer tests. |
| Unstable genotype hashes across compiler/platform versions | Medium | Medium | Hash only manually serialized little-endian field bytes with a specified FNV-1a 64-bit implementation; do not hash Rust structs directly. |
| Future changes to `M_INT`, `R_MAX`, or slot counts invalidate hardcoded sizes | Medium | Medium | Use stride helper formulas parameterized by compile-time counts and write those counts into `run_meta.json`; avoid fixed-size packed structs for new sidecars. |
| Large files slow snapshots and perturb simulation timing | Medium | Medium | Use `BufWriter`; keep genome output opt-in; consider future unique-genome dictionary/dedup only if profiling shows sidecar writes dominate runtime. |

## Verification

Complete implementation is verified when all of the following pass:

1. `cargo fmt --all`
2. `cargo check --workspace`
3. `cargo test -p marl-format`
4. `cargo test -p marl-engine`
5. `cargo test -p marl-viewer-rs`
6. Default smoke run creates field, cells, traits, and metadata but no genomes:
   - `cargo run -p marl-engine -- --ticks 1 --snapshot 1 --stats 1 --output /tmp/opencode/marl_trait_smoke`
   - `python scripts/check_binary_dump.py /tmp/opencode/marl_trait_smoke 0 --require-traits`
7. Genome-enabled smoke run creates and validates genome sidecar:
   - Use a temporary TOML under `/tmp/opencode` with `[output] write_binary_traits = true` and `write_binary_genomes = true` plus short run settings.
   - `cargo run -p marl-engine -- --config /tmp/opencode/<temp-config>.toml`
   - `python scripts/check_binary_dump.py /tmp/opencode/marl_genome_smoke 0 --require-traits --require-genomes`
8. Confirm storage math on smoke output: `traits_size == 122 * cell_count`, `genomes_size == 558 * cell_count`, and `cells_size == 25 * cell_count` for current constants.
