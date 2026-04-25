# Mock: One Complete Cell Update Tick Under the B+E Hybrid Model

**Created:** 2026-03-15 (Iteration 002)
**Purpose:** Concrete pseudocode walkthrough of a single cell's update tick under the B+E hybrid architecture (parametric receptors/effectors + catalytic reaction network core). Demonstrates how a catalyst transforms substrates, how the receptor layer reads the field, how the effector layer writes back, and how HGT transfers a reaction rule (not just a parameter).

---

## 1. Data Structures

```
// === FIELD (dense, all voxels) ===
// S = 8 extracellular species
// Stored as [500 x 500 x 200 x 8] float16 array
// Species semantics are emergent, but initial seeding suggests:
//   0: "light-energy-carrier"  (produced by photosynthesis analog)
//   1: "oxidant"               (like O2, diffuses from top)
//   2: "reductant"             (like H2S, sourced from bottom)
//   3: "carbon-source"         (like CO2, ambient)
//   4: "organic-waste"         (secreted by metabolism)
//   5: "signal-A"              (freely evolvable signaling molecule)
//   6: "signal-B"              (freely evolvable signaling molecule)
//   7: "structural-deposit"    (slow-diffusing, for biofilm matrix)

// === CELL STATE (sparse, per living cell) ===
struct CellState {
    pos:            (u16, u16, u16),     // voxel coordinates
    lineage_id:     u64,
    age:            u32,

    // --- Intracellular concentrations (M = 8 internal species) ---
    // Internal species are a SEPARATE namespace from external species.
    // Transport functions map between the two namespaces.
    internal:       [8] float32,         // float32 for ODE stability

    // --- Ruleset (the genome) ---
    ruleset:        Ruleset,
}

struct Ruleset {
    // --- Layer 1: Receptor/Transduction (Option B) ---
    // Fixed architecture. Each external species has a receptor.
    // Hill function: activation_i = c_ext_i^n_i / (k_i^n_i + c_ext_i^n_i)
    receptors:      [S] ReceptorParams,  // S = 8, one per external species

    // --- Layer 2: Transport (membrane crossing) ---
    // Maps external species to/from internal species.
    // uptake_rate and secrete_rate govern bidirectional transport.
    transport:      [S] TransportParams, // S = 8, one per external species

    // --- Layer 3: Intracellular Catalytic Network (Option E) ---
    // R_max = 16 reaction slots. Each is an "abstract enzyme."
    // Inactive reactions have v_max = 0 (no-ops, but still evaluated).
    reactions:      [16] Reaction,

    // --- Layer 4: Effector/Fate ---
    // Maps internal concentrations to secretion rates and fate decisions.
    effectors:      [S] EffectorParams,
    fate:           FateParams,

    // --- Meta-parameters ---
    hgt_propensity: float16,             // probability of accepting foreign genes
    mutation_rate:  float16,             // per-parameter mutation rate
}

struct ReceptorParams {
    k_half:         float16,             // half-saturation concentration
    n_hill:         float16,             // Hill coefficient (cooperativity)
    gain:           float16,             // scaling factor for activation signal
}
// Size: 6 bytes per receptor, 48 bytes total for S=8

struct TransportParams {
    uptake_rate:    float16,             // max import rate (external -> internal)
    secrete_rate:   float16,             // max export rate (internal -> external)
    ext_species:    u8,                  // which external species this transporter handles
    int_species:    u8,                  // which internal species it maps to
}
// Size: 6 bytes per transporter, 48 bytes total for S=8

struct Reaction {
    substrate:      u8,                  // input internal species (consumed)
    product:        u8,                  // output internal species (produced)
    catalyst:       u8,                  // which internal species catalyzes this
    cofactor:       u8,                  // optional second input (consumed), 0xFF = none
    k_m:            float16,             // Michaelis constant (substrate affinity)
    v_max:          float16,             // maximum rate when substrate is saturated
    k_cat:          float16,             // catalyst half-saturation (see Section 3)
}
// Size: 10 bytes per reaction, 160 bytes total for R_max=16

struct EffectorParams {
    threshold:      float16,             // internal conc above which secretion occurs
    rate:           float16,             // secretion rate scaling
    int_species:    u8,                  // which internal species drives this effector
    ext_species:    u8,                  // which external species is secreted
}
// Size: 6 bytes per effector, 48 bytes total for S=8

struct FateParams {
    division_energy:  float16,           // internal[0] threshold for division
    death_energy:     float16,           // internal[0] threshold for death
    quiescence_energy: float16,          // threshold for quiescence (suspend effectors)
}
// Size: 6 bytes

// TOTAL RULESET SIZE:
//   receptors:  48
//   transport:  48
//   reactions: 160
//   effectors:  48
//   fate:        6
//   meta:        4
//   TOTAL:     314 bytes per cell
//   At 100K cells: ~31 MB. At 1M cells: ~314 MB. Fits in 8 GB with field.
```

