# Research: Assembly Index Computation for MARL Rulesets

**Created:** 2026-03-15 (Iteration 007)
**Purpose:** Concrete algorithm for computing assembly index over MARL rulesets. Required for paper outline Section 5.1 (OEE metrics) and Experiments 4-5 (assembly index controls, long-run OEE test). This document specifies what "assembly index" means for catalytic reaction networks, how to compute it, and what it costs per tick.

---

## 1. Background: Assembly Theory Applied to Non-Molecular Objects

Assembly theory (Cronin et al., 2023, Nature) defines the **assembly index** (AI) of an object as the minimum number of joining operations needed to construct it from a set of basic building blocks. For molecules, building blocks are individual bonds; joining operations are bond formations. The **assembly** of a population combines AI with copy number: objects that are both complex (high AI) and abundant (high copy number) are evidence of selection.

The theory is not limited to molecules. Assembly index has been extended to binary strings (Abrahao et al., 2024, Mathematics), where it reduces to the shortest addition chain problem. The key insight: assembly index is domain-independent -- it measures the minimum number of recursive constructions from parts, regardless of the substrate.

For MARL rulesets, we define an analogous assembly index: the minimum number of edit operations to construct the ruleset from the null (empty) state.

---

## 2. Formal Definition: Ruleset Assembly Index

### 2.1 The Null Ruleset

The **null ruleset** is the state of a cell with no functional metabolism:

```
null_ruleset = Ruleset {
    receptors:  [S] { k_half=0, n_hill=1, gain=0 }   // no sensing
    transport:  [S] { uptake_rate=0, secrete_rate=0 }  // no membrane transport
    reactions:  [R_MAX] { v_max=0 }                    // no reactions (all inactive)
    effectors:  [S] { threshold=inf, rate=0 }          // no secretion
    fate:       { death=0.05, quiescence=0.2, division=0.8 }  // defaults
    hgt_propensity: 0
    mutation_rate:  0
}
```

A cell with a null ruleset has no active reactions, no transport, no sensing. It survives only as long as its initial energy lasts (approximately 50 ticks at lambda_maintenance = 0.02).

### 2.2 Edit Operations

The assembly index counts the minimum number of the following **atomic edit operations** needed to transform the null ruleset into the target ruleset:

| Operation | Description | Cost |
|-----------|-------------|------|
| **activate_reaction** | Set v_max of an inactive slot to a nonzero value (while also setting substrate, product, catalyst, k_m, k_cat) | 1 |
| **rewire_reaction** | Change one species index (substrate, product, or catalyst) of an already-active reaction | 1 |
| **tune_kinetic** | Change one kinetic parameter (v_max, k_m, k_cat) of an already-active reaction | 1 |
| **activate_receptor** | Set gain of a receptor from 0 to nonzero (while also setting k_half, n_hill) | 1 |
| **tune_receptor** | Change one receptor parameter (k_half, n_hill, gain) | 1 |
| **activate_transporter** | Set uptake_rate or secrete_rate from 0 to nonzero (with species mapping) | 1 |
| **tune_transport** | Change one transport parameter | 1 |
| **activate_effector** | Set an effector from inactive to active (finite threshold, nonzero rate, species mapping) | 1 |
| **tune_effector** | Change one effector parameter | 1 |
| **tune_fate** | Change one fate threshold | 1 |
| **tune_meta** | Change hgt_propensity or mutation_rate | 1 |

All operations have uniform cost = 1. This is a deliberate simplification: it means activation of a new reaction (which requires setting 5-7 parameters at once) costs the same as tuning an existing parameter. This is defensible because activation IS the key creative step -- it introduces a new catalytic capability -- while tuning is gradient-following within an existing architecture.

### 2.3 Assembly Index Definition

```
assembly_index(ruleset) = min number of edit operations to transform null_ruleset -> ruleset
```

This is equivalent to an edit distance from null, where each edit operation has cost 1.

