# 003 — Ruleset Representation Format

**Date:** 2026-03-15
**Status:** Accepted — B+E Hybrid (parametric receptor/effector + catalytic reaction network)
**Updated:** 2026-03-15 (Iteration 002 — decision accepted, catalyst mechanism resolved, pseudocode formalized)

## Context

The ruleset is the core evolvable unit of MARL. It encodes how a cell reads external chemical concentrations and responds with secretion, consumption, internal state changes, and cell fate decisions. The choice of representation determines: expressivity of possible behaviors, computational cost of evaluation, ease of mutation and HGT, and whether the system behaves more like a game or a research instrument.

**Design principle (from director guidance):** The ruleset should feel like a natural extension of the chemical substrate, not a bolted-on controller. Chemistry-first reasoning: the ruleset should be something that could plausibly "be chemical" — a catalytic network, not a program.

## Options Considered

- **Option A — Lookup table / bitfield:** Enumerate neighborhood chemical states (discretized into bins), map each to an output action vector. O(1) evaluation, trivially mutable (flip bits or swap bin mappings). Limited expressivity — behavior is fully specified by the table, no interpolation between states. State space grows exponentially with species count and bin resolution. Best for: performance-critical, game-oriented use.

- **Option B — Parametric threshold function:** The ruleset is a small set of continuous parameters (sensitivity thresholds, saturation constants, response weights) fed into a fixed Hill-function-style decision architecture. Secretion rate of species X = f(c_external, k_sensitivity, n_hill, r_max). Mutation is Gaussian noise on parameters. Evaluation is O(species × parameters), fast and uniform across all cells — GPU-friendly. Expressivity is limited to the fixed functional form but continuous parameter space is rich. Best for: balance of performance and evolutionary dynamics.

- **Option C — Small program / instruction set:** The ruleset is a short program in a minimal custom instruction set that reads chemical concentrations and outputs an action vector. Mutation is instruction substitution/insertion/deletion. Maximum expressivity, genuine open-ended evolution possible. Evaluation cost is O(program length), heterogeneous across cells, hostile to GPU batching. Best for: research instrument, publishable evolutionary dynamics. Precedent: Avida (Lenski et al., 2003) demonstrated that instruction-set genomes produce publishable evolutionary complexity, but Avida's programs operate on abstract logic, not chemistry.

- **Option D — Small neural network (weights as genome):** Fixed architecture MLP, weights are the evolvable parameters. Mutation is weight perturbation. Continuous, differentiable, GPU-friendly in principle. Expressivity is good. Interpretability is poor — harder to reason about what a ruleset is actually doing, which matters for a research context. Precedent: Neural Cellular Automata (Mordvintsev et al., 2020) use ~8K parameters per cell — far too many for per-cell storage at MARL's scale. A compact variant (~50-100 weights) might work but reduces expressivity.

- **Option E — Catalytic reaction network (NEW, chemistry-first):** The ruleset is a fixed-length vector of reaction rules, each encoding: (input_species, output_species, catalyst_species, kinetic_parameters). The cell's intracellular chemistry IS the ruleset. Each reaction rule is an "abstract enzyme." Mutation modifies kinetic parameters or swaps species indices. HGT transfers individual reaction rules — directly paralleling real horizontal gene transfer of metabolic operons. Evaluation is O(R_max) per cell per tick, uniform if padded to fixed length. Best for: chemistry-first design, natural HGT, research instrument with interpretable dynamics.

## Literature Grounding

### Why Option E deserves serious consideration

1. **Hutton (2007)** demonstrated that genome-as-catalytic-network produces self-reproducing cells in artificial chemistry. The genome encodes enzymes; enzymes catalyze reactions; reactions sustain the cell. MARL would abstract this from particle scale to field scale.

2. **Kauffman's RAF theory** predicts that catalytic reaction networks spontaneously form self-sustaining autocatalytic sets above a critical reaction density (~1-2 catalyzed reactions per molecule). With R_max=16 reactions and M=8 species, MARL cells would be above this threshold.