---

## 2. The Complete Cell Update Tick (Pseudocode)

This runs once per tick for each living cell. On GPU, all cells execute this in parallel.

```
function cell_tick(cell: CellState, field: Field, light: LightField, dt: float)
    -> (field_deltas: [S] float, event: Event?)
{
    // ============================================================
    // PHASE 1: RECEPTOR PASS (read external field)
    // Fixed-topology parametric layer. GPU-uniform: same code path
    // for all cells, only parameters differ.
    // ============================================================

    let ext_conc: [S] float = field.read(cell.pos)  // local concentrations
    let light_here: float = light.read(cell.pos)     // light availability

    // Compute activation signal for each external species
    let activation: [S] float
    for i in 0..S:
        let c = ext_conc[i]
        let k = cell.ruleset.receptors[i].k_half
        let n = cell.ruleset.receptors[i].n_hill
        let g = cell.ruleset.receptors[i].gain
        // Hill function: sigmoidal response to external concentration
        activation[i] = g * (c^n) / (k^n + c^n)

    // Light is treated as an additional activation signal.
    // It maps to internal species 0 ("energy carrier") via a
    // special photosynthesis-like transport that is part of the
    // receptor layer, not the reaction network.
    let light_activation = light_here  // linear, no Hill (could evolve)

    // ============================================================
    // PHASE 2: TRANSPORT PASS (membrane crossing)
    // Move chemicals across the interstitial/intracellular boundary.
    // This is where the compartmentalization is enforced.
    // ============================================================

    let field_deltas: [S] float = [0.0; S]

    for i in 0..S:
        let tp = cell.ruleset.transport[i]
        let ext_idx = tp.ext_species
        let int_idx = tp.int_species

        // Uptake: external -> internal
        // Rate depends on external concentration (Michaelis-Menten-like)
        let uptake = tp.uptake_rate * ext_conc[ext_idx]
                     / (1.0 + ext_conc[ext_idx])

        // Secretion: internal -> external
        // Rate depends on internal concentration
        let secretion = tp.secrete_rate * cell.internal[int_idx]
                        / (1.0 + cell.internal[int_idx])

        // Apply transport (conserving mass)
        cell.internal[int_idx] += (uptake - secretion) * dt
        field_deltas[ext_idx]  += (secretion - uptake) * dt

    // Light-driven energy input (special case: no external species consumed,
    // energy appears in internal species 0)
    cell.internal[0] += light_activation * dt * 0.1  // scaling factor

    // ============================================================
    // PHASE 3: INTRACELLULAR REACTION NETWORK (the catalytic core)
    // This is Option E. Each reaction is an "abstract enzyme."
    // The catalyst must be PRESENT at sufficient concentration
    // for the reaction to proceed — this is the key design choice.
    // ============================================================

    // Evaluate all R_max reactions. Inactive reactions (v_max=0) are no-ops.
    // This loop is fixed-length, so GPU threads stay in lockstep.

    let internal_deltas: [M] float = [0.0; M]

    for r in 0..R_MAX:
        let rxn = cell.ruleset.reactions[r]

        // Skip inactive reactions (v_max == 0)
        if rxn.v_max == 0.0:
            continue  // branch is fine — all threads check same condition type

        let substrate_conc = cell.internal[rxn.substrate]
        let catalyst_conc  = cell.internal[rxn.catalyst]

        // --- THE CATALYST MECHANISM ---
        // The reaction rate depends on BOTH substrate and catalyst concentration.
        // Catalyst is NOT consumed (true catalysis).
        // But catalyst must be present — a cell with zero catalyst gets zero rate.
        //
        // rate = v_max * [S]/(k_m + [S]) * [C]/(k_cat + [C])
        //
        // This is a product of two Michaelis-Menten terms:
        //   - First term: substrate saturation (standard enzyme kinetics)
        //   - Second term: catalyst availability (the "enzyme" must exist)
        //
        // Why this matters for evolution:
        //   - A cell must PRODUCE its catalysts to run its metabolism
        //   - Catalysts are themselves products of other reactions
        //   - This creates natural dependency chains: to run reaction R,
        //     you need catalyst C, which is produced by reaction Q,
        //     which needs catalyst B, which is produced by...
        //   - Autocatalytic loops (A catalyzes B, B catalyzes A) are
        //     the self-sustaining "metabolism" of the cell
        //   - Kauffman's RAF theory predicts these loops form spontaneously
        //     above a critical reaction density

        let substrate_term = substrate_conc / (rxn.k_m + substrate_conc + 1e-6)
        let catalyst_term  = catalyst_conc  / (rxn.k_cat + catalyst_conc + 1e-6)

        // EPSILON: uncatalyzed background rate (see research-bootstrapping.md)
        // Allows reactions to proceed at 0.1% of max rate without catalyst.
        // Solves the autocatalytic bootstrapping problem while preserving
        // the 1000x evolutionary advantage of functional catalysis.
        let EPSILON = 0.001  // simulation parameter, tunable
        let rate = rxn.v_max * substrate_term * (EPSILON + catalyst_term)

        // Optional cofactor consumption (if cofactor != 0xFF)
        let cofactor_available = 1.0
        if rxn.cofactor != 0xFF:
            cofactor_available = cell.internal[rxn.cofactor]
                                / (0.1 + cell.internal[rxn.cofactor] + 1e-6)

        let effective_rate = rate * cofactor_available * dt

        // Apply stoichiometry
        internal_deltas[rxn.substrate] -= effective_rate    // substrate consumed
        internal_deltas[rxn.product]   += effective_rate    // product produced
        // catalyst is NOT consumed (catalytic)
        if rxn.cofactor != 0xFF:
            internal_deltas[rxn.cofactor] -= effective_rate * 0.5  // cofactor partially consumed

    // Apply all reaction deltas atomically
    for i in 0..M:
        cell.internal[i] += internal_deltas[i]
        cell.internal[i] = max(0.0, cell.internal[i])  // concentrations can't go negative

    // --- Maintenance cost: all internal species decay slowly ---
    for i in 0..M:
        cell.internal[i] *= (1.0 - 0.01 * dt)  // 1% decay per tick

    // ============================================================
    // PHASE 4: EFFECTOR PASS (write back to field)
    // Maps internal state to secretion. Fixed topology, parametric.
    // ============================================================

    for i in 0..S:
        let eff = cell.ruleset.effectors[i]
        let int_conc = cell.internal[eff.int_species]
        if int_conc > eff.threshold:
            let secrete_amount = eff.rate * (int_conc - eff.threshold) * dt
            field_deltas[eff.ext_species] += secrete_amount
            cell.internal[eff.int_species] -= secrete_amount

    // ============================================================
    // PHASE 5: FATE DECISION
    // Based on internal species 0 ("energy carrier") concentration.
    // ============================================================

    let energy = cell.internal[0]
    cell.age += 1

    let event: Event? = null

    if energy < cell.ruleset.fate.death_energy:
        event = Event::Death(cell.pos)
    elif energy > cell.ruleset.fate.division_energy:
        // Check for adjacent empty voxel
        let target = find_empty_adjacent(cell.pos)
        if target != null:
            event = Event::Reproduce(cell.pos, target)
            // Division cost: split internal concentrations
            for i in 0..M:
                cell.internal[i] *= 0.5  // parent keeps half
    elif energy < cell.ruleset.fate.quiescence_energy:
        // Quiescent: alive but metabolically reduced
        // (effectors were already evaluated above; could skip them
        //  in a future optimization)
        pass

    return (field_deltas, event)
}
```

