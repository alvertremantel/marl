# 007 -- Energy Currency Formalization

**Date:** 2026-03-15
**Status:** Accepted
**Updated:** 2026-03-15 (Iteration 006)

## Context

Since the B+E hybrid was accepted (ADR-003, Iteration 002), internal species 0 has been informally designated as "energy" -- the currency that gates cell fate decisions (division, quiescence, death). The cell-agent module spec says:

> **Fate decision:** Internal species 0 ("energy carrier") compared against evolvable thresholds for death, quiescence, and division.

But this was never formally decided. The question is: **is energy a distinguished species with hardcoded special rules, or is it just another internal species that happens to be referenced by the fate decision layer?**

This matters for three reasons:

1. **Generality.** MARL aspires to be an "everything explorer." Hardcoding energy as special narrows the design space. If internal species 0 has unique physics (e.g., mandatory decay, special conservation laws), then we've introduced a predefined cell type constraint through the back door.

2. **Evolvability.** Can evolution rewire which internal species gates fate decisions? If the fate thresholds reference a hardcoded species index, then evolution cannot discover alternative energy currencies. If the index is evolvable, then what started as "energy" might evolve to mean something else entirely.

3. **Biological realism (at the abstract level).** Real cells do not have a single "energy" variable. They have ATP, NADH, proton motive force, membrane potential -- multiple interconvertible energy carriers. The abstraction level matters: too simple and we lose dynamics; too complex and we lose tractability.

---

## Options Considered

### Option A: Hardcoded Energy Species

Internal species 0 is permanently designated as "energy." The fate decision layer always reads `internal_conc[0]`. This index is not evolvable.

**Behavior:**
- Fate thresholds (division, quiescence, death) always compare against `internal_conc[0]`
- Reactions that produce species 0 are "energy-producing reactions" by definition
- Species 0 is not semantically different in the reaction network -- it participates in reactions like any other species -- but it has a privileged role in the fate layer

**Advantages:**
- Simplest implementation. The fate decision is a fixed function of one known variable.
- Interpretable. When analyzing a simulation, "energy" always means species 0. Cross-lineage comparisons are meaningful.
- Biologically defensible. Real cells all use ATP as the universal energy currency (at the abstract level). Fixing a single energy carrier captures this universality.
- GPU-friendly. The fate decision reads from a known offset, enabling compiler optimization.

**Disadvantages:**
- Reduces evolvability of the fate decision. Evolution can only modify the thresholds, not which signal drives division.
- Imposes a semantic constraint on species 0 that may bias evolution. If species 0 is always the bottleneck for division, evolution is forced to optimize for species 0 production, potentially crowding out alternative metabolic strategies.

### Option B: Evolvable Fate Species Index

The fate decision layer references an evolvable species index `fate_species: u8` (range 0..M-1). Mutation can change which internal species gates division.

**Behavior:**
- Fate thresholds compare against `internal_conc[fate_species]`
- Different lineages could evolve to use different internal species as their "energy"
- `fate_species` is part of the ruleset and subject to mutation and HGT

**Advantages:**
- Maximum evolutionary freedom. Cells could evolve to divide based on accumulated signal, structural precursor, or any other internal quantity.
- Avoids privileging any species semantically.
- Enables an interesting evolutionary dynamic: lineages that evolve more robust fate-gating signals might have advantages.

**Disadvantages:**
- Makes cross-lineage comparison harder. "Energy" means different things to different cells. Metrics like "total energy in the system" become meaningless.
- Mutations to `fate_species` are likely catastrophic. Switching the species that gates division from a well-tuned production pathway to a random species will almost certainly kill the cell. This creates a strong selection pressure against mutations to `fate_species`, making it effectively frozen after initial evolution.
- Adds one byte to the ruleset. Negligible.
- Complicates the narrative for publication. Reviewers will ask "what is energy?" and the answer "it depends on the cell" is harder to explain.

### Option C: Multi-Species Fate Function (Weighted Sum)

The fate decision is a weighted sum of multiple internal species:

```
fate_signal = sum_over_i(w_i * internal_conc[i])
```

where the weights `w_i` are evolvable parameters (float16). Some weights may be negative (toxin accumulation triggers death). The fate thresholds compare against `fate_signal`.

