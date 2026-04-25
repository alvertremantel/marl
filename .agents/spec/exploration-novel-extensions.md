# Exploration: Novel Extensions for MARL

**Created:** 2026-03-15 (Iteration 004)
**Purpose:** Deep analysis of the most promising novel directions that could make MARL a publication-worthy contribution, not just a simulator. Each extension is evaluated for biological plausibility, implementation feasibility, evolutionary potential, and paper impact.

---

## Overview

MARL's core spec establishes a solid foundation: reaction-diffusion field, sparse evolved agents, field-mediated interaction, HGT, Beer-Lambert light. But a publication needs to demonstrate that this foundation enables dynamics not achievable in prior systems. The extensions below are the strongest candidates for that demonstration.

---

## 1. Niche Construction: Cells That Modify Local Diffusion Coefficients

### 1.1 Concept

In real biofilms, cells secrete extracellular polymeric substances (EPS) that fundamentally alter their local environment. EPS creates a gel matrix that reduces diffusion coefficients by 20-80% compared to free water (Stewart, 2003). This is textbook niche construction: organisms modifying their environment in ways that change selection pressures on themselves and their neighbors.

In MARL, this could be implemented by allowing cells to secrete a "structural" species (already reserved as external species 7 in the Winogradsky scenario) that modifies the local diffusion coefficient. Where structural-deposit concentration is high, diffusion slows. This creates a positive feedback loop: cells that secrete EPS slow down diffusion of nutrients near them, potentially trapping locally produced metabolites and creating concentration hotspots.

### 1.2 Biological Grounding

Measured De/Daq ratios in real biofilms (Stewart, 2003; PMC148055):
- Light gases (O2, CO2, CH4): ~0.6
- Most organic solutes: ~0.25
- Large molecules (proteins): ~0.05-0.15

EPS composition varies enormously between species and conditions. Pseudomonas biofilms produce multiple matrix molecules including polysaccharides, nucleic acids, and proteins (Flemming & Wingender, 2010). The matrix is not just a passive barrier -- it actively structures the local chemical environment, creating micro-niches with distinct pH, oxygen, and nutrient profiles.

Real biofilm diffusion is heterogeneous and anisotropic. Recent work (arxiv 2408.07626, 2024) shows that anisotropic diffusion models better capture biofilm communication patterns than isotropic ones. Time-varying channel models (arxiv 2511.03856, 2025) show that biofilm maturation creates water channels that modify local transport.

### 1.3 Implementation in MARL

**Minimal implementation (recommended for v1):**

```
// In field_update, modify diffusion coefficient per voxel:
D_local[x,y,z] = D_base * (1.0 - alpha * structural[x,y,z] / (K_eps + structural[x,y,z]))

where:
  D_base    = species-specific base diffusion coefficient
  alpha     = maximum diffusion reduction factor (e.g., 0.8 = 80% reduction)
  structural = concentration of structural-deposit species at (x,y,z)
  K_eps     = half-saturation for EPS effect (controls how much EPS is needed)
```

This is a single multiplication added to the field update shader. The `structural` species already exists in the field; we just read it and use it to modulate D.

**VRAM cost:** Zero additional VRAM. The structural species is already allocated. The D_local computation uses existing buffers.

**Compute cost:** One extra multiply-add per voxel per species per tick in the field update pass. At 50M voxels x 12 species, this is 600M extra operations -- trivial vs. the memory bandwidth cost.

**Evolutionary pathway:** Cells evolve to secrete the structural species via their effector layer (already supported). Cells in EPS-rich regions retain their secreted metabolites (slower diffusion = less dilution), which benefits cells that produce useful metabolites and harms cells that rely on importing metabolites from far away. This creates selection pressure for biofilm formation WITHOUT hard-coding any biofilm logic.

### 1.4 Emergent Dynamics

