# Mock: Niche Construction / Biofilm Formation Scenario Under the B+E Hybrid Model

**Created:** 2026-03-15 (Iteration 008)
**Purpose:** Third validation scenario (alongside Winogradsky and QS). Demonstrate that cells evolving to secrete structural EPS species create emergent diffusion barriers, metabolite trapping, core-periphery spatial structure, and ecological inheritance -- all from the existing B+E architecture with the niche construction mechanism specified in field-update.md. This completes the ecological-social-constructive triptych that the paper's core argument is built around.

---

## 1. The Scenario

We seed a population of identical cells on the bottom surface (z=199, substrate-attached) of a nutrient-limited environment. Nutrients diffuse in from above. A toxin diffuses in from the lateral boundaries. Cells that evolve to secrete the structural-deposit species (ext index 7) create local diffusion barriers via the D_local mechanism. The question: does EPS production evolve? Does it create a diffusion barrier that traps metabolites? Does this produce emergent biofilm architecture beyond simple vertical zonation?

Unlike the Winogradsky scenario (which seeds three distinct metabolisms) and the QS scenario (which seeds two phenotypes), this scenario starts with a **single homogeneous population**. Phenotypic differentiation must emerge through mutation and selection. This is the hardest test of the B+E model: can evolutionary novelty arise spontaneously?

---

## 2. Species Map

Using the S=12 external / M=16 internal namespace from ADR-006:

| External Index | Label | Biological Analog | Notes |
|---|---|---|---|
| 0 | energy-carrier | ATP proxy | Internal energy currency (not present in field) |
| 1 | carbon-source | Glucose / organic nutrient | Diffuses from top boundary |
| 2 | toxin | Antimicrobial / environmental stressor | Diffuses from lateral boundaries |
| 3 | waste-product | Metabolic byproduct | Produced by cells, potentially re-usable |
| 4 | signal-A | Autoinducer (not seeded, may evolve) | Available for QS evolution |
| 5 | (unused) | | |
| 6 | (unused) | | |
| 7 | structural | EPS / extracellular matrix | Modifies local D via D_local mechanism |
| 8-11 | (unused) | | Available for evolutionary innovation |

Internal species (M=16):

| Internal Index | Label | Role |
|---|---|---|
| 0 | int-energy | Energy carrier (fate decisions) |
| 1 | int-carbon | Internalized carbon nutrient |
| 2 | int-toxin | Internalized toxin (harmful) |
| 3 | int-waste | Internalized waste product |
| 4 | int-structural-precursor | Precursor for EPS secretion |
| 5 | enzyme-A | Core metabolism catalyst |
| 6 | enzyme-B | Core metabolism catalyst (autocatalytic pair) |
| 7 | enzyme-C | EPS synthesis catalyst (initially unused) |
| 8-15 | (unused) | Available for evolutionary innovation |

---

## 3. Initial Conditions

### 3.1 Field Initialization

```
Grid: 500 x 500 x 200.

External species initial concentrations:
  carbon-source[z]:  1.0 at z=0 (top), exponential decay: 1.0 * exp(-z/50)
                     Represents nutrient supply from overlying liquid medium
  toxin:             sourced from all 4 lateral boundaries at constant 0.3
                     Diffuses inward. Represents environmental stress.
  waste-product:     0.0 everywhere
  signal-A:          0.0 everywhere
  structural:        0.0 everywhere (no EPS yet)
  all others:        0.0

Boundary conditions:
  Top face (z=0):     constant carbon-source = 1.0 (nutrient reservoir)
  Bottom face (z=199): zero-flux Neumann (solid substrate)
  Lateral faces:       constant toxin = 0.3 (environmental stress)
```

### 3.2 Light Field

```
Light is NOT the primary driver in this scenario.
I_0 = 1.0 at z=0 but cells are at z~199 (bottom).
With ~200 voxels of water column: light(z=199) ~ exp(-0.001 * 200) ~ 0.82
(Low water attenuation, no cells in the column above initially.)
Light is available but weak; energy must come primarily from carbon metabolism.
```

### 3.3 Cell Seeding

```
Starter cells: 500 identical cells at z=199 (bottom surface), random (x,y)
No phenotypic variation. All cells have the same ruleset.
Occupancy: 500 / 250,000 = 0.2% of the bottom plane.
```

All cells are seeded with the **Generalist** ruleset defined below.

---

## 4. Starter Metabolism: The Generalist Ruleset

