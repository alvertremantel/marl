# Research: Literature Landscape and Positioning

**Created:** 2026-03-15 (Iteration 001)
**Purpose:** Map the ALife, artificial chemistry, and agent-based modeling landscape to identify MARL's whitespace and relevant prior art.

---

## 1. Positioning Matrix

| System | Substrate | Evolution | Chemistry | Compartments | Spatial | Scale | MARL Relation |
|--------|-----------|-----------|-----------|--------------|---------|-------|---------------|
| **Lenia / Flow-Lenia** | Continuous CA | External / emerging | None | None | 2D (3D emerging) | ~1K^2 | Continuous field, but no chemistry or agents |
| **Avida** | Digital (instruction set) | Intrinsic | None | None | 2D grid | ~60x60 | Evolution focus, but no physical substrate |
| **Tierra** | Digital (instruction set) | Intrinsic | None | None | 1D memory | ~32K | Self-replicating programs; ecology, no physics |
| **PhysiCell** | Field + sparse agents | None | Reaction-diffusion | Implicit (cell volumes) | 3D | ~500K cells | Architecture twin, but no evolution |
| **AlChemy** | Lambda calculus | Emergent | Abstract (functional) | None (well-stirred) | None | ~1K molecules | Emergence from abstract chemistry, no space |
| **Hutton AC** | 2D particle grid | Emergent | Reaction rules | Membrane loops | 2D | ~100x100 | Compartmentalized AC, but small scale |
| **SwarmChemistry** | Boids-like particles | External | Kinetic (parameters) | None | 2D continuous | ~1K particles | Heterogeneous agents, no field substrate |
| **Neural CA** | Continuous grid | Trained (differentiable) | None | None | 2D | ~64x64 | Differentiable self-organization, but trained not evolved |
| **Kauffman RAF** | Graph/multiset | Emergent | Catalytic networks | Implicit | None | Abstract | Theory of autocatalytic emergence |
| **Gamma/HOCL** | Multiset | N/A (programming model) | Multiset rewriting | None | None | Abstract | Chemical programming paradigm |
| **MARL** | Field + sparse agents | Intrinsic (evolvable rulesets + HGT) | Reaction-diffusion + abstract enzymes | Explicit (interstitial/intracellular) | 3D voxel | 50M voxels | **This project** |

---

## 2. Detailed System Analyses

### 2.1 Lenia and Flow-Lenia

**What it is:** Lenia (Chan, 2019) is a continuous generalization of Conway's Game of Life, extending discrete states, space, and time to continuous domains. It produces remarkably lifelike self-organizing patterns ("creatures") that are geometric, resilient, and adaptive.

**Flow-Lenia** (Plantec et al., 2023) extends Lenia with two critical features:
- **Mass conservation:** Patterns can't create or destroy mass, enabling more physically grounded dynamics
- **Parameter localization:** Instead of global update rules, parameters become local properties of emerging structures, allowing different "species" to coexist in the same simulation

**Relevance to MARL:**
- Flow-Lenia's parameter localization is conceptually similar to MARL's per-cell rulesets -- both allow heterogeneous local rules
- Flow-Lenia's mass conservation parallels MARL's conservation constraints on chemical species
- Key difference: Lenia has no chemistry. Its "field" is abstract state, not chemical concentration. MARL's field has physical meaning (diffusivity, decay, reaction stoichiometry)
- Key difference: Lenia creatures are emergent patterns in the field; MARL cells are explicit agents with internal state. This is a fundamental architectural distinction.
- Flow-Lenia struggles with open-ended evolution -- it "paves the way" but doesn't demonstrate it. MARL's explicit evolution machinery (mutation, HGT) may have an advantage here.

**Citation:** Chan, B.W.-C. (2019). Lenia: Biology of Artificial Life. Complex Systems, 28(3). Plantec et al. (2023). Flow-Lenia: Towards open-ended evolution in cellular automata through mass conservation and parameter localization. ALIFE 2023.

### 2.2 Avida and Tierra

**What they are:** Tierra (Ray, 1991) introduced self-replicating programs competing for CPU time and memory in a 1D "primordial soup." Avida (Lenski et al., 2003) refined this into a 2D grid where digital organisms execute instructions, replicate, and can evolve to perform computational tasks (logic functions) for fitness rewards.

**Relevance to MARL:**
- Avida's instruction-set genomes represent Option C in ADR-003 (small program / instruction set). Avida demonstrated genuine open-ended evolution of complexity, including the landmark Lenski et al. (2003) Nature paper showing evolution of complex features from simpler building blocks.
- Tierra demonstrated parasitism, hyperparasitism, and ecological dynamics without fitness functions -- analogous to MARL's "no fitness function" principle
- Key difference: Both operate on a symbolic/computational substrate. MARL operates on a physical/chemical substrate. This means MARL's "behaviors" are grounded in diffusion physics, not arbitrary computation.
- Key difference: Avida organisms don't interact through a shared medium -- they compete for space and CPU. MARL's field-mediated interaction creates genuine spatial ecology.