---

## 3. The Catalyst Mechanism: Worked Example

Consider a cell with these three reactions active (others have v_max=0):

```
Reaction 0: substrate=2(reductant-int), product=0(energy), catalyst=3(enzyme-A), v_max=1.0
Reaction 1: substrate=3(raw-material),  product=3(enzyme-A),catalyst=4(enzyme-B), v_max=0.5
Reaction 2: substrate=3(raw-material),  product=4(enzyme-B),catalyst=0(energy),   v_max=0.3
```

This forms a dependency chain:
- Reaction 0 produces **energy** from a reductant, but only if **enzyme-A** is present
- Reaction 1 produces **enzyme-A** from raw material, but only if **enzyme-B** is present
- Reaction 2 produces **enzyme-B** from raw material, but only if **energy** is present

This is an **autocatalytic loop**: energy -> enzyme-B -> enzyme-A -> energy. Once bootstrapped (any nonzero amount of energy), it is self-sustaining. If any component drops to zero, the loop collapses.

Bootstrapping happens via:
- **Uncatalyzed background rate (EPSILON=0.001):** All reactions proceed at 0.1% of max rate even without catalyst. This trickle produces trace catalyst, which increases the rate, which produces more catalyst. See [[research-bootstrapping]] for full analysis.
- Light input (Phase 2 adds energy to internal[0] unconditionally)
- HGT donation (a neighbor cell already running this loop donates a reaction rule)
- Random mutation creating a reaction that produces a needed catalyst
- Nonzero initial internal concentrations (cells seeded with enzyme[5,6,7] = 0.01)