Niche construction via diffusion modification could produce:
- **Self-organized biofilm structure.** Cells at the colony edge experience high diffusion (no EPS) and fast growth. Interior cells experience low diffusion and slower growth but retain metabolites. This creates a natural core-periphery structure.
- **Chemical isolation.** EPS-producing lineages could create "walls" that trap their metabolites, outcompeting non-producers who share their metabolites freely. This is a public goods game -- a classic evolutionary dynamic.
- **Diffusion barriers between zones.** In the Winogradsky column, an EPS-producing layer between the oxic and anoxic zones could sharpen the oxidant gradient, strengthening the ecological separation.
- **Niche inheritance.** Dead cells leave behind their EPS deposits (structural species decays slowly). Daughter cells inherit the modified environment. This is ecological inheritance in the niche construction sense (Odling-Smee et al., 2003).

### 1.5 Paper Impact

**High.** Niche construction is a hot topic in evolutionary biology. No existing ALife system combines niche construction with reaction-diffusion field dynamics at this scale. The ability to show that evolved EPS production creates emergent biofilm structure would be a novel result. Combined with the Winogradsky column scenario, this demonstrates multi-level self-organization (chemical zonation + biofilm architecture) from a single substrate.

**Key citation:** Odling-Smee, F.J., Laland, K.N., & Feldman, M.W. (2003). Niche Construction: The Neglected Process in Evolution. Princeton University Press. Taylor (2004) showed niche construction drives complexity increases in ALife simulations.

### 1.6 Risk Assessment

**Low risk.** This extension requires:
- One extra line in the field update shader (modulate D by structural concentration)
- No new data structures
- No changes to cell update logic (cells already can secrete structural species)
- The structural species already exists in the species namespace

The only design question is whether diffusion reduction should be per-species (some chemicals diffuse through EPS better than others) or uniform. Per-species is more realistic but requires S additional parameters. Recommendation: start with uniform reduction, extend to per-species if needed.

---

## 2. Quorum Sensing Abstractions

### 2.1 Concept

Quorum sensing (QS) is a mechanism by which bacteria sense their local population density through diffusible signal molecules called autoinducers. When autoinducer concentration exceeds a threshold, gene expression changes -- typically activating behaviors that are only useful at high cell density (biofilm formation, toxin production, bioluminescence).

MARL already supports quorum sensing naturally through its existing architecture. Here is why: signal species (external slots 5-6 in the Winogradsky scenario) are produced by cells, diffuse through the field, and are sensed by receptors (Layer 1 of the B+E hybrid). A cell whose receptor for signal-A has a high gain and appropriate k_half will activate its intracellular network differently when signal-A concentration is high (many nearby cells secreting it) vs. low (few neighbors).

The question is not "can MARL do quorum sensing?" but "what additional mechanisms would make quorum sensing dynamics richer and more evolvable?"

### 2.2 What MARL Already Supports

The existing B+E hybrid supports basic QS:

```
Quorum sensing circuit (existing mechanisms):
1. Cell secretes signal-A (effector layer, rate proportional to some internal state)
2. Signal-A diffuses through field (field update pass)
3. Neighboring cells sense signal-A (receptor layer, Hill function)
4. High signal-A activation triggers different intracellular reaction rates
   (e.g., signal-A receptor activation modulates which reactions fire)
5. Response: changed secretion pattern, changed metabolism, changed fate decision
```

This is already a complete QS circuit. The threshold behavior comes from the Hill function with cooperativity n > 1 (steep sigmoid). The density-dependence comes from field diffusion -- more cells secreting = higher local concentration.

### 2.3 What's Missing: Signal Degradation and Interference

In real QS systems, signal molecules are not just produced and sensed -- they are also actively degraded by enzymes called quorum quenching enzymes. Some bacteria produce these enzymes to disrupt neighbors' QS circuits (a form of chemical warfare).

MARL's current design supports signal degradation passively (the field decay term lambda reduces all species concentrations over time). But it does not support ACTIVE degradation -- a cell that consumes or destroys signal molecules to jam its neighbors' communication.

**Proposed extension: Signal consumption as a metabolic reaction.**

This requires no new mechanism. A cell can evolve a reaction rule:

```
Reaction: substrate=signal-A(int), product=energy(int), catalyst=enzyme-X(int)
Transport: uptake signal-A(ext) -> signal-A(int), high uptake_rate
```

This cell actively imports signal-A from the field and converts it to energy. The effect: it depletes signal-A in its local neighborhood, preventing nearby cells from reaching quorum. This is quorum quenching, and it emerges from existing MARL mechanisms without any extension.

The fact that MARL already supports this without modification is a strength to highlight in the paper.

### 2.4 What Would Be Novel: Multi-Signal Crosstalk

Real bacteria use multiple QS circuits simultaneously (e.g., P. aeruginosa has at least 3 distinct QS systems: Las, Rhl, and PQS). These circuits interact -- one can activate or repress another.

With S=12 (proposed in ADR-006), MARL could support 3-4 distinct signal species. The receptor and reaction layers already support multi-signal integration: a cell's intracellular reaction network can have reactions that depend on multiple signal-derived internal species.

The key insight is that multi-signal QS does NOT require any new mechanisms in MARL. It requires only sufficient species slots (addressed by ADR-006) and evolutionary time for cells to discover multi-signal strategies.

### 2.5 Paper Impact

**Medium.** QS in computational models is well-studied (Muller Vasconcelos, 2021; Frederick et al., 2011; Anguige et al., 2004). MARL's contribution would be showing that QS emerges from evolution rather than being programmed. The novel angle is QS evolving alongside metabolism and niche construction in the same substrate -- something no existing model achieves.

### 2.6 Implementation Requirements

**None for basic QS.** Already supported.

For richer QS dynamics, the main requirement is more species slots (ADR-006: S=12 provides 3+ signal slots). No code changes needed beyond the species count increase.

---

## 3. Mechanical Pressure from Crowding

### 3.1 Concept

In real biofilms, growing cell populations generate mechanical pressure. When cells divide and adjacent voxels are occupied, physical forces push cells outward. This causes biofilm expansion, buckling instabilities, and vertical growth (verticalization). Recent agent-based modeling work (Beroz et al., 2018; Hartmann et al., 2019) showed that localized mechanical instabilities drive the transition from flat monolayer biofilms to 3D structures.

MARL currently handles crowding with a simple rule: when a cell divides, it checks for an adjacent empty voxel. If none is available, division fails (the cell retains its energy). This is a hard occupancy constraint -- cells cannot push each other.

### 3.2 Design Options

**Option 1: Shoving Model (Agent-Based)**

When a cell divides with no adjacent empty voxel, it "shoves" a chain of cells outward to create space. The cell at the end of the chain is pushed into an empty voxel. This is the standard approach in iDynoMiCS and similar biofilm models.

- **Pros:** Physical, produces realistic biofilm morphology.
- **Cons:** Non-local operation (shove chains can be long). Hostile to GPU parallelism -- multiple cells may try to shove simultaneously in conflicting directions. Requires conflict resolution (sequential processing or probabilistic resolution).
- **Risk:** High. This introduces complex spatial logic that is hard to parallelize.

**Option 2: Pressure Field (Continuum)**

Add a "pressure" or "cell-density" field that diffuses and exerts a force on cells. When a voxel and its neighbors are occupied, pressure increases. Cells in high-pressure regions have increased death rate or reduced division rate. Cells at the colony edge (low pressure) divide freely.

```
pressure[x,y,z] = sum of cell_density in 26-neighborhood
cell.death_probability += pressure_sensitivity * pressure[cell.pos]
```