3. **Gamma/HOCL chemical programming** (Banatre et al., 1986) formalizes "multiset rewriting as chemistry" — computation as reaction rules on a bag of molecules. Option E is a bounded, GPU-friendly variant of this paradigm.

4. **Fontana's AlChemy** proved that abstract chemistry (not literal biochemistry) can produce genuine self-organization. Option E preserves this abstraction while adding spatial embedding and GPU tractability.

5. **Flow-Lenia's parameter localization** (Plantec et al., 2023) — making update rules local to each entity — is conceptually parallel to Option E's per-cell reaction networks. Flow-Lenia demonstrated that localized rules enable multi-species coexistence.

### Why Options B and E are complementary

Option B describes the **receptor/transduction layer** (how cells sense external chemistry). Option E describes the **intracellular metabolism layer** (how cells process internal chemistry). These are not competing — they address different parts of the cell update:

```
External field → [Option B: receptor/transduction] → internal signals
                                                         ↓
Internal state → [Option E: catalytic network]    → updated internal state
                                                         ↓
Internal state → [Option B: effector/secretion]   → field deltas + fate decisions
```

This layered design preserves GPU-friendliness (the receptor/effector layers are fixed-topology parametric functions) while adding evolutionary richness (the intracellular network topology can evolve).

## Concrete Sizing for Option E

```
Per reaction:  ~8 bytes (2 species indices + kinetic params in float16)
Per cell:      R_max=16 reactions = 128 bytes metabolism
               + ~48 bytes receptor/transduction (Option B, 8 species)
               + ~48 bytes effector/secretion
               + ~16 bytes fate/HGT/mutation params
               = ~240 bytes total ruleset

At 100K cells: 24 MB ruleset storage (trivial vs 0.8 GB field)
At 1M cells:   240 MB (still fits in 8 GB VRAM with field)
```

## Decision

**Accepted: B+E Hybrid** (parametric receptor/effector + catalytic reaction network core).

As of Iteration 002, the B+E hybrid has been formalized with a complete pseudocode walkthrough (see [[mock-hybrid-cell-tick]]) and all three previously-open questions have been resolved:

1. **R_max=16 is sufficient.** With M=8 internal species, 16 reaction slots yield C(512, 16) ~ 10^30 possible metabolisms. Kauffman's RAF theory predicts autocatalytic loops at ~1-2 catalyzed reactions per species, well below our 16/8 = 2.0 ratio.

2. **Catalyst mechanism: concentration-dependent.** Rate = v_max x [S]/(k_m + [S]) x [C]/(k_cat + [C]). The catalyst species must be present at sufficient concentration. This creates natural dependency chains: to run a reaction, you need its catalyst, which is itself a product of another reaction. Autocatalytic loops emerge when these dependencies form cycles.

3. **Topology mutations ARE allowed.** With small probability (~0.01 per reproduction), a reaction slot can be rewired: substrate, product, or catalyst species index is randomized. This allows evolution to explore the space of reaction network topologies, not just kinetic parameters. HGT transfers complete reaction rules (including topology), paralleling real bacterial HGT of metabolic operons.

Remaining open for empirical validation:
- Whether v_max bounds need tuning for ODE stability
- Whether reaction reversibility should be added

**Resolved (Iteration 007):** Two-substrate reactions (A + B -> C) are NOT needed for v1. Single-substrate with optional cofactor is sufficient. See rationale below.

### Why Single-Substrate Reactions Are Sufficient

The question of whether MARL needs two-substrate reactions (A + B -> C, catalyzed by D) has been deferred since iteration 3. The answer is: **single-substrate with optional cofactor is sufficient for v1**, and two-substrate reactions should be listed as a v2 extension.

**Arguments for sufficiency:**