### 2.4 Why Edit Distance, Not Pathway Assembly

In molecular assembly theory, the assembly index is the length of the shortest *assembly pathway* -- a sequence of joining operations where intermediate products can be reused. This is what makes the problem NP-hard (it reduces to the shortest addition chain problem).

For MARL rulesets, we use a simpler formulation: edit distance from null. This is computable in O(1) time per ruleset because the edits are independent -- there is no "reuse of intermediate products." Each parameter is either at its null value or not, and changing it costs exactly 1 operation regardless of other parameter values.

This is a meaningful simplification. In molecular assembly, the NP-hardness comes from the combinatorial reuse of substructures (subgraph isomorphism). In a parameterized ruleset, there is no substructure reuse -- each parameter is an independent degree of freedom. The interesting structure is in the *topology* of the reaction network, not in parameter sharing.

---

## 3. Concrete Algorithm

### 3.1 Per-Ruleset Assembly Index (Exact, O(1))

```python
def assembly_index(ruleset, null=NULL_RULESET):
    """
    Compute the assembly index of a ruleset.
    This is the total number of parameters that differ from the null state.

    Cost: O(R_MAX + S) per ruleset = O(1) since R_MAX and S are constants.
    """
    ai = 0

    # Count active reactions (each costs 1 for activation)
    for r in range(R_MAX):
        if ruleset.reactions[r].v_max != 0.0:
            ai += 1  # activation cost
            # Count non-default kinetic parameters within the active reaction
            if ruleset.reactions[r].k_m != DEFAULT_KM:
                ai += 1
            if ruleset.reactions[r].k_cat != DEFAULT_KCAT:
                ai += 1
            # Species wiring is part of activation -- counted above

    # Count active receptors
    for i in range(S):
        if ruleset.receptors[i].gain != 0.0:
            ai += 1  # activation
            if ruleset.receptors[i].k_half != DEFAULT_KHALF:
                ai += 1
            if ruleset.receptors[i].n_hill != DEFAULT_NHILL:
                ai += 1

    # Count active transporters
    for i in range(S):
        if ruleset.transport[i].uptake_rate != 0.0 or ruleset.transport[i].secrete_rate != 0.0:
            ai += 1  # activation
            # tune counts for non-default rates
            if ruleset.transport[i].uptake_rate != DEFAULT_UPTAKE:
                ai += 1
            if ruleset.transport[i].secrete_rate != DEFAULT_SECRETE:
                ai += 1

    # Count active effectors
    for i in range(S):
        if ruleset.effectors[i].rate != 0.0:
            ai += 1  # activation
            if ruleset.effectors[i].threshold != DEFAULT_THRESH:
                ai += 1

    # Count non-default fate parameters
    if ruleset.fate.death_energy != DEFAULT_DEATH:
        ai += 1
    if ruleset.fate.quiescence_energy != DEFAULT_QUIESCENCE:
        ai += 1
    if ruleset.fate.division_energy != DEFAULT_DIVISION:
        ai += 1

    # Count non-default meta parameters
    if ruleset.hgt_propensity != 0.0:
        ai += 1
    if ruleset.mutation_rate != 0.0:
        ai += 1

    return ai
```

### 3.2 Topology-Aware Assembly Index (Richer, Still O(1))

The above algorithm counts parameter edits but ignores the *structural complexity* of the reaction network. A reaction network with 8 reactions forming two independent autocatalytic loops is more complex than 8 reactions all converting the same substrate to the same product.

A richer metric weights the assembly index by topological features:

```python
def topology_assembly_index(ruleset):
    """
    Assembly index that accounts for reaction network structure.
    Combines parameter-count AI with topological complexity bonus.

    Cost: O(R_MAX^2 + R_MAX * M) per ruleset = O(1) for fixed constants.
    """
    # Base: parameter-level assembly index
    ai = assembly_index(ruleset)

    # Topological bonus: count unique species roles
    substrates_used = set()
    products_used = set()
    catalysts_used = set()

    for r in range(R_MAX):
        if ruleset.reactions[r].v_max != 0.0:
            substrates_used.add(ruleset.reactions[r].substrate)
            products_used.add(ruleset.reactions[r].product)
            catalysts_used.add(ruleset.reactions[r].catalyst)

    # Species diversity: how many distinct species participate?
    all_species = substrates_used | products_used | catalysts_used
    species_diversity = len(all_species)

    # Catalytic depth: longest chain in the dependency graph
    # A -> B means "species A is a catalyst for a reaction producing species B"
    # This measures how many steps of catalytic dependency exist.
    depth = catalytic_depth(ruleset)

    # Cycle count: number of autocatalytic cycles in the reaction graph
    cycles = count_autocatalytic_cycles(ruleset)

    # Combined index: base + structural bonuses
    # Weights chosen so that topological features contribute ~30-50% of total AI
    # for complex rulesets
    topology_ai = ai + species_diversity + 2 * depth + 3 * cycles

    return topology_ai


def catalytic_depth(ruleset):
    """
    Compute the longest directed path in the catalytic dependency graph.

    Graph: nodes = internal species (M nodes)
           edge A -> B exists if there is a reaction where:
             catalyst=A, product=B (A enables production of B)

    Longest path in a DAG (or longest simple path if cycles exist).
    Since M <= 16 and R_MAX = 16, this graph has at most 16 nodes
    and 16 edges. DFS with memoization is O(M + R_MAX) = O(1).
    """
    # Build adjacency list
    adj = defaultdict(set)
    for r in range(R_MAX):
        rxn = ruleset.reactions[r]
        if rxn.v_max != 0.0:
            adj[rxn.catalyst].add(rxn.product)

    # DFS with memoization for longest path
    memo = {}
    def longest_path(node, visited):
        if node in memo and not (visited & memo[node][1]):
            return memo[node][0]
        max_len = 0
        for neighbor in adj[node]:
            if neighbor not in visited:
                visited.add(neighbor)
                length = 1 + longest_path(neighbor, visited)
                max_len = max(max_len, length)
                visited.remove(neighbor)
        return max_len

    max_depth = 0
    for node in adj:
        max_depth = max(max_depth, longest_path(node, {node}))

    return max_depth


def count_autocatalytic_cycles(ruleset):
    """
    Count the number of distinct autocatalytic cycles in the reaction network.

    A cycle exists when: species A catalyzes production of B, B catalyzes
    production of C, ..., and some species catalyzes production of A.

    Since the graph has at most M=16 nodes, cycle detection via DFS is trivial.
    We count strongly connected components (SCCs) with size > 1 using Tarjan's
    algorithm. Each SCC represents one or more interlocking autocatalytic loops.

    Cost: O(M + R_MAX) = O(1).
    """
    # Build the same catalyst -> product graph
    adj = defaultdict(set)
    for r in range(R_MAX):
        rxn = ruleset.reactions[r]
        if rxn.v_max != 0.0:
            adj[rxn.catalyst].add(rxn.product)

    # Tarjan's SCC algorithm on a 16-node graph
    sccs = tarjan_scc(adj, M)

    # Count SCCs with more than 1 node (= autocatalytic cycles)
    cycle_count = sum(1 for scc in sccs if len(scc) > 1)

    return cycle_count
```

### 3.3 Reaction Network Topology Hash

For the MODES novelty metric and phylogenetic analysis, we also need a **topology fingerprint** -- a hash that identifies the structural identity of a reaction network, ignoring kinetic parameters:

```python
def topology_hash(ruleset):
    """
    Compute a fingerprint of the reaction network topology.
    Two rulesets with the same topology hash have the same set of
    (substrate, product, catalyst) triples for active reactions,
    regardless of kinetic parameters.

    Cost: O(R_MAX * log(R_MAX)) for sorting. O(1) for fixed R_MAX.
    """
    # Extract active reaction topologies, sorted for canonical form
    active = []
    for r in range(R_MAX):
        rxn = ruleset.reactions[r]
        if rxn.v_max != 0.0:
            active.append((rxn.substrate, rxn.product, rxn.catalyst))

    # Sort to make the hash order-independent (slot position doesn't matter)
    active.sort()

    # Hash the sorted tuple list
    return hash(tuple(active))
```

---

## 4. Population-Level Assembly Metrics

### 4.1 Population Assembly (Cronin's A)

Following Cronin et al. (2023), the population-level assembly combines individual AI with copy number:

```python
def population_assembly(cells):
    """
    A(population) = sum over unique topologies t:
                      assembly_index(t) * log2(1 + copy_number(t))

    The log weighting prevents a single abundant simple organism from
    dominating the metric. Complex + abundant = high contribution.
    """
    # Group cells by topology
    topology_groups = defaultdict(list)
    for cell in cells:
        h = topology_hash(cell.ruleset)
        topology_groups[h].append(cell)

    A = 0.0
    for h, group in topology_groups.items():
        ai = topology_assembly_index(group[0].ruleset)
        copy_number = len(group)
        A += ai * math.log2(1 + copy_number)

    return A
```

### 4.2 Assembly Index Distribution

For the paper (Figure 10), track the full distribution, not just the mean:

```python
def assembly_distribution(cells):
    """
    Returns (min, p25, median, p75, max, mean) of assembly index
    across all living cells. Plotted every 100 ticks.
    """
    indices = [topology_assembly_index(c.ruleset) for c in cells]
    return {
        'min':    min(indices),
        'p25':    percentile(indices, 25),
        'median': percentile(indices, 50),
        'p75':    percentile(indices, 75),
        'max':    max(indices),
        'mean':   mean(indices),
        'count':  len(indices),
    }
```

### 4.3 Assembly Index vs. Random Baseline (Experiment 4)

Paper Experiment 4 requires comparing evolved AI against random rulesets:

```python
def random_assembly_baseline(n_samples=10000):
    """
    Generate n_samples random rulesets and compute their AI distribution.
    This establishes the null hypothesis: what AI would you expect
    from random parameterization?

    Expected result: random rulesets have moderate AI (many parameters
    are nonzero by chance) but low topology_assembly_index (random wiring
    rarely produces autocatalytic cycles or deep dependency chains).
    """
    indices = []
    for _ in range(n_samples):
        r = random_ruleset()
        indices.append(topology_assembly_index(r))
    return assembly_distribution_stats(indices)
```

---

## 5. Computational Cost Analysis

### 5.1 Per-Cell Cost

| Operation | Cost per cell | At 100K cells |
|-----------|--------------|---------------|
| assembly_index (basic) | ~200 comparisons | 20M comparisons |
| topology_assembly_index | ~200 comparisons + graph traversal (16 nodes) | 20M + 1.6M |
| topology_hash | sort 16 elements + hash | 1.6M sort ops |

All operations are integer/comparison-based. No floating point. At 100K cells, the total computation is roughly 25M integer operations -- approximately 0.01 ms on a modern CPU. This is negligible.

### 5.2 Measurement Frequency

Assembly index does NOT need to be computed every tick. The paper calls for measurements every 100 ticks (Experiment 5: 10,000+ tick run). At 100-tick intervals:

- Compute assembly_distribution for all cells: ~0.01 ms
- Compute population_assembly: ~0.02 ms (includes grouping by topology hash)
- Write to log: ~0.1 ms

**Total overhead: < 0.15 ms every 100 ticks.** This is unmeasurable against the ~10 ms/tick simulation cost.

### 5.3 Storage Cost

Per measurement epoch (every 100 ticks):
- 6 distribution statistics: 48 bytes
- Population assembly value: 8 bytes
- Unique topology count: 4 bytes
- Optional: top-10 most abundant topologies with their AI: 160 bytes