There is only ONE starter phenotype. All differentiation must evolve.

### 4.1 Transport Layer

```
Transporter 0: uptake carbon (ext 1 -> int 1), uptake_rate=0.5
Transporter 1: uptake toxin (ext 2 -> int 2), uptake_rate=0.1 (inadvertent)
Transporter 2: secrete waste (int 3 -> ext 3), secrete_rate=0.3
Transporter 3: secrete structural (int 4 -> ext 7), secrete_rate=0.0 (INACTIVE)
Others: inactive
```

Note: Transporter 3 for EPS secretion is present but disabled (rate=0). A single mutation to `secrete_rate > 0` enables EPS production. This is a realistic evolutionary distance -- a single regulatory change activates a pre-existing but silent pathway.

### 4.2 Catalytic Network

```
Reaction 0: carbon(1) -> energy(0), catalyst=enzyme-A(5), v_max=0.8, k_m=0.1
  "Core metabolism: carbon -> energy"

Reaction 1: carbon(1) -> enzyme-A(5), catalyst=enzyme-B(6), v_max=0.2, k_m=0.3
Reaction 2: carbon(1) -> enzyme-B(6), catalyst=enzyme-A(5), v_max=0.15, k_m=0.3
  "Autocatalytic enzyme loop (standard survival machinery)"

Reaction 3: carbon(1) -> waste(3), catalyst=energy(0), v_max=0.2, k_m=0.2
  "Metabolic waste production (unavoidable byproduct of high metabolism)"

Reaction 4: energy(0) -> [consumed], catalyst=toxin(2), v_max=2.0, k_m=0.01
  "TOXIN DAMAGE: toxin catalyzes energy destruction"
  substrate=energy(0), product=waste(3), catalyst=toxin(2)
  High v_max + low k_m means even trace toxin is harmful.

Reaction 5: carbon(1) -> structural-precursor(4), catalyst=enzyme-C(7), v_max=0.4, k_m=0.2
  "EPS PRECURSOR SYNTHESIS: produces structural precursor for secretion"
  Initially inactive because enzyme-C(7) starts at 0 and nothing produces it.

Reaction 6: carbon(1) -> enzyme-C(7), catalyst=enzyme-A(5), v_max=0.0, k_m=0.3
  "ENZYME-C BIOSYNTHESIS: produces the EPS synthesis catalyst"
  v_max=0 in the starter ruleset. Must be activated by mutation.
  A single mutation setting v_max > 0 bootstraps the entire EPS pathway:
    enzyme-A catalyzes enzyme-C production -> enzyme-C catalyzes structural
    precursor production -> transporter secretes structural -> D_local drops.

Reactions 7-15: inactive (v_max=0)
```

### 4.3 Receptor Layer

```
Receptor 1 (carbon): k_half=0.5, n_hill=1.0, gain=1.0
Receptor 2 (toxin):  k_half=0.1, n_hill=2.0, gain=1.0
  Hill n=2 provides steep sensitivity to toxin presence.
Others: default (k_half=1.0, n_hill=1.0, gain=0.5)
```

### 4.4 Fate Thresholds

```
division_energy:   1.5
death_energy:      0.05
quiescence_energy: 0.15
```

### 4.5 Initial Internal Concentrations

```
internal = [0.1, 0.0, 0.0, 0.0, 0.0, 0.01, 0.01, 0.0, ...]
            energy                      enzA  enzB  enzC(zero!)
```

Enzyme-C starts at zero. The EPS pathway is completely silent in the founder population.

### 4.6 Mutation Parameters

```
mutation_rate: 0.05 per reproduction
  (5% chance of any single parameter mutating per reproduction event)
topology_mutation_rate: 0.01 per reproduction
  (1% chance of a species index or v_max being randomized)
```

These rates are high enough that within ~100 generations, some lineage will discover Reaction 6 activation (v_max: 0 -> small positive). The key insight is that EPS production requires TWO mutations: (1) Reaction 6 v_max > 0, and (2) Transporter 3 secrete_rate > 0. These can occur independently and in either order. A cell with only one mutation gains nothing. Only when both are present does EPS appear in the field.

---

## 5. The Niche Construction Feedback Loop

When a cell acquires both EPS mutations, the following cascade occurs:

```
1. enzyme-A (already present) catalyzes enzyme-C production (Reaction 6)
2. enzyme-C catalyzes structural-precursor synthesis (Reaction 5)
3. Transporter 3 secretes structural-precursor into ext field as "structural" (ext 7)
4. structural[x,y,z] increases at the cell's voxel
5. D_local = D_base * (1 - 0.8 * structural / (1.0 + structural))
6. Diffusion of ALL species at this voxel slows down
7. Consequences:
   a. Carbon diffusing from above is TRAPPED near the cell (slower outward diffusion)
   b. Waste products are TRAPPED (slower clearance, potential self-poisoning)
   c. Toxin diffusing from boundaries is PARTIALLY BLOCKED (slower inward diffusion)
   d. Structural species itself is trapped (positive feedback: EPS begets more EPS)
```

This creates a **dual-edged sword** for EPS producers:

**Benefits:**
- Nutrient trapping (local carbon concentration stays higher)
- Toxin exclusion (toxin diffuses more slowly into EPS-rich regions)
- EPS persistence (slow structural decay, lambda_structural=0.005, creates ecological inheritance)

**Costs:**
- Carbon diverted from energy production to EPS synthesis (metabolic burden)
- Waste trapping (self-produced waste accumulates, potentially toxic if waste reactions evolve)
- Reduced nutrient replenishment (once local carbon is consumed, fresh carbon diffuses in slowly)

Whether EPS production is net-beneficial depends on the local ecological context. This is the heart of the niche construction dynamic.

---

## 6. Tick-by-Tick Dynamics: Expected Trajectory

### Phase 1: Colonization (Ticks 0-50)

**All 500 cells are identical generalists at z=199:**

- Carbon at z=199 is low: `1.0 * exp(-199/50) = 0.019`. This is nutrient-limited.
- Toxin at center of XY plane is low (boundaries are 250 voxels away): `~0.01-0.05` depending on decay rate.
- Toxin at edges is high: `~0.2-0.3`.

**Energy balance for a generalist at z=199, center:**
```
Carbon uptake: 0.5 * 0.019 / (1 + 0.019) = 0.0094/tick
Reaction 0 (carbon -> energy): ~0.008 * 0.83 * 0.09 = 0.0006/tick (enzymes low)
Light input: ~0.82 * 0.1 = 0.082/tick (light still available at bottom)
Toxin damage: minimal in center
Maintenance decay: 0.02 * E

Steady-state energy estimate: (0.082 + 0.0006) / 0.02 = ~4.1
```

**Light is the primary energy source at this depth.** Carbon metabolism is weak due to low carbon concentration. This creates an interesting dynamic: cells do not starve at z=199 (light sustains them), but carbon is scarce. Any mechanism that increases local carbon concentration (like EPS trapping) provides a competitive advantage in carbon-based metabolism, enzyme production, and growth rate.

**Population growth:**
- Cells near center survive (low toxin, light sustains energy)
- Cells near lateral edges suffer toxin damage and die
- Net population grows slowly, concentrated in the center
- Daughter cells placed adjacent to parents (z=198 or same z, adjacent xy)
- By tick 50: ~2,000-5,000 cells, mostly z=198-199, concentrated in central region

**Vertical expansion begins:**
- As cells divide, daughters are placed at z=198 (above parent)
- The colony starts as a monolayer and begins to thicken
- This is the flat-to-3D transition observed in real biofilms (Beroz et al., 2018)

### Phase 2: Mutation and EPS Discovery (Ticks 50-200)

**With ~5,000 cells and mutation_rate=0.05:**
- ~250 mutation events per generation (~every 3-5 ticks)
- Topology mutations (rate=0.01): ~50 per generation

**Expected EPS pathway activation timeline:**

Mutation 1 (Reaction 6, v_max: 0 -> ~0.1): This is a topology mutation on a single reaction's v_max parameter. Probability per reproduction event = 0.01 * (1/16 reactions) * (1/4 parameters per reaction) ~ 0.00016. With 250 reproductive events per tick, expected first occurrence at tick ~25. But this mutation alone does nothing visible (produces enzyme-C, produces structural precursor, but Transporter 3 has rate=0).

Mutation 2 (Transporter 3, secrete_rate: 0 -> ~0.1): Probability per reproduction event = 0.05 * (1/8 transporters) ~ 0.006. With 250 events per tick, expected at tick ~1. This mutation alone does nothing (nothing produces int-structural-precursor without enzyme-C).

**The two mutations must co-occur in the same lineage.** Given that Mutation 2 is common and Mutation 1 is rarer:
- Many cells carry Mutation 2 (silent, no cost, drifts neutrally)
- A cell carrying Mutation 2 then acquires Mutation 1: EPS pathway activates
- Expected co-occurrence: tick ~50-100