**Insight for ADR-003:** Avida's success with instruction-set genomes is strong evidence that Option C (small program) can produce publishable evolutionary dynamics. But Avida's genomes operate on abstract logic, not chemistry. MARL needs a representation that is natively chemical.

**Citation:** Ray, T.S. (1991). An approach to the synthesis of life. Artificial Life II. Lenski, R.E. et al. (2003). The evolutionary origin of complex features. Nature, 423.

### 2.3 PhysiCell

**What it is:** PhysiCell (Ghaffarizadeh et al., 2018) is an open-source agent-based framework for 3D multicellular simulations. It couples a reaction-diffusion solver (BioFVM) with discrete cell agents that can secrete, consume, migrate, divide, and die based on local microenvironmental conditions.

**Relevance to MARL:**
- PhysiCell is MARL's closest architectural cousin. The field + sparse agent architecture is essentially identical.
- PhysiCell scales to ~500K cells on desktop hardware and millions on HPC -- relevant feasibility data for MARL
- Key difference: PhysiCell cell phenotypes are researcher-defined, not evolved. Cell rules are hand-coded or configured via XML/CSV. There is no mutation, no HGT, no evolutionary dynamics.
- Key difference: PhysiCell models explicit cell mechanics (adhesion, repulsion, motility) which MARL deliberately excludes
- PhysiCell's BioFVM uses a similar discretized diffusion solver with operator splitting

**Insight:** PhysiCell validates the architectural pattern MARL is built on. The novelty of MARL is the evolutionary layer on top. In a paper, MARL should be positioned as "PhysiCell meets Avida" -- the physical substrate of the former with the evolutionary dynamics of the latter.

**Citation:** Ghaffarizadeh, A. et al. (2018). PhysiCell: An open source physics-based cell simulator for 3-D multicellular systems. PLOS Computational Biology, 14(2).

### 2.4 AlChemy (Fontana & Buss)

**What it is:** AlChemy (Fontana & Buss, 1994) uses lambda calculus expressions as abstract "molecules." When two molecules interact (function application), they produce a new molecule (the result of lambda reduction). The system is run as a well-stirred reactor -- no spatial structure. Organizations emerge: self-maintaining sets of molecules that reproduce their members through mutual interaction.

**Recent update:** Kruszewski & Ballard (2024) revisited AlChemy and found that complex, stable organizations arise more frequently than originally documented and are robust against collapse into trivial fixed-points. However, they also found that stable organizations "cannot be easily combined into higher order entities" -- a limitation for hierarchical complexity.

**Relevance to MARL:**
- AlChemy demonstrates that abstract chemistry (not literal biochemistry) can produce genuine self-organization. This validates MARL's "abstract enzymes" approach.
- AlChemy's key limitation is the well-stirred assumption -- no spatial structure means no compartmentalization, no gradients, no ecology. MARL addresses this directly.
- AlChemy's lambda-calculus substrate is maximally expressive but computationally expensive and difficult to map to GPU. MARL needs a more constrained but GPU-friendly abstraction.
- The finding that AlChemy organizations can't easily combine into higher-order entities is a warning: MARL's chemistry needs to support hierarchical composition.

**Insight for biochemical abstraction:** AlChemy proves abstract chemistry works. But MARL's chemistry needs to be spatially embedded, GPU-evaluable, and composable -- constraints AlChemy doesn't face.

**Citation:** Fontana, W. & Buss, L. (1994). "The arrival of the fittest": Toward a theory of biological organization. Bulletin of Mathematical Biology, 56(1). Kruszewski, G. & Ballard, A. (2024). Self-Organization in Computation & Chemistry: Return to AlChemy. Chaos, 34(9).

### 2.5 Hutton's Artificial Chemistry

**What it is:** Hutton (2002, 2007) created a 2D artificial chemistry where particles on a grid interact via local reaction rules. Membrane loops spontaneously form from chains of bonded atoms, creating compartmentalized cells. Inside these cells, genetic material (atom chains) encodes enzymes that catalyze specific reactions. Cells can self-reproduce through a process of genome copying and membrane division.

**Relevance to MARL:**
- Hutton's system is the closest prior art to MARL's vision of compartmentalized, evolving chemistry. It demonstrates that membrane-bounded cells with internal genomes can emerge from simple reaction rules.
- Key difference: Hutton's compartmentalization is emergent (membranes form from particle interactions), while MARL's is architectural (cells occupy voxels, interstitial/intracellular is a design-level distinction). MARL's approach trades emergent membranes for computational tractability.
- Key difference: Hutton operates at very small scale (~100x100 2D) with individual particle tracking. MARL's field-based approach enables 50M voxels.
- Hutton's enzymes are specific catalysts encoded by genome sequences -- the genome IS the chemistry. This is a strong model for ADR-003 Option C.