**Behavior:**
- Division occurs when `fate_signal > division_threshold`
- Death occurs when `fate_signal < death_threshold`
- The weights determine how the cell integrates multiple metabolic signals into a fate decision

**Advantages:**
- Most biologically realistic. Real cell fate is determined by integration of multiple signals (ATP level, growth factor concentrations, DNA damage sensors, nutrient availability).
- Smooth evolutionary landscape. Small changes to weights produce small changes in fate behavior. No catastrophic "wrong species" mutations.
- Enables emergent complexity. A cell could evolve to require BOTH sufficient energy AND sufficient structural precursor to divide -- a primitive cell-cycle checkpoint.
- Still has an identifiable "energy-like" signal (the dominant positive weight), but it's emergent rather than imposed.

**Disadvantages:**
- Adds M weights to the ruleset: M x 2 bytes = 32 bytes at M=16. Small but not negligible.
- The weighted sum is a linear combination. Nonlinear fate decisions (e.g., "divide only if species 3 > 0.5 AND species 7 < 0.2") would require a more complex architecture. But the existing Hill-function receptor layer already provides nonlinearity -- the question is whether that nonlinearity should propagate into the fate layer.
- Risks of degenerate evolution: all weights might converge to the same pattern across lineages, effectively recovering Option A. This is not a problem -- it means Option A is a special case of Option C.

### Option D: Fate via Receptor Layer (No Separate Fate Species)

Eliminate the dedicated fate decision entirely. Instead, the receptor layer (which already produces activation signals via Hill functions) drives fate. One or more receptor outputs are wired to "fate effectors" -- internal virtual species that gate division/death.

**Behavior:**
- The receptor pass produces activation signals from external chemistry
- Some of these activations are wired (via evolvable indices) to "fate gates"
- Division occurs when the appropriate internal species (produced by the reaction network, driven by receptor-gated uptake) exceeds a threshold

**Advantages:**
- No special fate mechanism at all. Fate emerges from the same receptor-reaction-effector pipeline as everything else.
- Maximum design elegance -- the entire cell is one coherent catalytic network.

**Disadvantages:**
- Fate becomes fragile. If the reaction network mutates in a way that disrupts the pathway feeding the fate species, the cell dies without clear diagnostic signal.
- Bootstrapping is harder. A random ruleset must accidentally produce a pathway that sustains the right internal concentration to survive. The epsilon background rate helps, but the search space is larger.
- Less interpretable for analysis.

---

## Decision: Option A (Hardcoded Energy Species) with a Structured Generalization Path

**Accept Option A for v1.** Internal species 0 is the energy currency. The fate decision always reads `internal_conc[0]`. This index is not evolvable.

### Rationale

1. **The "frozen fate_species" argument seals it.** Option B's key advantage (evolvable fate index) is theoretical. In practice, mutations to `fate_species` are almost always lethal, so the parameter would evolve to a fixed value and stay there. Option A acknowledges this inevitability and simplifies accordingly.

2. **Interpretability matters for publication.** The paper needs clear energy accounting. "Total energy" must be a well-defined system-level quantity for the OEE metrics (assembly index, predictive information, etc.). A floating definition of energy per lineage makes this impossible.

3. **Option A does not prevent internal complexity.** Cells can still evolve complex metabolic networks that produce species 0 through multi-step pathways. The only constraint is that species 0 is what gets compared to fate thresholds. This is analogous to how real cells have complex metabolism but ATP is the universal checkpoint currency.

4. **Option C is the natural v2 generalization.** If v1 experiments show that the hardcoded energy species constrains evolutionary diversity, Option C (weighted fate signal) can be added without architectural changes -- it merely replaces the single-species read with a dot product. The single-species case (Option A) is the special case where w_0 = 1 and all other weights are zero.

5. **Egbert, Barandiaran, & Di Paolo (2003) argue that metabolism should be constitutive, not imposed.** In MARL, the energy species IS constitutive -- its production and consumption are governed entirely by the evolvable reaction network. The only "imposed" aspect is that species 0 is the one that matters for fate. This is a minimal imposition and analogous to the role of ATP in biochemistry: ATP's chemical structure is arbitrary, but its role as the universal energy carrier is deeply conserved and effectively frozen by selection.

### What This Means Concretely

The fate decision layer in cell-agent.md is:

```
energy = internal_conc[0]

if energy < death_threshold:
    emit Death event
elif energy < quiescence_threshold:
    cell enters quiescence (effectors suppressed, maintenance decay continues)
elif energy > division_threshold:
    emit Division event (daughter placed in adjacent empty voxel)
```

Where `death_threshold`, `quiescence_threshold`, and `division_threshold` are evolvable parameters in the ruleset (part of the fate block, 6 bytes total).

**Energy is not conserved globally.** There is no law of conservation of energy in MARL. Cells produce energy (species 0) by converting other internal species via their reaction network. Cells consume energy through maintenance decay (a constant per-tick drain on species 0). The field does not contain "energy" -- energy is purely intracellular. This is intentional: energy is an abstraction of metabolic capability, not a physical quantity.

**Energy maintenance cost.** Every cell loses a fixed amount of species 0 per tick (maintenance decay, `lambda_maintenance`). This is NOT part of the reaction network -- it is a hardcoded drain. This ensures that cells must actively produce energy to survive, preventing "do-nothing" rulesets from persisting indefinitely. The maintenance rate is a global constant, not evolvable per cell.

| Parameter | Value | Evolvable? | Notes |
|-----------|-------|-----------|-------|
| Energy species index | 0 | No | Hardcoded. All cells use internal_conc[0] for fate. |
| death_threshold | 0.05 (default) | Yes | Below this, cell dies. |
| quiescence_threshold | 0.2 (default) | Yes | Below this, effectors suppressed. |
| division_threshold | 0.8 (default) | Yes | Above this, cell divides. |
| lambda_maintenance | 0.02 (default) | No | Per-tick energy drain. Global constant. |

### Why Not Evolvable Maintenance?

Making `lambda_maintenance` evolvable would let cells evolve lower maintenance costs, which is equivalent to evolving to "need less energy." In the limit, a cell with `lambda_maintenance = 0` is immortal even with a null ruleset. This defeats the purpose of energy as a selective pressure. A fixed maintenance cost ensures that **all cells face the same minimal metabolic challenge**, and the selective advantage comes from how efficiently they meet it.

This is analogous to basal metabolic rate in biology: while organisms have different total metabolic rates, there is a fundamental cost of maintaining cellular machinery that cannot be eliminated by evolution (due to thermodynamic constraints). In MARL, `lambda_maintenance` plays this role.

---

## Consequences

1. **Internal species 0 is "energy" everywhere in the spec.** All documents referencing energy should use this convention. The Winogradsky mock already does (species 0 = energy-carrier).

2. **The fate block in the ruleset is 6 bytes:** three float16 thresholds (death, quiescence, division). No species index needed.

3. **Energy is intracellular only.** It does not exist in the external field. This is consistent with the separate namespace design (ADR-006): internal species have no mandatory correspondence to external species. Cells produce energy from whatever external chemicals they import and transform.

4. **lambda_maintenance must be tuned empirically.** Too high and cells die before establishing metabolism. Too low and null rulesets persist too long. The default of 0.02 means a cell with no energy production loses all energy in ~50 ticks (1/0.02), which is ~50 simulated days. This gives nascent cells enough time to bootstrap metabolism via the epsilon background rate.

5. **v2 upgrade path is clean.** Replace `energy = internal_conc[0]` with `energy = dot(weights, internal_conc)` and add M x 2 bytes to the ruleset. No other changes needed.

---

## References

- Egbert, M.D., Barandiaran, X.E., & Di Paolo, E.A. (2010). Behavioral Metabolution: Metabolism Based Behavior Enables New Forms of Adaptation and Evolution. Proc. ALIFE XII.
- Egbert, M.D., Barandiaran, X.E., & Di Paolo, E.A. (2003). Artificial Metabolism: Towards True Energetic Autonomy in Artificial Life. Proc. ECAL 2003. Springer LNAI 2801.
- Lane, N. & Martin, W.F. (2010). The energetics of genome complexity. Nature, 467, 929-934. [Universal energy currency argument: ATP is deeply conserved because switching currencies is catastrophically expensive]
- Ruiz-Mirazo, K. & Moreno, A. (2004). Basic autonomy as a fundamental step in the synthesis of life. Artificial Life, 10(3), 235-259. [Argues for metabolism as constitutive, not decorative]