1. **The cofactor mechanism already provides two-input reactions.** The existing Reaction struct has a `cofactor` field (u8, 0xFF = none). When a cofactor is specified, the reaction rate is modulated by cofactor availability: `rate *= cofactor_conc / (0.1 + cofactor_conc)`. The cofactor is partially consumed (50%). This is functionally a two-substrate reaction with asymmetric stoichiometry: the substrate is fully consumed, the cofactor is partially consumed, and the product is produced. This covers the most important case: reactions that require two inputs.

2. **Real enzyme kinetics are dominated by single-substrate transformations at the abstract level.** While bisubstrate reactions account for ~60% of known enzymes, many of these are ping-pong mechanisms where the enzyme processes substrates sequentially. At MARL's abstraction level (1 tick = 1 day, abstract enzymes not literal enzymes), a ping-pong mechanism is indistinguishable from two sequential single-substrate reactions. The cofactor mechanism captures the ordered-sequential case.

3. **Combinatorial expressivity is already vast.** With R_MAX=16 reactions over M=16 internal species, the space of possible reaction networks is C(512, 16) ~ 10^30 even with single substrates. Adding a second full substrate field would increase this to C(512^2, 16) ~ 10^60, which is interesting theoretically but provides no practical benefit for evolutionary search -- the existing space is already far larger than can be explored in any feasible simulation run.

4. **GPU uniformity is preserved.** Single-substrate reactions have uniform memory access: each reaction reads 3 species concentrations (substrate, product, catalyst) plus optionally 1 cofactor. Two-substrate reactions would add a fourth mandatory read, increasing register pressure and reducing occupancy for no gameplay benefit in v1.

5. **The paper lists this as a known limitation.** The paper outline (Section 6.2) already lists "1:1 stoichiometry is a simplification (no multi-substrate reactions in v1)" as an acknowledged limitation. Reviewers will accept this if the system demonstrates sufficient evolutionary dynamics without it.

**What two-substrate reactions would add (v2):**

- True condensation reactions (A + B -> AB), which are important for modeling polymerization and macromolecule synthesis.
- More realistic metabolic network topology -- real metabolism has many fan-in reactions (multiple inputs converging to one output).
- Richer evolutionary landscape for metabolic complementarity.

**Upgrade path:** Add a `substrate_2: u8` field to the Reaction struct (1 additional byte per reaction, 16 bytes per cell). Change the rate equation to include a third Michaelis-Menten term. No other architectural changes needed. This is a clean v2 extension.

## Consequences

- This decision gates all downstream module design. Do not begin [[Modules/cell-agent]] or [[Modules/hgt-engine]] implementation until resolved.
- Option B chosen: GPU batching by ruleset type is straightforward (all cells share the same evaluation function, only parameters differ). Batch-sort by parameter cluster if needed.
- Option C chosen: GPU batching degrades as programs diverge. CPU fallback for cell update pass may be necessary at high diversity.
- Option B+E chosen: GPU batching is maintained (fixed R_max, padded). Intracellular ODE integration adds compute per cell but is uniform in structure.

## Notes

The HGT transfer probability should itself be a mutable ruleset parameter regardless of representation choice. This allows HGT propensity to evolve, which is biologically realistic and produces interesting second-order dynamics.

**Literature references:**
- Hutton, T.J. (2007). Evolvable self-reproducing cells in a two-dimensional artificial chemistry. Artificial Life, 13(1).
- Kauffman, S.A. (1986). Autocatalytic sets of proteins. Journal of Theoretical Biology, 119(1).
- Banatre, J-P. & Le Metayer, D. (1986). A new computational model and its discipline of programming. INRIA Report.
- Fontana, W. & Buss, L. (1994). "The arrival of the fittest." Bulletin of Mathematical Biology, 56(1).
- Plantec, E. et al. (2023). Flow-Lenia: Towards open-ended evolution in cellular automata. ALIFE 2023.
- Mordvintsev, A. et al. (2020). Growing Neural Cellular Automata. Distill.
- Lenski, R.E. et al. (2003). The evolutionary origin of complex features. Nature, 423.