This is the core evolutionary dynamic: cells must evolve and maintain autocatalytic loops to survive. HGT can introduce new loops or repair broken ones.

---

## 4. HGT of a Reaction Rule (Not Just a Parameter)

When a cell reproduces (Phase 5 triggers Reproduce event), the HGT engine processes the daughter:

```
function hgt_process(parent: CellState, daughter: CellState, neighbors: [CellState])
{
    // Step 1: Copy parent ruleset to daughter (vertical inheritance)
    daughter.ruleset = copy(parent.ruleset)

    // Step 2: Point mutations on ALL layers
    for each mutable parameter p in daughter.ruleset:
        if random() < daughter.ruleset.mutation_rate:
            p += gaussian(0, sigma_p)  // sigma depends on parameter type
            p = clamp(p, valid_range)

    // Topology mutation: with small probability, rewire a reaction
    if random() < 0.01:  // topology mutation rate (could be evolvable too)
        let slot = random_int(0, R_MAX)
        // Rewire: change substrate, product, or catalyst species index
        daughter.ruleset.reactions[slot].substrate = random_int(0, M)
        daughter.ruleset.reactions[slot].product   = random_int(0, M)
        daughter.ruleset.reactions[slot].catalyst   = random_int(0, M)
        // Keep kinetic params from parent (or randomize — design choice)

    // Step 3: HORIZONTAL GENE TRANSFER
    // Key design: HGT transfers a COMPLETE REACTION RULE, not a single parameter.
    // This parallels real biology where HGT transfers entire operons
    // (gene clusters encoding complete metabolic capabilities).
    //
    // Reference: Pal et al. (2005, Nature Genetics) showed that bacterial
    // metabolic networks grow primarily by HGT of transport and catalysis genes.

    for neighbor in neighbors:
        if neighbor.lineage_id == parent.lineage_id:
            continue  // skip clonemates (no genetic novelty)

        if random() < parent.ruleset.hgt_propensity:
            // Select a random reaction from the neighbor
            let donor_slot = random_int(0, R_MAX)
            let donor_rxn  = neighbor.ruleset.reactions[donor_slot]

            // Only transfer if the donor reaction is active (v_max > 0)
            if donor_rxn.v_max == 0.0:
                continue

            // Place into a random slot in the daughter
            let target_slot = random_int(0, R_MAX)

            // OVERWRITE the daughter's reaction at that slot
            // This is "gene replacement" — the daughter loses one reaction
            // and gains a different one from the neighbor.
            daughter.ruleset.reactions[target_slot] = copy(donor_rxn)

            // CRITICAL: The transferred reaction refers to species by INDEX.
            // Since all cells share the same internal species namespace
            // (M species, same indices everywhere), the reaction rule
            // is immediately functional in its new host.
            //
            // This is what makes HGT "transfer a metabolic capability":
            //   - If the neighbor has a reaction that converts species 2 -> 0
            //     catalyzed by species 3, and the daughter receives it,
            //     the daughter now has that metabolic capability.
            //   - Whether it's USEFUL depends on whether the daughter
            //     has the required catalyst (species 3) at sufficient
            //     concentration — which depends on the rest of its metabolism.
            //
            // This creates a natural "compatibility" filter:
            //   - Transferred reactions that fit the recipient's existing
            //     metabolic network (produce needed catalysts, consume
            //     available substrates) are beneficial
            //   - Transferred reactions that don't fit are neutral or harmful
            //     (they consume substrates the cell needs, or produce
            //     products it can't use)
            //   - This is evolution acting on the network, not just on parameters

            break  // at most one HGT event per reproduction

    // Step 4: Lineage bookkeeping
    daughter.lineage_id = new_lineage_id(parent.lineage_id, generation++)

    // Step 5: Initialize daughter internal concentrations
    // Daughter gets half of parent's internal concentrations (cell division)
    for i in 0..M:
        daughter.internal[i] = parent.internal[i]  // already halved in cell_tick
}
```