**Insight:** Hutton validates that "genome as catalytic network" is viable. But MARL needs to achieve this at 10^7 voxel scale, which requires the chemistry to be more abstract (field-level, not particle-level).

**Citation:** Hutton, T.J. (2002). Evolvable self-replicating molecules in an artificial chemistry. Artificial Life, 8(3). Hutton, T.J. (2007). Evolvable self-reproducing cells in a two-dimensional artificial chemistry. Artificial Life, 13(1).

### 2.6 Neural Cellular Automata (Mordvintsev et al.)

**What it is:** Growing Neural Cellular Automata (Mordvintsev et al., 2020) uses a small neural network (~8K parameters) as the update rule for each cell in a 2D grid. The network is trained via gradient descent to produce self-organizing, self-repairing patterns. Each cell carries a 16-dimensional state vector and perceives its 3x3 neighborhood via Sobel filters.

**Relevance to MARL:**
- NCA demonstrates that small neural networks can encode complex morphogenetic behaviors -- relevant to ADR-003 Option D (neural network weights as genome)
- NCA's per-cell state vector is analogous to MARL's internal concentration vector
- Key difference: NCA is trained (differentiable optimization toward a target), not evolved. There is no mutation, no population, no ecology.
- Key difference: NCA cells are homogeneous (same weights everywhere). MARL cells are heterogeneous (different rulesets).
- The ~8K parameter count per NCA "ruleset" is informative for sizing MARL rulesets -- but may be too large for per-cell storage at scale (50M voxels x 8K params = 400GB, obviously infeasible). MARL rulesets must be much smaller.

**Insight for ADR-003:** NCA validates that neural-network-based update rules produce rich dynamics. But MARL can't use the NCA approach directly -- it needs per-cell heterogeneous rulesets that are small enough to store and fast enough to evaluate. A compact parametric form (Option B) or very small network is more realistic.

**Citation:** Mordvintsev, A. et al. (2020). Growing Neural Cellular Automata. Distill. doi:10.23915/distill.00023.

### 2.7 Kauffman's Autocatalytic Sets (RAF Theory)

**What it is:** Kauffman (1986) proposed that sufficiently complex chemical reaction systems will spontaneously form autocatalytic sets -- self-sustaining networks where every molecule's formation is catalyzed by some other molecule in the set. Hordijk & Steel formalized this as Reflexively Autocatalytic and Food-generated (RAF) theory, proving that RAF sets arise with high probability when each molecule catalyzes ~1-2 reactions on average.

**Relevance to MARL:**
- RAF theory provides a theoretical foundation for why MARL's abstract chemistry might spontaneously produce self-sustaining metabolic networks
- The phase transition property (autocatalytic sets appear suddenly as reaction density crosses a threshold) suggests MARL should tune its chemical complexity to be above this threshold
- A key criticism: Vasas et al. (2010, PNAS) argued that autocatalytic sets lack evolvability -- they are self-sustaining but can't easily diversify. However, more recent work (Hordijk et al., 2021) shows that autocatalytic sets CAN evolve through a process of subset selection and growth.

**Insight:** MARL's explicit evolutionary machinery (mutation, HGT) addresses the evolvability concern about pure autocatalytic sets. The cells are the evolvable units, not the chemical networks themselves. But the chemistry should be complex enough that autocatalytic-like dynamics can emerge within cells.

**Citation:** Kauffman, S.A. (1986). Autocatalytic sets of proteins. Journal of Theoretical Biology, 119(1). Hordijk, W. & Steel, M. (2017). Chasing the tail: The emergence of autocatalytic networks. BioSystems, 152.

### 2.8 SwarmChemistry (Sayama)

**What it is:** SwarmChemistry (Sayama, 2009) uses heterogeneous populations of Boids-like self-propelled particles, where different particle types have different kinetic parameters (speed, turning rate, interaction range). The "chemistry" is emergent from kinetic interactions -- no explicit reactions, but mixing different parameter sets produces diverse collective behaviors.

**Relevance to MARL:**
- SwarmChemistry's "recipe" concept (a distribution of parameter types in a local neighborhood) is loosely analogous to MARL's HGT -- local mixing of behavioral parameters
- Morphogenetic SwarmChemistry adds re-differentiation (particles can change type based on local context), which is analogous to MARL's cells responding to local chemistry
- Key difference: SwarmChemistry has no underlying field. All interaction is direct (particle-particle). MARL's field-mediated interaction is fundamentally different.
- Key difference: SwarmChemistry particles don't have internal state or metabolism