**Alternatively:** topology mutation randomizes Reaction 6's v_max directly from 0 to a nonzero value in a cell that already carries the transporter mutation.

### Phase 3: EPS Invasion or Failure (Ticks 200-500)

**When the first EPS-producing lineage appears:**

The EPS mutant faces a cost-benefit calculation:

**Cost of EPS production:**
```
Reaction 6 diverts carbon to enzyme-C production:
  rate = 0.1 * [carbon]/(0.3+[carbon]) * [enzyme-A]/(0.1+[enzyme-A])
  At [carbon]=0.02, [enzyme-A]=0.1: rate = 0.1 * 0.063 * 0.5 = 0.003/tick

Reaction 5 diverts carbon to structural precursor:
  rate = 0.4 * [carbon]/(0.2+[carbon]) * [enzyme-C]/(0.1+[enzyme-C])
  Initially low (enzyme-C starts at 0, builds up slowly)
  At steady state enzyme-C~0.05: rate ~ 0.4 * 0.09 * 0.33 = 0.012/tick

Total carbon diverted: ~0.015/tick out of ~0.01/tick available
```

**PROBLEM: At z=199 with carbon=0.019, the cost of EPS production may exceed the available carbon.** The EPS mutant diverts essentially all its carbon to EPS synthesis, leaving nothing for core metabolism.

**Resolution: The benefit must outweigh the cost, and it does -- but only after a critical mass of EPS accumulates.**

The dynamics are non-trivial:

1. **Initially, EPS mutant is energy-poor** compared to wild-type. It diverts carbon to EPS production while getting the same light energy as wild-type. It grows slightly slower.

2. **After ~10-20 ticks of EPS secretion**, structural concentration at the mutant's voxel reaches ~0.1-0.3:
```
D_local = D_base * (1 - 0.8 * 0.2/(1.0+0.2)) = D_base * 0.867
  -> 13% diffusion reduction. Modest.
```

3. **After ~50 ticks**, structural reaches ~1.0:
```
D_local = D_base * (1 - 0.8 * 1.0/(1.0+1.0)) = D_base * 0.6
  -> 40% diffusion reduction. Significant.
```
   Now carbon diffusing through this voxel lingers longer. The mutant effectively increases its local carbon concentration by ~40% compared to neighbors. If the mutant's daughter cells also produce EPS (inherited ruleset), the effect compounds.

4. **After ~100 ticks**, a cluster of EPS-producing daughters creates a contiguous EPS zone with structural ~2-5:
```
D_local = D_base * (1 - 0.8 * 3.0/(1.0+3.0)) = D_base * 0.4
  -> 60% diffusion reduction.
```
   Carbon entering the EPS zone from above accumulates. Toxin from the sides is partially excluded. The cluster experiences **higher carbon AND lower toxin** than surrounding wild-type cells.

**This is the tipping point.** The EPS cluster's growth rate now exceeds the wild-type because:
- Higher local carbon => more energy from Reaction 0
- Lower local toxin => less energy destruction from Reaction 4
- These benefits are shared among all cells in the cluster (EPS is a public good)

### Phase 4: Biofilm Architecture Emergence (Ticks 500-2000)

**The EPS-producing cluster expands outward and upward:**

As the cluster grows, a characteristic spatial structure emerges:

```
SIDE VIEW (x-z plane through biofilm center):

z=194:                    [ sparse active cells ]
z=195:               [  active growth front  ]
z=196:          [ transitional zone: moderate EPS, moderate carbon ]
z=197:      [ EPS-rich interior: HIGH structural, LOW carbon (consumed) ]
z=198:  [ EPS-rich interior: HIGH structural, VERY LOW carbon, waste accumulates ]
z=199:  [ substrate-attached base: DENSE EPS, near-zero carbon, HIGH waste ]
        =========== solid substrate ===========

TOP VIEW (x-y plane at z=197):

           wild-type territory (no EPS, low carbon, some toxin)
          /                                               \
    [toxin] [  EPS      [ EPS-rich core:    ] EPS    ] [toxin]
    [from ] [  border:  [ Low D, high waste ] border ] [from ]
    [edges] [  D drops  [ carbon consumed   ] D drops] [edges]
          \                                               /
           wild-type territory (no EPS, low carbon, some toxin)
```

**Core-periphery differentiation emerges from diffusion alone:**