- **Pros:** GPU-friendly (just another field pass). Smooth, differentiable. Produces density-dependent growth naturally.
- **Cons:** Not physically accurate (pressure doesn't diffuse like a chemical). Doesn't produce mechanical buckling or verticalization.
- **Risk:** Low. This is a simple field computation with no architectural changes.

**Option 3: Effective Carrying Capacity (Simplest)**

Each voxel has a maximum cell occupancy of 1 (current design) or a small number (e.g., 2-4, representing multi-cell "packets"). When occupancy is at capacity, division fails. This is the current MARL behavior.

- **Pros:** Simplest possible model. Already implemented.
- **Cons:** No pressure dynamics. No colony expansion beyond random vacancies.
- **Risk:** None.

### 3.3 Recommendation

**Option 2 (Pressure Field) for v1, with Option 3 as baseline.**

The pressure field approach is the best balance of realism and GPU-friendliness. It can be implemented as a derived field computed from the cell registry:

1. After cell update, compute cell_density field (1 where cell exists, 0 elsewhere)
2. Smooth cell_density with a 3D Gaussian blur (or just read the 6-neighbor sum)
3. Pass smoothed density to the cell update shader as "pressure"
4. Cells read local pressure and adjust fate thresholds: `effective_division_energy = base_division_energy + pressure * pressure_sensitivity`

This makes division harder in crowded regions and easier at colony edges, producing natural colony expansion without explicit shoving mechanics.

### 3.4 Evolutionary Implications

Pressure sensitivity can be made an evolvable parameter in the fate layer:

```
struct FateParams {
    division_energy:     float16,
    death_energy:        float16,
    quiescence_energy:   float16,
    pressure_sensitivity: float16,  // NEW: how much crowding affects division threshold
}
```

Cells that evolve low pressure sensitivity divide even in crowded conditions (aggressive colonizers). Cells that evolve high pressure sensitivity only divide when there is space (conservative growers). This creates a tradeoff: aggressive cells expand fast but may exhaust local resources; conservative cells grow slowly but maintain sustainable populations.

### 3.5 Paper Impact

**Medium.** Mechanical effects in biofilms are well-studied in agent-based models. MARL's contribution would be the evolutionary dimension: pressure sensitivity evolving alongside metabolism and niche construction. The combination is novel.

### 3.6 VRAM and Compute Cost

- Cell density field: 50M x 2 bytes = 100 MB (or reuse existing field buffer by adding cell_density as a derived species)
- Gaussian blur: one extra 3D stencil pass = ~2-3 ms/tick
- Total cost: minimal

---

## 4. Information-Theoretic Measures for Open-Ended Evolution

### 4.1 Concept

Measuring whether MARL achieves open-ended evolution (OEE) is as important as achieving it. The existing OEE metrics document (exploration-oee-metrics.md) covers MODES and evolutionary activity statistics. This section extends that analysis with information-theoretic measures that are more rigorous and more publishable.

### 4.2 Assembly Index (Cronin et al., 2023)

Assembly theory, published in Nature (2023), defines molecular complexity via the **assembly index**: the minimum number of joining operations needed to construct a molecule from basic building blocks. The **assembly** of a system combines this index with the copy number (abundance) of each molecule type: high assembly = complex molecules present in many copies = evidence of selection.

**MARL analog:** Define the "assembly index" of a ruleset as the minimum number of mutation/HGT steps needed to construct it from a null ruleset (all v_max=0, all species indices=0):

```
assembly_index(ruleset) = min edit distance from null ruleset to current ruleset

where edit operations are:
  - Set one v_max to a nonzero value (1 step)
  - Change one species index (substrate, product, or catalyst) (1 step)
  - Change one kinetic parameter (k_m, k_cat) (1 step)
  - Add one transporter mapping (1 step)
```

The assembly index of a population is:

```
A(population) = sum over unique ruleset topologies t:
                  assembly_index(t) * log(1 + copy_number(t))
```

High A(population) means the population contains complex rulesets that are abundant -- evidence that selection (not drift) produced them.

**Advantages over MODES:**
- Grounded in physical theory (not ad hoc)
- Published in Nature (strong citation)
- Computable from ruleset snapshots (no time series needed)
- Naturally separates drift (high complexity, low copies) from selection (high complexity, high copies)

**Implementation cost:** Moderate. Requires computing edit distance from null for each unique ruleset topology. This is a CPU-side analysis, not real-time. Can be computed per-epoch (every 100-1000 ticks).

### 4.3 Predictive Information / Excess Entropy

Predictive information (Bialek et al., 2001; Grassberger, 1986) measures the mutual information between the past and future of a time series. It quantifies how much of the future is predictable from the past -- a measure of structured complexity.

**MARL application:** For each cell lineage, construct a time series of "metabolic state" (e.g., which reactions are active, what the internal concentration profile looks like). The predictive information of this time series measures how much a cell's future metabolic state depends on its past. High predictive information = complex, structured metabolism. Low = random or simple.

```
PI(lineage) = I(X_past ; X_future)
            = H(X_future) - H(X_future | X_past)
```

Where X_t is the metabolic state at tick t, discretized into bins.

**Population-level metric:** Mean PI across all living cells at a given tick. If mean PI increases over evolutionary time, the population is evolving more complex metabolic regulation.

**Complementary to assembly index:** Assembly index measures structural complexity of the genome. Predictive information measures dynamic complexity of the phenotype. Both should increase in a genuinely open-ended system.

### 4.4 Transfer Entropy Between Ecological Partners

Transfer entropy (Schreiber, 2000) measures directed information flow between two time series:

```
TE(X -> Y) = I(Y_future ; X_past | Y_past)
```

This measures how much knowing X's past helps predict Y's future, beyond what Y's own past provides.

**MARL application:** Compute transfer entropy between:
- Cell lineage A's secretion pattern and cell lineage B's growth rate
- External chemical concentrations and cell population dynamics
- QS signal levels and metabolic state changes

High TE between species indicates genuine ecological coupling (not just correlation). This could demonstrate that MARL's cells evolve to causally influence each other through the chemical field.

**Paper value:** Transfer entropy as evidence of evolved ecological interaction is a novel analysis. Prior ALife papers have used it for agent-agent interaction in 2D worlds (Beer & Williams, 2015), but not in a reaction-diffusion context.

### 4.5 Entropy Reduction Rate (ERR)

Recent theoretical work (Frontiers in Complex Systems, 2025) proposes that evolution is fundamentally driven by informational entropy reduction. Living systems reduce internal uncertainty by extracting meaningful information from the environment.

**MARL application:** Measure the entropy of each cell's internal concentration vector over time:

```
H_internal(cell, t) = -sum_i p_i log(p_i)

where p_i = internal[i] / sum(internal)
```

If a cell's internal entropy decreases over its lifetime (concentrations become more structured, less uniform), it is organizing its internal chemistry. If the population's mean internal entropy decreases over evolutionary time, evolution is driving internal organization.

**Complementary to other metrics:** ERR measures moment-to-moment metabolic organization. Assembly index measures genetic complexity. Predictive information measures temporal structure. Together, they provide a multi-scale view of complexity.

### 4.6 Recommended Metric Suite for Publication

For a paper on MARL's open-ended evolution, I recommend this suite of 6 metrics:

| Metric | What it Measures | Level | Compute Cost |
|--------|-----------------|-------|--------------|
| MODES (4 hallmarks) | Change, novelty, complexity, ecology | Population | Low |
| Evolutionary activity | Novel genotype persistence | Population | Low |
| Assembly index | Genetic structural complexity | Individual/Population | Moderate |
| Predictive information | Metabolic dynamic complexity | Individual/Lineage | High |
| Transfer entropy | Ecological causal coupling | Pairs/Ecosystem | High |
| Entropy reduction rate | Internal metabolic organization | Individual | Low |

The first two are standard OEE metrics (already in exploration-oee-metrics.md). The last four are novel contributions that leverage MARL's chemical substrate for information-theoretic analysis not possible in traditional ALife systems.

### 4.7 Paper Impact

**Very high.** Information-theoretic measures of OEE are an active research frontier. Most existing measures are generic (apply to any evolving system). MARL's chemical substrate enables DOMAIN-SPECIFIC information measures that capture metabolic complexity, ecological coupling, and niche construction effects. This combination has not been published.

The assembly index connection to Cronin et al. (2023) ties MARL to a high-profile physical chemistry result, broadening the paper's appeal beyond the ALife community.

---

## 5. Multi-Scale Temporal Dynamics

### 5.1 Concept

Real cellular biology operates on multiple timescales:
- **Intracellular reactions:** seconds to minutes (enzyme kinetics, signaling cascades)
- **Cell division:** hours to days
- **Diffusion across tissue:** hours to days
- **Evolutionary change:** weeks to years

MARL currently collapses all of these into a single tick (1 day). This is appropriate for evolutionary dynamics but may miss fast intracellular dynamics that affect cell behavior within a single tick.

### 5.2 Sub-Tick Intracellular Integration

The cell update pass currently runs the intracellular ODE with a single Forward Euler step per tick. If reactions have widely varying rates (some v_max = 2.0, others = 0.01), this single step may miss fast dynamics or introduce instability.

**Proposed extension:** Allow N_sub sub-steps per tick for the intracellular ODE:

```
for sub in 0..N_sub:
    evaluate_reactions(cell, dt / N_sub)
```

With N_sub = 4-8, fast intracellular dynamics are resolved while the field update (which is the bandwidth bottleneck) still runs at 1 step/tick.

**Cost:** Increases cell update compute by N_sub factor. At 100K cells, this is still negligible vs. field update. At 1M cells with N_sub=8, cell update might become ~2-3 ms/tick.

### 5.3 Paper Impact

**Low.** Multi-scale integration is standard in computational biology (PhysiCell uses 3 separate timescales). Not a novel contribution by itself, but necessary for correctness.

### 5.4 Recommendation

Include N_sub as a simulation parameter (default = 1 for fast runs, 4-8 for accuracy). This is a one-line change to the cell update loop. Low risk, moderate benefit for numerical accuracy.

---

## 6. Phylogenetic Tree Reconstruction

### 6.1 Concept

MARL already tracks lineage_id for every cell. This data enables reconstruction of phylogenetic trees showing the evolutionary history of the population.

### 6.2 Implementation

At each reproduction event, record:
- Parent lineage_id
- Daughter lineage_id
- Tick number
- Parent and daughter voxel positions
- Whether HGT occurred (and from which donor lineage)
- Ruleset fingerprint (hash of reaction network topology)

This log enables post-hoc reconstruction of:
- Standard bifurcating phylogenetic trees (vertical inheritance)
- Reticulate phylogenetic networks (including HGT events)
- Spatial phylogeography (where lineages originated and migrated)

### 6.3 HGT Visualization

The combination of vertical inheritance and HGT creates a reticulate (network) phylogeny, not a simple tree. This is directly analogous to bacterial phylogenomics, where HGT makes species trees reticulate. Visualizing MARL's reticulate phylogeny would be a compelling figure for a paper.

### 6.4 Paper Impact

**High.** Phylogenetic visualization is standard in evolutionary biology papers. Showing a reticulate phylogeny where HGT events are visible, combined with metabolic innovation at each branching point, would be a powerful demonstration of MARL's evolutionary dynamics.

### 6.5 Implementation Cost

CPU-side logging only. No impact on simulation performance. The lineage_id is already tracked. The only addition is writing (parent_id, daughter_id, tick, hgt_donor_id, ruleset_hash) to a log file per reproduction event.

---

## 7. Priority Ranking and Implementation Order

Based on paper impact, implementation cost, and risk:

| Rank | Extension | Impact | Cost | Risk | When |
|------|-----------|--------|------|------|------|
| 1 | Niche construction (diffusion modification) | High | Very low | Low | v1 |
| 2 | Info-theoretic metrics (assembly index + ERR) | Very high | Moderate | None (analysis only) | v1 |
| 3 | Phylogenetic tree reconstruction | High | Very low | None (logging only) | v1 |
| 4 | Quorum sensing | Medium | None (already supported) | None | v1 (just demonstrate it) |
| 5 | Pressure field (crowding) | Medium | Low | Low | v1 or v2 |
| 6 | Predictive information + transfer entropy | Very high | High | None (analysis only) | v2 (needs long runs) |
| 7 | Multi-scale temporal dynamics | Low | Low | Low | v2 |

**v1 priorities:** Niche construction and info-theoretic metrics should be in the first version. They are low-cost and high-impact. Phylogenetic logging is free and should be on from the start. QS just needs to be demonstrated, not implemented (it is already supported).

**v2 priorities:** Predictive information and transfer entropy require long simulation runs and sophisticated offline analysis. They belong in a follow-up paper or as supplementary analysis.

---

## 8. The Paper Argument

With these extensions, MARL's publication narrative becomes:

**Title direction:** "Emergent Chemical Ecology and Niche Construction in a 3D Reaction-Diffusion Substrate with Evolvable Cellular Agents"

**Core argument:** MARL is the first system that combines:
1. 3D reaction-diffusion field as primary physics (not just a background)
2. Sparse cellular agents with evolvable catalytic reaction networks
3. Horizontal gene transfer of metabolic capabilities
4. Niche construction via field modification (diffusion coefficient modulation)
5. No fitness function, no predefined cell types

**Key results to demonstrate:**
1. Winogradsky column zonation emerges from chemistry alone
2. Niche construction (EPS secretion) produces emergent biofilm structure
3. Quorum sensing evolves without being programmed
4. Assembly index increases over evolutionary time (selection, not drift)
5. Transfer entropy reveals evolved ecological coupling between lineages
6. Reticulate phylogeny shows HGT driving metabolic innovation

**Whitespace claim:** No existing ALife system (Lenia, Avida, PhysiCell, Neural CA) combines all of these. Lenia has no chemistry. Avida has no spatial chemistry. PhysiCell has no evolution. Neural CA have no compartmentalization.

---

## 9. References

- Stewart, P.S. (2003). Diffusion in biofilms. J. Bacteriol., 185(5), 1485-1491.
- Flemming, H.-C. & Wingender, J. (2010). The biofilm matrix. Nature Rev. Microbiol., 8, 623-633.
- Odling-Smee, F.J., Laland, K.N., & Feldman, M.W. (2003). Niche Construction: The Neglected Process in Evolution. Princeton University Press.
- Taylor, T. (2004). Niche construction and the evolution of complexity. ALIFE IX Proceedings.
- Cronin, L. et al. (2023). Assembly theory explains and quantifies selection and evolution. Nature, 622, 244-249.
- Bialek, W., Nemenman, I., & Tishby, N. (2001). Predictability, complexity, and learning. Neural Computation, 13(11), 2409-2463.
- Schreiber, T. (2000). Measuring information transfer. Physical Review Letters, 85(2), 461.
- Grassberger, P. (1986). Toward a quantitative theory of self-generated complexity. Intl. J. Theor. Phys., 25(9), 907-938.
- Beroz, F. et al. (2018). Verticalization of bacterial biofilms. Nature Physics, 14, 954-960.
- Hartmann, R. et al. (2019). Emergence of three-dimensional order and structure in growing biofilms. Nature Physics, 15, 251-256.
- Adams, A. et al. (2019). Formal definitions of unbounded evolution and innovation reveal universal mechanisms for open-ended evolution in dynamical systems. Scientific Reports, 7, 997.
- Bedau, M. et al. (2000). Open problems in artificial life. Artificial Life, 6(4), 363-376.
- Pal, C. et al. (2005). Adaptive evolution of bacterial metabolic networks by horizontal gene transfer. Nature Genetics, 37(12), 1372-1375.
- Muller Vasconcelos, L. (2021). Bacterial quorum sensing as a networked decision system.
- Frederick, M.R. et al. (2011). A mathematical model of quorum sensing regulated EPS production in biofilm communities. Theoretical Biology and Medical Modelling, 8(1), 8.
- Ghaffarizadeh, A. et al. (2018). PhysiCell: An open source physics-based cell simulator for 3-D multicellular systems. PLoS Computational Biology, 14(2), e1005991.
