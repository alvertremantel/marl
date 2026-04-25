# Research: The Biochemical Abstraction Layer

**Created:** 2026-03-15 (Iteration 001)
**Purpose:** Ground MARL's interstitial/intracellular boundary and "abstract enzyme" concept in prior art, and propose concrete formalization options.

---

## 1. The Core Question

MARL needs a chemistry that is:
- **General enough** that neurons, metabolic pathways, signaling cascades, and novel structures can all emerge from the same substrate
- **Specific enough** that the interstitial (extracellular field) vs. intracellular boundary is meaningful and physically grounded
- **Efficient enough** to run on a consumer GPU at 50M voxel scale
- **Evolvable** -- the chemistry must be parameterized by the cell's ruleset, so evolution can explore the space of possible chemical behaviors

This document surveys how existing systems handle this problem and proposes options for MARL.

---

## 2. The Compartmentalization Spectrum

Different systems handle the inside/outside boundary at very different levels of abstraction:

### Level 0: No Compartments (well-stirred)
**Examples:** AlChemy, Kauffman RAF theory
- All molecules interact with all other molecules
- No spatial structure, no inside/outside
- Simplest computationally, but loses the fundamental biological reality that chemistry is compartmentalized

### Level 1: Emergent Compartments (membrane from particles)
**Examples:** Hutton AC, autopoietic vesicle models (Ganti chemoton, Varela autopoiesis)
- Membranes form from particle interactions as emergent structures
- Inside/outside distinction arises dynamically
- Beautiful but computationally expensive: requires tracking individual particles and their bonds
- Scale limit: ~10^4 particles (2D), far below MARL's 50M voxels

### Level 2: Architectural Compartments (cell = container)
**Examples:** PhysiCell, BSim, most agent-based cell models
- Each cell agent has an explicit internal state (concentrations, gene expression levels)
- The external field is separate from internal state
- Transport between internal and external is governed by explicit rules (uptake rates, secretion rates)
- **This is MARL's level.** The voxel occupied by a cell has both a field contribution (interstitial) and a cell state (intracellular).

### Level 3: Organelle-level Compartments
**Examples:** Whole-cell models (Karr et al., 2012), detailed metabolic simulations
- Multiple compartments within a cell (nucleus, mitochondria, ER, etc.)
- Far too detailed for MARL's scale and purpose

**MARL's choice: Level 2** is the right abstraction. It preserves the biologically critical inside/outside boundary while being computationally tractable at scale. The key design question is: what exactly lives "inside" vs. "outside," and how do things cross the boundary?

---

## 3. The Interstitial/Intracellular Boundary: Formal Definition

### 3.1 What Lives Where

| Domain | Data Structure | Species | Update Rule |
|--------|---------------|---------|-------------|
| **Interstitial (field)** | Dense 3D array, [N^3 x S] | S extracellular species (shared across all voxels) | Reaction-diffusion PDE (Laplacian + decay + sources) |
| **Intracellular (cell)** | Sparse per-cell vector, [M] per cell | M intracellular species (private to each cell) | Small ODE system driven by ruleset |

The interstitial field is the "public commons" -- shared, diffusible, accessible to all cells. Intracellular concentrations are private -- only the cell itself can read and modify them.

### 3.2 Boundary Crossing: The Membrane Abstraction

The cell membrane is not modeled as a physical structure. Instead, it is abstracted as a set of **transport functions** encoded in the cell's ruleset:

- **Uptake:** cell reads external concentration c_ext and imports at rate f_uptake(c_ext, params). Decrements field, increments internal.
- **Secretion:** cell writes internal concentration c_int to field at rate f_secrete(c_int, params). Decrements internal, increments field.
- **Passive exchange:** some species may have a passive equilibration rate (diffusion across membrane analog).

The transport functions are governed by ruleset parameters, which means evolution can modify:
- What a cell imports (sensitivity thresholds)
- What it exports (secretion rates)
- How selective the "membrane" is (differential transport rates)

This is biologically honest: real cell membranes are selectively permeable, with active transport driven by evolved protein machinery. MARL abstracts "protein machinery" as "ruleset parameters governing transport functions."