---

## 5. Why This Design Works for Open-Ended Evolution

### 5.1 Autocatalytic Self-Sustenance
Cells must maintain autocatalytic loops to survive. A cell with no active autocatalytic loop will have its internal concentrations decay to zero and die. This is the "chemical self-consistency" selection pressure -- no fitness function needed.

### 5.2 Metabolic Innovation via HGT
When a cell receives a reaction rule from a neighbor, it gains a new catalytic capability. If that capability fits into an existing autocatalytic loop (or creates a new one), the cell has a genuinely novel metabolism. This is open-ended because the space of possible R_max-reaction networks over M species is combinatorially vast.

With R_max=16 reactions and M=8 species:
- Each reaction has 8 choices for substrate, 8 for product, 8 for catalyst = 512 possible reaction "types"
- A cell can have any subset of 16 from these 512
- Number of possible metabolisms: C(512, 16) ~ 10^30
- Even accounting for many being nonfunctional, the viable space is enormous

### 5.3 Chemical Ecology via Field Mediation
Cells interact only through the field. Cell A secretes species 4 (organic waste); Cell B uptakes species 4 as its carbon source. This is syntrophy -- a chemical mutualism that emerges without being programmed. The B+E hybrid supports this naturally because:
- Receptors (Layer 1) determine what the cell senses
- Transport (Layer 2) determines what crosses the membrane
- Reactions (Layer 3) determine what happens internally
- Effectors (Layer 4) determine what gets secreted
- All four layers are evolvable and transferable