At 10,000 ticks = 100 epochs: ~22 KB total. Negligible.

---

## 6. Relationship to Molecular Assembly Theory

### 6.1 Differences from Cronin et al.

| Aspect | Molecular AT (Cronin) | MARL Ruleset AT |
|--------|----------------------|-----------------|
| Object | Molecular graph | Parameterized reaction network |
| Building blocks | Bonds | Edit operations |
| Joining | Bond formation (subgraph reuse) | Parameter modification |
| Complexity source | Substructure sharing (NP-hard) | Network topology (polynomial) |
| Key insight | Complex molecules imply selection | Complex rulesets imply selection |

### 6.2 Why MARL's Version is Polynomial

Molecular assembly index is NP-hard because the "joining" operation allows reuse of intermediate substructures, making it equivalent to the shortest addition chain problem. In MARL, each parameter is an independent degree of freedom -- there is no reuse. The topological complexity (catalytic depth, cycle count) is computed on a graph with at most M=16 nodes and R_MAX=16 edges, which is trivially small.

This is not a weakness -- it is a feature. MARL's assembly index is cheap to compute because the interesting complexity is in the *dynamics* of the network (what it does when embedded in the field), not in the *structure* of the encoding. The topology_assembly_index enriches the basic edit-distance metric with structural features that capture this dynamic complexity indirectly.

### 6.3 Connection to the Paper

For publication, we frame MARL's assembly index as an adaptation of assembly theory to parameterized networks:

> "Following Cronin et al. (2023), we define the assembly index of a MARL ruleset as the minimum number of edit operations needed to construct it from a null (non-functional) state. Unlike molecular assembly, where the NP-hard shortest pathway problem arises from substructure reuse, ruleset assembly is polynomial because each parameter is an independent degree of freedom. We enrich the basic edit-distance metric with topological features (catalytic depth, autocatalytic cycle count) that capture the structural complexity of the evolved reaction network."

This framing positions MARL's metric as a principled domain adaptation of assembly theory, not an ad hoc complexity measure.

---

## 7. Validation Strategy

### 7.1 Sanity Checks

1. **Null ruleset has AI = 0.** By definition.
2. **Hand-designed Winogradsky phototroph has AI ~ 15-25.** (3 active reactions, 3 active receptors, 3 active transporters, 3 active effectors, tuned fate = 12 activations + tuning).
3. **Random rulesets have moderate AI but low topology_AI.** Random wiring rarely produces autocatalytic cycles.
4. **Evolved populations have higher topology_AI than random.** This is the key OEE claim.

### 7.2 Expected Dynamics (Experiment 5)

- Ticks 0-100: AI stable at seeded level (hand-designed starter metabolisms).
- Ticks 100-500: AI rises as mutation explores parameter space. Mostly tuning, not topology changes.
- Ticks 500-2000: Punctuated increases as topology mutations create new reaction pathways. AI should correlate with metabolic innovation events visible in the phylogeny.
- Ticks 2000-10000: Continued increase with decreasing rate. Whether AI plateaus or keeps rising is the central OEE question.

---

## 8. References

- Cronin, L. et al. (2023). Assembly theory explains and quantifies selection and evolution. Nature, 622, 244-249.
- Abrahao, F.S. et al. (2024). Assembly Theory of Binary Messages. Mathematics, 12(10), 1600.
- Liu, Y. et al. (2024). Rapid Computation of the Assembly Index of Molecular Graphs. arXiv:2410.09100.
- Marshall, S.M. et al. (2021). Exploring and mapping chemical space with molecular assembly trees. Science, 372(6545), 956-960.
- Jirasek, M. et al. (2024). Investigating and Quantifying Molecular Complexity Using Assembly Theory and Spectroscopy. ACS Central Science, 10(5), 1054-1064.
- Sharma, A. et al. (2025). Assembly theory and its relationship with computational complexity. npj Complexity, 2, 49.