### 3.3 The "Abstract Enzyme" Concept

In MARL, an "enzyme" is not a literal protein. It is a **parametric transformation rule** that converts intracellular species into other intracellular species, or that modulates transport rates. Concretely:

An abstract enzyme is a tuple: (substrate_species, product_species, rate_parameters)

The cell's ruleset encodes a set of such tuples. Each tick, the intracellular ODE integrates:

```
d[P]/dt = sum over enzymes: rate(params, [S]) - decay * [P]
```

Where rate() is a Hill-function or Michaelis-Menten analog parameterized by the ruleset.

This is abstract because:
- No protein folding, no amino acid sequence, no tertiary structure
- The "enzyme" is just a parameterized reaction rule
- But it preserves the essential biological property: enzymes are specific (they act on particular substrates) and catalytic (they speed up particular transformations)
- Evolution acts on the parameters (affinities, rates, specificities) and potentially on the network topology (which reactions exist)

---

## 4. Design Options for Intracellular Chemistry

### Option A: Fixed Reaction Topology, Evolvable Parameters

The set of possible intracellular reactions is fixed at initialization (e.g., every internal species can potentially be converted to every other internal species). The ruleset encodes which of these reactions are "active" (rate > threshold) and at what rate.

**Advantages:**
- Uniform compute structure: every cell evaluates the same set of reactions, just with different parameters. GPU-friendly.
- Mutation is simple: perturb rate parameters with Gaussian noise.
- HGT is natural: transfer a subset of rate parameters.

**Disadvantages:**
- Limited novelty: the reaction topology can't evolve, only the parameters. All cells are structurally identical metabolic networks with different tuning.
- This is essentially ADR-003 Option B applied to intracellular chemistry.

**Expressivity estimate:** With M=8 internal species, the full reaction topology has M^2 = 64 possible reactions. Each with ~3 parameters (rate, affinity, cooperativity) = ~192 evolvable parameters per cell. This is manageable on GPU.

### Option B: Evolvable Reaction Topology (Catalytic Network Genome)

The ruleset IS the reaction network. Each "gene" in the ruleset encodes one reaction rule: (input species, output species, catalyst species, rate parameters). The number of active reactions can vary between cells. Mutation can add, remove, or modify reactions. HGT transfers individual reaction rules.

**Advantages:**
- Maximum expressivity: the space of possible metabolisms is combinatorially vast.
- HGT is biologically natural: transferring a "gene" = transferring a reaction rule = transferring a metabolic capability. This is exactly what real HGT does.
- Novel reaction topologies can evolve, not just parameter tuning.
- Autocatalytic sets can form: a cell might evolve a self-sustaining cycle of reactions.

**Disadvantages:**
- Variable-length rulesets are hostile to GPU batching. Different cells have different numbers of reactions.
- Evaluation cost scales with number of active reactions per cell.
- Harder to implement than fixed topology.

**This is the chemistry-first approach the director guidance suggests exploring.**

### Option C: Hybrid (Fixed Core + Evolvable Extensions)

A small fixed reaction network handles basic metabolism (energy from light, maintenance costs, basic transport). On top of this, a variable set of "specialty" reactions can be evolved. The fixed core ensures all cells can survive; the evolvable extensions determine niche specialization.

**Advantages:**
- GPU-friendly core (fixed reactions, uniform across all cells)
- Evolutionary richness in the extensions
- Graceful degradation: if all specialty reactions are lost through mutation, the cell still has basic metabolism

**Disadvantages:**
- Arbitrary distinction between "core" and "extension" -- what's core?
- Limits the generality ("everything explorer" principle): if some behaviors require modifying the core, they're inaccessible

---

## 5. How Other Systems Handle This

### PhysiCell
PhysiCell's cells have researcher-defined phenotypes with fixed sets of behaviors (secretion rates, uptake rates, proliferation thresholds). These are configured via XML, not evolved. The internal state is essentially a flat parameter vector.

**Lesson:** PhysiCell validates that a flat parameter vector is sufficient for rich cell behaviors in a field-mediated context. But PhysiCell doesn't need evolvability.