1. **Periphery cells (EPS border):** Have moderate EPS. Carbon still diffuses in from outside. Toxin partially blocked. These are the fastest-growing cells. They expand the colony outward.

2. **Interior cells (EPS core):** Dense EPS surrounds them. Carbon is consumed by periphery cells before reaching the interior. Waste products accumulate (trapped by EPS). These cells are carbon-starved but toxin-protected. They enter quiescence or slow growth.

3. **Substrate-attached base (z=199):** Oldest EPS deposits (highest structural concentration). Near-impermeable to diffusion. Cells here may die from carbon starvation, leaving behind their EPS deposits -- creating **dead zones** with persistent matrix, exactly as observed in real biofilms (Flemming & Wingender, 2010).

**This IS emergent biofilm architecture.** No spatial programming created the core-periphery structure. It arises from three interacting dynamics:
- EPS secretion modifying local diffusion
- Nutrient consumption creating gradients
- Cell growth concentrated at the nutrient-replete periphery

### Phase 5: Cheater Invasion and Social Dynamics (Ticks 2000-5000)

**The EPS matrix is a public good.** A cell within the EPS zone benefits from reduced toxin diffusion and metabolite retention whether or not it produces EPS itself.

**Cheater emergence:**
- A mutation sets Reaction 6 v_max back to 0 or Transporter 3 secrete_rate to 0
- The cheater saves the carbon cost of EPS production (~0.015/tick)
- It benefits from the surrounding EPS matrix (produced by neighbors)
- Cheater divides faster than producer in the same EPS environment

**But cheaters erode the commons:**
- As cheater frequency increases in a region, local EPS production drops
- Structural species decays slowly (lambda_structural=0.005, half-life ~140 ticks)
- Without replenishment, the EPS barrier weakens over ~200-300 ticks
- Carbon leaks out, toxin leaks in
- Both cheaters and producers suffer

**Spatial dynamics protect cooperators (Nadell et al., 2016):**
- Producer daughters are placed adjacent to producer parents
- This creates producer-rich clusters that maintain local EPS
- Cheaters thrive at the boundary between producer clusters and the exterior
- But cheaters cannot sustain themselves in isolation (no EPS = no protection = toxin exposure)

**Expected equilibrium:**
- Producer-dominated core with persistent EPS matrix
- Cheater frequency highest at colony periphery where EPS benefits are maximal but contribution is not needed (fresh carbon available)
- Overall cheater frequency: 20-40% (stable mixed equilibrium from frequency-dependent selection)

### Phase 6: Advanced Evolutionary Innovations (Ticks 5000+)

Given sufficient evolutionary time, the following innovations may emerge:

1. **EPS-dependent metabolism:** A cell evolves to consume the structural species as a carbon source. This is an EPS-degrader -- it breaks down the matrix. In MARL, this would be a reaction: `structural-precursor(4) -> energy(0)` after uptaking structural from the field. This creates a third strategy: the degrader, which destroys the public good for private benefit.

2. **Waste cross-feeding:** Interior cells accumulate waste. A mutant evolves to metabolize waste-product as an energy source (Reaction: `waste(3) -> energy(0)`). This creates syntrophic coupling: periphery cells produce waste, interior cells consume it. This mirrors the acetate cross-feeding observed in E. coli biofilms.

3. **Signal-linked EPS production:** A cell evolves to produce signal-A at high density and gate EPS production on signal-A concentration. This creates QS-regulated biofilm formation -- merging the QS scenario into the niche construction scenario. EPS production activates only when a quorum is reached, avoiding costly EPS production by isolated cells.

4. **Vertical specialization:** If the biofilm thickens to 5-10 layers, cells at different depths experience different light/carbon/waste/toxin profiles. Vertical metabolic zonation (the Winogradsky dynamic) can emerge within the biofilm -- merging all three validation scenarios.

---

## 7. Does the EPS Barrier Create a Protected Interior Niche?

**Yes, through three mechanisms:**

### 7.1 Toxin Exclusion

With alpha=0.8 and structural=3.0 at the biofilm core:
```
D_toxin_local = D_base * (1 - 0.8 * 3.0/4.0) = D_base * 0.4

Toxin steady-state at center of EPS zone vs. outside:
  Outside (no EPS): toxin ~ 0.1 (from boundary diffusion + decay)
  Inside EPS core:  toxin ~ 0.04 (diffusion reduced by 60%)

Toxin damage at center: 2.0 * 0.04/(0.01+0.04) = 1.6/tick
Toxin damage outside:   2.0 * 0.1/(0.01+0.1)   = 1.82/tick

Modest but real protection. Compounds over the biofilm's lifetime.
```

