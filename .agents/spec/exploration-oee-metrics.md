# Exploration: Measuring Open-Ended Evolution in MARL

**Created:** 2026-03-15 (Iteration 001)
**Purpose:** Survey existing OEE metrics and propose how MARL should measure and demonstrate open-ended evolution for publication.

---

## 1. Why This Matters

MARL's core claim is that open-ended evolution emerges from chemical self-consistency without a fitness function. To publish this claim, we need quantitative evidence. The ALife community has developed specific frameworks for measuring open-endedness, and MARL should be designed from the start to produce the right observables.

---

## 2. The MODES Framework

The most widely-cited measurement framework is MODES (Measurements of Open-Ended Dynamics in Evolving Systems), proposed by Adams et al. (2019, Artificial Life 25(1)). MODES defines four hallmarks:

### 2.1 Change Potential
The system continues to produce new types/behaviors over time, not converging to a fixed state.
- **MARL observable:** Ruleset parameter entropy over time. Track the distribution of active reaction configurations across the population. If entropy plateaus, change has stopped.
- **Implementation:** Per-tick snapshot of ruleset diversity (e.g., mean pairwise Hamming distance on discretized reaction vectors).

### 2.2 Novelty Potential
New types are genuinely novel, not just recombinations of existing types.
- **MARL observable:** Track the set of all reaction network topologies ever observed. Plot cumulative unique topologies over time. If the curve keeps rising, novelty is ongoing.
- **Implementation:** Hash each cell's reaction network topology (ignoring kinetic parameters) to a fingerprint. Maintain a set of seen fingerprints.

### 2.3 Complexity Potential
The complexity of organisms increases over time (or at least doesn't decrease).
- **MARL observable:** Number of active reactions per cell (reactions with rate > threshold). Mean and maximum active-reaction count over time. Also: information-theoretic complexity of the ruleset (compressibility of the reaction parameter vector).
- **Implementation:** Track active_reaction_count per cell. Compute population statistics per epoch.

### 2.4 Ecological Potential
The system supports multiple coexisting types that interact and create ecological niches.
- **MARL observable:** Number of distinct "species" (clusters in ruleset parameter space) that coexist stably. Species richness and evenness over time. Spatial segregation of species (do they form distinct zones?).
- **Implementation:** K-means clustering on ruleset parameter vectors. Track cluster count, Gini coefficient of cluster sizes, spatial autocorrelation of cluster assignments.

---

## 3. Evolutionary Activity Statistics

Bedau et al. (2000) proposed evolutionary activity statistics, measuring:

- **Total activity:** Count of all new genotypes that persist for at least T ticks
- **Mean activity per genotype:** Average persistence time
- **Diversity:** Number of distinct active genotypes at any time

A system exhibits open-ended evolution if total activity grows without bound (new genotypes keep appearing and persisting).

**MARL implementation:** Track lineage_id and birth/death times. A genotype "persists" if any descendant with the same reaction topology is alive T ticks later. This directly leverages MARL's lineage tracking infrastructure.

---

## 4. MARL-Specific Metrics

Beyond standard OEE metrics, MARL's chemical substrate enables unique measurements:

### 4.0 Assembly Index (Now Fully Specified)

The assembly index metric described conceptually in [[exploration-novel-extensions]] Section 4.2 is now fully specified with concrete algorithms, pseudocode, cost analysis, and validation strategy in [[research-assembly-index]]. The algorithm defines ruleset assembly index as edit distance from a null ruleset, enriched by topology-aware features (catalytic dependency depth, autocatalytic cycle count). Population-level assembly follows Cronin et al. (2023): A(pop) = sum_t [ AI(t) * log2(1 + copy_number(t)) ].

### 4.1 Metabolic Diversity
- How many distinct metabolic strategies (sets of active reactions) coexist?
- Do cells in different spatial zones use different metabolisms? (Winogradsky column emergence)
- Is there metabolic complementarity (cell A produces what cell B consumes)?

### 4.2 Chemical Ecosystem Stability
- Does the system reach chemical steady state, or do chemical concentrations continue to fluctuate?
- Ongoing fluctuation suggests ongoing evolutionary dynamics; steady state suggests convergence.
- Track variance of field concentrations over sliding windows.

### 4.3 HGT Dynamics
- What is the population-level HGT rate over time?
- Do HGT-accepting vs. HGT-rejecting strategies coexist? (Analog of restriction-modification)
- Which reaction rules transfer most frequently? (Analog of mobile genetic elements)

### 4.4 Spatial Structure Emergence
- Do cells self-organize into spatial structures (layers, clusters, filaments)?
- Measure spatial autocorrelation of cell type assignments
- Track the number of distinct spatial zones (connected components of same-type cells)

---

## 5. Neutral Shadow Runs

Adams et al. recommend pairing each experimental run with a "neutral shadow" run where selection is disabled. This provides a baseline: any metric that increases in the real run but not the shadow run is evidence of adaptive evolution, not drift.

**MARL implementation:** Run a parallel simulation where cell fate decisions are randomized (death and division probabilities ignore energy/chemistry). Compare metrics between the two runs.

---

## 6. Publication Strategy

For a paper demonstrating MARL's open-ended evolution, the minimum viable evidence is:

1. **MODES metrics showing ongoing change, novelty, complexity, and ecological diversity** over at least 10,000 ticks (10,000 simulated days = ~27 simulated years)
2. **Winogradsky column emergence** — qualitative demonstration that vertical chemical zonation appears without explicit programming
3. **HGT dynamics** — demonstration that HGT propensity itself evolves, and that horizontally transferred genes confer adaptive benefit
4. **Comparison to neutral shadow run** — showing that metrics exceed neutral baseline

Nice-to-have evidence:
5. Phylogenetic tree reconstruction showing branching and diversification
6. Metabolic complementarity analysis (cell A feeds cell B feeds cell C)
7. Comparison of OEE metrics against Avida or Lenia benchmarks

---

## 7. Design Implications

To support these metrics, MARL needs:
- **Lineage tracking** (already in spec: lineage_id in CellState)
- **Periodic snapshots** of population statistics (ruleset diversity, spatial distribution)
- **Reaction topology fingerprinting** (hash of active reactions, ignoring parameters)
- **HDF5 or similar export** (mentioned in renderer.md as planned)
- **Neutral shadow run mode** (randomize fate decisions)

These are all observation-layer features that don't affect the core simulation loop.