### 5.4 GPU Tractability
Despite the apparent complexity, this is GPU-friendly because:
- **Fixed instruction flow:** All cells execute the same pseudocode with the same loop bounds (S=8 receptors, S=8 transporters, R_MAX=16 reactions, S=8 effectors). No branching on cell identity.
- **Uniform memory access:** Each cell reads exactly 8 floats from the field (its local concentrations) and writes 8 floats of deltas. No scatter/gather across the grid.
- **Small per-cell state:** ~314 bytes ruleset + 32 bytes internal concentrations = ~346 bytes per cell. At 100K cells, this is 34 MB -- negligible vs. the 800 MB field.

---

## 6. Numerical Considerations

### 6.1 Internal Concentrations in float32
Unlike the field (which uses float16 for memory), internal concentrations should use float32 for the ODE integration. The intracellular reaction network involves products of small numbers (concentrations near zero during bootstrapping), and float16's 3.3 decimal digits of precision would cause unacceptable quantization of rates near the catalyst threshold.

### 6.2 Stability of the Reaction ODE
The intracellular ODE is stiff when reaction rates vary widely (some reactions fast, others slow). Forward Euler is acceptable if v_max values are bounded (e.g., v_max in [0, 2.0]) and dt = 1.0 (one tick = one day, a large timestep for intracellular kinetics). The key safety measures:
- Clamp concentrations to [0, C_max] after each tick
- Bound v_max at mutation time
- The catalyst term naturally limits rates: even if v_max is large, the rate is bounded by catalyst availability
- If empirical testing shows instability, switch to a single implicit Euler step for the intracellular ODE (more expensive but unconditionally stable)

### 6.3 Conservation
The transport pass conserves mass between internal and external domains. The reaction network does NOT conserve mass (it can create or destroy internal species). This is intentional -- it models the fact that cells have access to energy (from light) and can perform thermodynamically driven synthesis. However, the field-level mass is conserved (diffusion + decay + cell sources/sinks), preventing runaway accumulation.

---

## 7. Open Questions Remaining

1. ~~**Should reactions be allowed to have TWO substrates?**~~ **Resolved (Iteration 007): No, not for v1.** Single-substrate with optional cofactor is sufficient. The cofactor mechanism already provides two-input reaction semantics (substrate consumed, cofactor partially consumed). At MARL's abstraction level, ping-pong bisubstrate mechanisms are indistinguishable from sequential single-substrate reactions. Two-substrate reactions are a clean v2 extension (add `substrate_2: u8` to the Reaction struct). See [[Decisions/003-ruleset-representation]] for full rationale.

2. **Catalyst decay/dilution:** Currently catalysts are not consumed. Should they dilute at cell division? (Yes -- internal concentrations are halved at division, including catalysts. This is already in the pseudocode.)

3. **Reaction reversibility:** Should reactions be inherently reversible (rate depends on both forward and reverse concentrations), or strictly irreversible? Reversibility is more physical but doubles the ODE terms.

4. **Rate-limiting by energy:** Should all reactions have an energy cost (consume internal[0])? This would make metabolism inherently energy-limited, which is biologically realistic. Currently only transport and maintenance have energy costs.

---

## 8. References

- Pal, C., Papp, B., & Lercher, M. J. (2005). Adaptive evolution of bacterial metabolic networks by horizontal gene transfer. Nature Genetics, 37(12), 1372-1375.
- Kauffman, S. A. (1986). Autocatalytic sets of proteins. Journal of Theoretical Biology, 119(1), 1-24.
- Hordijk, W. & Steel, M. (2017). Chasing the tail: The emergence of autocatalytic networks. BioSystems, 152, 1-10.
- Fontana, W. & Buss, L. (1994). The arrival of the fittest. Bulletin of Mathematical Biology, 56(1), 1-64.
- Hutton, T. J. (2007). Evolvable self-reproducing cells in a two-dimensional artificial chemistry. Artificial Life, 13(1), 11-30.