### 7.2 Metabolite Retention

Carbon entering the EPS zone from above diffuses more slowly through the matrix:
```
Without EPS: carbon at z=197 ~ 0.025 (gradient from z=0)
With EPS (D reduced 60%): carbon accumulates at the EPS boundary
  EPS acts as a "sponge" -- carbon enters faster than it leaves
  Local carbon at EPS boundary: ~0.04-0.06 (60-140% increase)
```

But this benefit is concentrated at the **periphery** of the EPS zone, not the interior. Interior cells have their carbon consumed by periphery cells. This creates the core-periphery gradient.

### 7.3 Ecological Inheritance

When an EPS-producing cell dies (energy < 0.05), its body is removed but the structural species it secreted persists in the field:
```
structural(t+1) = structural(t) * (1 - lambda_structural)
                = structural(t) * 0.995

Half-life: ln(2)/0.005 = 139 ticks
```

A daughter cell or colonizer arriving at this voxel inherits a pre-built EPS environment. The dead cell's investment in niche construction benefits its descendants. This is textbook ecological inheritance (Odling-Smee et al., 2003) -- and it emerges without being programmed.

---

## 8. Does Emergent Spatial Structure Go Beyond Simple Vertical Zonation?

**Yes. The biofilm scenario produces at least four spatial patterns not present in the Winogradsky column:**

### 8.1 Lateral Core-Periphery Structure

The Winogradsky column produces vertical zonation (driven by light and chemical gradients along the z-axis). The biofilm scenario produces **lateral** structure: a core-periphery pattern in the xy-plane driven by EPS accumulation and nutrient depletion from the colony edge inward.

### 8.2 Fractal Colony Boundaries

At the colony edge, cells in nutrient-rich zones divide faster than cells in nutrient-poor zones. With diffusion-limited growth (carbon must diffuse to the growth front), the colony boundary develops fingering instabilities -- a well-known pattern in biofilm morphology (fractal DLA-like growth at low nutrient/low diffusion). MARL's EPS mechanism makes this more complex: EPS at the colony edge slows outward carbon diffusion, potentially stabilizing or destabilizing the growth front depending on the EPS production rate relative to the carbon diffusion rate.

### 8.3 Dead Core / Living Rim

Interior cells that exhaust local carbon die. Their EPS persists. The result is a spatial structure with:
- Living, actively growing cells at the periphery (nutrient access)
- Dead/quiescent cells in the core (nutrient-depleted)
- Persistent EPS matrix throughout (slow decay)

This "dead core / living rim" pattern is characteristic of real thick biofilms. In MARL, it emerges from the interaction of EPS diffusion barriers, nutrient depletion, and maintenance energy requirements (lambda_maintenance=0.02 ensures unfed cells die within ~50 ticks).

### 8.4 Water Channels

An intriguing prediction: if EPS is costly and cheaters evolve, cheater-dominated corridors within the biofilm would have reduced EPS and therefore higher local diffusion. These corridors could function as **water channels** -- higher-diffusion pathways through the EPS matrix. Real biofilms contain such channels (Flemming & Wingender, 2010). In MARL, they would emerge from the spatial interplay between EPS producers and cheaters.

---

## 9. Failure Modes

### 9.1 EPS Never Evolves (No Selective Advantage)

**Risk:** If the metabolic cost of EPS exceeds the diffusion-trapping benefit, no lineage sustains EPS production.

**Likelihood:** Low. The analysis in Phase 3 shows that EPS clusters with structural > 1.0 create significant carbon trapping (40%+ diffusion reduction). The 50-tick delay before benefits materialize means that EPS producers must survive the initial cost period. With light providing baseline energy, cells do not starve from the carbon diversion alone.

**Mitigation if needed:** Reduce the cost by lowering Reaction 5's v_max (less carbon diverted per tick) or increasing alpha (more diffusion reduction per unit EPS, faster payoff).

### 9.2 EPS Dominates Too Quickly (No Interesting Dynamics)

**Risk:** If EPS is overwhelmingly beneficial, it sweeps to fixation within 100 ticks and no producer-cheater dynamics develop.

**Likelihood:** Moderate. EPS requires two independent mutations and has a delayed payoff. But once an EPS cluster reaches critical mass, its growth advantage may be overwhelming.