**Citation:** Sayama, H. (2009). Swarm Chemistry. Artificial Life, 15(1).

### 2.9 Gamma / HOCL (Banatre et al.)

**What it is:** Gamma (Banatre & Le Metayer, 1986) is a programming model where computation is expressed as chemical reactions on a multiset (bag of molecules). Reaction rules consume input molecules and produce output molecules. Execution is inherently parallel -- any applicable reaction can fire at any time.

**Relevance to MARL:**
- Gamma's "multiset rewriting as chemistry" is a clean conceptual model for how MARL's intracellular chemistry could work: a cell's internal state is a multiset of abstract molecules; the ruleset is a set of reaction rules that transform the multiset
- This would make HGT a transfer of reaction rules between cells -- biologically natural
- Key limitation: Gamma is a programming model, not a spatial simulation. No diffusion, no fields.

**Insight for ADR-003:** The Gamma model suggests a potential Option E: rulesets as multiset-rewriting reaction rules. This is chemistry-native, naturally composable via HGT (transfer individual rules), and has clear biological analogy (enzyme = reaction catalyst). Worth exploring.

---

## 3. The Whitespace: What MARL Does That Nobody Else Does

Based on this survey, MARL occupies a unique position at the intersection of three capabilities that no existing system combines:

### 3.1 Reaction-Diffusion Field + Sparse Evolved Agents
- PhysiCell has the field + agents but no evolution
- Avida/Tierra have evolution but no physical field
- Lenia has a continuous field but no explicit agents
- **MARL combines all three**

### 3.2 Explicit Compartmentalization with Evolvable Chemistry
- Hutton has compartmentalized evolving chemistry but at particle scale (~100x100)
- AlChemy has abstract evolving chemistry but no space or compartments
- **MARL has compartmentalized evolving chemistry at field scale (50M voxels)**

### 3.3 Horizontal Gene Transfer in a Spatial Chemical Context
- Avida has spatial evolution but no HGT
- No existing ALife system (that this survey found) combines HGT with field-mediated chemical ecology
- **MARL's HGT in a chemical context is novel**

### 3.4 Publication Angle
The strongest publication angle is: **MARL is the first system to embed evolvable cellular agents with horizontal gene transfer in a 3D reaction-diffusion substrate, enabling emergent chemical ecology (Winogradsky column dynamics) without predefined fitness functions or cell types.**

This positions against:
- PhysiCell (same architecture, adds evolution)
- Avida (same evolutionary dynamics, adds physical chemistry)
- Lenia/Flow-Lenia (same continuous field, adds explicit agents and evolution)
- Hutton (same compartmentalized chemistry, scales to 3D with millions of voxels)

---

## 4. Key Papers to Cite

| Paper | Year | Why |
|-------|------|-----|
| Ghaffarizadeh et al. "PhysiCell" | 2018 | Architectural precedent (field + sparse agents) |
| Chan "Lenia" | 2019 | Continuous CA reference; MARL extends with chemistry |
| Plantec et al. "Flow-Lenia" | 2023 | Parameter localization; mass conservation |
| Fontana & Buss "AlChemy" | 1994 | Abstract chemistry self-organization |
| Hutton "Evolvable self-reproducing cells" | 2007 | Compartmentalized evolving AC |
| Lenski et al. "Evolution of complex features" (Avida) | 2003 | Open-ended evolution in digital organisms |
| Kauffman "Autocatalytic sets" | 1986 | Theoretical basis for emergent metabolism |
| Mordvintsev et al. "Growing Neural CA" | 2020 | Neural update rules; self-organization |
| Sayama "Swarm Chemistry" | 2009 | Heterogeneous agent chemistry |
| Ray "Tierra" | 1991 | Self-replicating programs; no fitness function |
| Bedau et al. "Open-ended evolution" | 2000 | OEE metrics and definitions |
| Taylor et al. "OEE workshop" | 2016 | Requirements for open-ended evolution |
| Adams et al. "MODES toolbox" | 2019 | Measuring open-ended dynamics |

---

## 5. Gaps Identified for Future Research

1. **No existing system combines RD + evolution + HGT at scale.** This is MARL's core novelty.
2. **Compartmentalization in ALife is usually emergent** (membranes from particles) **or absent.** MARL's architectural compartmentalization (voxel = cell) is a pragmatic middle ground that hasn't been well-explored.
3. **OEE metrics for chemically-grounded systems** are underdeveloped. MODES was designed for Avida-like systems. MARL may need adapted metrics.
4. **GPU-scale artificial chemistries** barely exist. Most AC work is CPU-based at small scale. MARL's GPU-first constraint at 50M voxels is genuinely novel territory.
5. **The "abstract enzyme" abstraction level** needs formal definition. The literature offers a spectrum from literal protein folding to pure lambda calculus. MARL needs to find the sweet spot.