### Hutton AC
Hutton's cells have linear genomes encoding enzyme sequences. Each enzyme catalyzes a specific reaction. The genome is literally the chemistry -- reading the genome produces the enzymes that run the metabolism.

**Lesson:** Genome-as-catalytic-network is viable and produces self-reproduction. But Hutton operates at particle scale, not field scale.

### AlChemy
AlChemy molecules ARE functions (lambda expressions). Interaction IS computation. There is no distinction between "genome" and "metabolism" -- every molecule is both data and program.

**Lesson:** Collapsing the genome/metabolism distinction is elegant but makes compartmentalization harder. MARL benefits from keeping them separate: the ruleset (genome) defines the reactions; the internal concentrations (metabolism) are the state.

---

## 6. Recommendation

**Option B (Evolvable Reaction Topology)** is the most promising for MARL's goals, with practical constraints addressed as follows:

### Addressing GPU Batching
- Cap the maximum number of reactions per cell (e.g., R_max = 16)
- Pad shorter rulesets with no-op reactions (rate = 0)
- All cells evaluate R_max reactions, but inactive ones contribute zero. This wastes some compute but maintains uniform instruction flow.
- Sort cells by active-reaction-count for partial batching optimization

### Addressing HGT
- HGT transfers individual reaction rules (genes). A daughter cell receives some reaction rules from a neighbor, potentially gaining a new metabolic capability.
- This is the most biologically natural HGT model -- it directly parallels real horizontal gene transfer of metabolic operons.

### Addressing Autocatalytic Emergence
- With enough intracellular species (M >= 6) and enough reaction slots (R_max >= 8), the combinatorial space of possible metabolisms is vast.
- Autocatalytic cycles (species A catalyzes production of B, B catalyzes production of A) can emerge naturally.
- Kauffman's theory predicts this will happen above a certain reaction density threshold.

### Concrete Data Structure Sketch

```
Reaction {
    input_species:   u8,          // which internal species is consumed (0..M-1)
    output_species:  u8,          // which internal species is produced (0..M-1)
    catalyst:        u8,          // which species catalyzes (0..M-1, or NONE)
    k_m:             float16,     // Michaelis-Menten half-saturation
    v_max:           float16,     // maximum rate
    cooperativity:   float16,     // Hill coefficient
}
// Size: ~8 bytes per reaction

Ruleset {
    reactions:       [R_max] Reaction,  // intracellular reaction rules
    transport:       [S] TransportParams, // uptake/secretion for each external species
    fate_thresholds: FateParams,        // energy thresholds for division/death
    hgt_propensity:  float16,           // probability of accepting foreign genes
    mutation_rate:   float16,           // per-parameter mutation rate
}
// Size: R_max * 8 + S * 6 + 10 ~ 150-200 bytes per cell (R_max=16, S=8)
```

At 100K live cells (sparse population), total ruleset storage: ~20 MB. Easily fits in VRAM alongside the 0.8 GB field.

---

## 7. Open Questions for Future Iterations

1. **Should internal species overlap with external species?** i.e., can a cell's internal "oxygen analog" be the same chemical as the field's "oxygen analog," or are they separate namespaces? Overlapping is more physically honest (real cells contain real oxygen) but increases coupling complexity.

2. **Energy currency:** Should there be a distinguished "energy" species (like ATP), or should energy be an emergent property of favorable reactions? A distinguished energy currency simplifies fate decisions but is less general.

3. **Enzyme degradation:** Should intracellular enzymes (reaction rules) have a "durability" that degrades over time, requiring the cell to continuously "express" them? This adds a metabolic cost to maintaining complex metabolisms and creates selection pressure toward simpler rulesets in nutrient-poor environments.

4. **Catalytic vs. stoichiometric:** Should reactions consume the catalyst species, or is catalysis truly catalytic (catalyst is not consumed)? True catalysis is more biologically correct but means a single molecule of catalyst can drive unlimited reaction -- potentially causing numerical instability.

5. **External reactions:** Should the interstitial field support reactions between extracellular species (not just diffusion and decay)? Real extracellular chemistry includes spontaneous oxidation, pH-dependent reactions, etc. This would add richness but increase field-update compute cost.