**Mitigation:** Increase the cost of EPS production (higher Reaction 5 v_max = more carbon diverted) or decrease the benefit (lower alpha = less diffusion reduction). Tune so that EPS advantage is real but not decisive.

### 9.3 Waste Accumulation Kills the Biofilm

**Risk:** Trapped waste products (from Reaction 3) poison the biofilm interior. If waste becomes toxic via an evolved Reaction, the entire EPS zone could collapse.

**Likelihood:** Low in v1 (waste has no toxicity mechanism in the starter ruleset). Could emerge through topology mutations adding a waste-catalyzes-energy-destruction reaction (analogous to toxin damage). This would actually be an interesting evolved dynamic.

**Mitigation:** None needed. This is a feature -- waste accumulation limits biofilm thickness, creating realistic density-dependent constraints.

### 9.4 Carbon Never Reaches z=199

**Risk:** If carbon-source decay is too fast or the grid is too deep, carbon concentration at z=199 is effectively zero and all cells are light-dependent only.

**Impact:** Reduces the benefit of EPS (nothing to trap if carbon is zero). Cells survive on light but cannot grow carbon-based metabolism.

**Check:** At z=199, carbon = 1.0 * exp(-199/50) = 0.019. This is low but nonzero. With D_eff calibrated for day-scale diffusion and lambda_carbon ~ 0.01, the steady-state carbon at z=199 is:
```
carbon_ss = source * D / (D + lambda * L^2/2) where L = distance from source
```
This is non-trivial to compute analytically. The key is that carbon > 0 at the bottom. If it is too low, the scenario can be adjusted by:
- Reducing grid height (use 100 z-levels instead of 200)
- Reducing carbon decay rate
- Adding a bottom carbon source (sediment organics)

### 9.5 Two-Mutation Barrier Too High

**Risk:** EPS requires two mutations in the same lineage. If lineages are short-lived, the second mutation may never accumulate.

**Likelihood:** Low. Mutation 2 (transporter activation) is common (rate ~0.006 per reproduction). It drifts neutrally and accumulates in the population. By tick 100, ~50% of cells carry it. Mutation 1 (Reaction 6 activation) then only needs to occur once in a carrier.

**Alternative:** Set Transporter 3 secrete_rate to a small positive value (0.01) in the starter ruleset. This reduces the barrier to a single mutation (Reaction 6 activation) but makes EPS evolution slightly less interesting narratively.

---

## 10. Quantitative Predictions for Validation

When MARL is implemented, this scenario provides testable predictions:

1. **EPS production evolves within 200-500 ticks** from the homogeneous starting population (given mutation rates specified above).

2. **Structural species concentration should show sharp spatial boundaries** at the EPS colony edge, with a transition width scaling with `sqrt(D_structural / lambda_structural)`.

3. **Carbon concentration inside the EPS zone should exceed carbon outside** at the same z-level by 40-100% once structural > 1.0 (metabolite trapping).

4. **Toxin concentration inside the EPS zone should be lower than outside** by 30-60% (diffusion exclusion).

5. **Colony growth rate should be higher in EPS-producing clusters** than non-producing populations at the same depth (nutrient trapping + toxin exclusion > EPS production cost, once structural > ~1.0).

6. **Core-periphery structure should emerge by tick 1000:** high growth rate at periphery, quiescence/death in interior, measured by cell division rate as a function of distance from colony edge.

7. **Cheater frequency should increase to 20-40% and stabilize** (frequency-dependent selection). Pure cheater populations should go extinct faster than mixed populations.

8. **Dead cell fraction in biofilm interior should exceed 50% by tick 2000** (carbon starvation + maintenance decay).

9. **Ecological inheritance test:** Remove all living cells at tick 1000, re-seed 100 cells into the EPS-rich environment. These cells should grow faster than 100 cells seeded into a clean environment (same initial conditions minus the EPS). Growth rate difference quantifies the ecological inheritance effect.

10. **Ablation test (niche construction off):** Run the same scenario with alpha=0 (no diffusion modification). Population should be smaller, less spatially structured, and have lower metabolic diversity. The difference demonstrates niche construction's evolutionary effect.

---

## 11. Comparison to Other Validation Scenarios

| Feature | Winogradsky | Quorum Sensing | Niche Construction |
|---|---|---|---|
| **Driving force** | Light + chemical gradients (vertical) | Cell density + diffusion (horizontal) | EPS + diffusion modification (3D) |
| **Starting population** | 3 distinct metabolisms | 2 phenotypes (producer + cheater) | 1 homogeneous population |
| **Key emergent structure** | Vertical zonation | Clustered cooperation | Core-periphery biofilm |
| **Selection pressure** | Chemical self-consistency | Social evolution | Niche construction feedback |
| **Phenotype differentiation** | Seeded | Seeded | Must evolve de novo |
| **Key B+E feature tested** | Catalytic dependency chains | Autocatalytic signal amplification | EPS pathway + D_local modification |
| **Architecture modifications** | Epsilon background rate | None | None (D_local already specified) |
| **Number of mutations required** | 0 (seeded) | 0 (seeded) | 2 (EPS pathway activation) |
| **Paper contribution** | Emergent chemical ecology | Emergent social behavior | Emergent niche construction |
| **Triptych role** | Ecological pillar | Social pillar | Constructive pillar |

**Together, the three scenarios demonstrate that a single evolvable substrate -- the B+E hybrid with D_local modification -- produces ecological, social, AND constructive evolutionary dynamics.** This is the core argument of the paper.

---

## 12. Connection to Niche Construction Theory

This scenario is a direct computational test of Odling-Smee, Laland, and Feldman's (2003) niche construction framework:

### 12.1 The Four Conditions for Niche Construction (Odling-Smee et al., 2003)

1. **Organisms modify their environment** -- EPS-producing cells modify local diffusion coefficients. Satisfied.
2. **Environmental modification changes selection pressures** -- Reduced diffusion traps carbon (benefit) and waste (cost), alters toxin exposure. Satisfied.
3. **Modified selection pressures feed back on the constructors and their descendants** -- EPS producers and their daughters (who inherit the EPS environment) experience different selection than non-producers. Satisfied.
4. **Ecological inheritance transmits the constructed niche to offspring** -- Dead cells leave EPS deposits. Daughter cells placed adjacent to parents inherit the EPS environment. Satisfied.

### 12.2 Comparison to Taylor (2004) ALife Niche Construction

Taylor (2004, ALIFE IX) showed that niche construction in ALife simulations drives complexity increases. MARL extends this result in two ways:

1. **Chemical grounding.** Taylor's niche construction modified abstract environmental parameters. MARL's niche construction modifies diffusion coefficients -- a physical, measurable quantity with known biological analogs (Stewart, 2003).

2. **Continuous, graduated effect.** Taylor's construction was binary (modified or not). MARL's EPS effect is continuous (proportional to structural concentration), creating smooth fitness landscapes rather than step functions.

### 12.3 Novel Contribution

No prior ALife system demonstrates all of the following simultaneously:
- Evolved niche construction (not programmed)
- Continuous diffusion modification (not binary)
- Ecological inheritance via persistent environmental modification
- Public goods dynamics (EPS as shared resource)
- Core-periphery spatial structure emergence
- Interaction with other evolved behaviors (potential QS + niche construction coupling)

This makes the niche construction scenario a genuinely novel contribution to the ALife literature.

---

## 13. References

- Beroz, F. et al. (2018). Verticalization of bacterial biofilms. Nature Physics, 14, 954-960.
- Flemming, H.-C. & Wingender, J. (2010). The biofilm matrix. Nature Rev. Microbiol., 8, 623-633.
- Hartmann, R. et al. (2019). Emergence of three-dimensional order and structure in growing biofilms. Nature Physics, 15, 251-256.
- Nadell, C.D., Drescher, K., & Foster, K.R. (2016). Spatial structure, cooperation and competition in biofilms. Nature Rev. Microbiol., 14, 589-600.
- Odling-Smee, F.J., Laland, K.N., & Feldman, M.W. (2003). Niche Construction: The Neglected Process in Evolution. Princeton University Press.
- Stewart, P.S. (2003). Diffusion in biofilms. J. Bacteriol., 185(5), 1485-1491.
- Taylor, T. (2004). Niche construction and the evolution of complexity. ALIFE IX Proceedings.
- Yan, J. et al. (2019). Extracellular-matrix-mediated osmotic pressure drives Vibrio cholerae biofilm expansion and cheater exclusion. Nature Communications, 8, 327.
- Xavier, J.B. & Foster, K.R. (2007). Cooperation and conflict in microbial biofilms. PNAS, 104(3), 876-881.
- Drescher, K. et al. (2014). Solutions to the public goods dilemma in bacterial biofilms. Current Biology, 24(1), 50-55.
