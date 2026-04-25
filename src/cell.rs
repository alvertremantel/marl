//! Cell agent module — the evolvable catalytic core of the simulation.
//!
//! Each cell is a small biochemical computer: it senses its environment
//! (receptors), exchanges chemicals across its membrane (transporters),
//! runs internal catalytic reactions, secretes products (effectors), and
//! makes life/death/division decisions based on energy levels.
//!
//! The entire behavior of a cell is encoded in its **Ruleset** — a
//! collection of parameter arrays that define receptor sensitivities,
//! transport rates, reaction kinetics, etc. Rulesets mutate during
//! division, so evolution acts on these parameters directly. There is
//! no fitness function; selection emerges from the chemistry.
//!
//! ## The 5-Phase Cell Tick
//!
//! Each simulation tick, every cell runs through five phases in order:
//!
//! 1. **Receptor pass** — sense external chemical concentrations using
//!    Hill-function activations. (Currently computed but not yet wired
//!    to gate downstream phases.)
//!
//! 2. **Transport pass** — move chemicals across the cell membrane.
//!    Uptake brings external species into the cell; secretion pushes
//!    internal species out. Both use Michaelis-Menten saturation.
//!
//! 3. **Intracellular reactions** — the catalytic network. Each reaction
//!    consumes a substrate and produces a product, catalyzed by a third
//!    species. Michaelis-Menten kinetics with an epsilon background rate
//!    (see `research-bootstrapping.md` for why epsilon matters).
//!
//! 4. **Effector pass** — secrete internal species into the field when
//!    concentrations exceed a threshold. Quiescent cells skip this.
//!
//! 5. **Fate decision** — based on energy (internal[0]):
//!    - Energy > division_energy → divide
//!    - Energy < death_energy → die
//!    - Energy < quiescence_energy → go dormant (skip effectors)
//!    - Otherwise → active

use crate::config::*;

/// Receptor parameters: Hill-function sensor for an external chemical.
///
/// The Hill equation models cooperative binding:
///   activation = gain * c^n / (k_half^n + c^n)
///
/// - `k_half`: the concentration at which activation reaches 50%
/// - `n_hill`: cooperativity coefficient (1 = hyperbolic, >1 = sigmoidal switch)
/// - `gain`: maximum output scaling
#[derive(Clone, Debug)]
pub struct ReceptorParams {
    pub k_half: f32,
    pub n_hill: f32,
    pub gain: f32,
}

/// Membrane transporter: moves one chemical species between the external
/// field and the cell's internal pool.
///
/// Each transporter is specific to one (ext_species, int_species) pair.
/// Uptake and secretion rates are independent — a transporter can do both,
/// creating a net flux direction based on concentration gradients.
#[derive(Clone, Debug)]
pub struct TransportParams {
    pub uptake_rate: f32,
    pub secrete_rate: f32,
    pub ext_species: u8,
    pub int_species: u8,
}

/// A single catalytic reaction in the cell's metabolic network.
///
/// Models enzyme kinetics: substrate → product, catalyzed by a third
/// internal species. Rate follows Michaelis-Menten:
///   rate = v_max * [S]/(k_m + [S]) * (epsilon + [catalyst]/(k_cat + [catalyst]))
///
/// The epsilon term ensures a tiny background rate even without catalyst,
/// preventing evolutionary dead ends where a useful reaction can never
/// start because the catalyst doesn't exist yet.
///
/// Optional cofactor: if cofactor != 0xFF, the reaction also requires
/// (and partially consumes) a second internal species.
#[derive(Clone, Debug)]
pub struct Reaction {
    pub substrate: u8, // consumed internal species
    pub product: u8,   // produced internal species
    pub catalyst: u8,  // enzyme/catalyst species
    pub cofactor: u8,  // second required species, 0xFF = none
    pub k_m: f32,      // Michaelis constant for substrate
    pub v_max: f32,    // maximum reaction rate
    pub k_cat: f32,    // half-saturation for catalyst
}

/// Effector: conditional secretion of an internal species into the field.
///
/// When internal[int_species] exceeds `threshold`, the cell secretes at
/// the given rate. This creates emergent signaling — cells that accumulate
/// waste products or metabolic byproducts automatically release them,
/// making those chemicals available to neighboring cells.
#[derive(Clone, Debug)]
pub struct EffectorParams {
    pub threshold: f32,
    pub rate: f32,
    pub int_species: u8,
    pub ext_species: u8,
}

/// Energy thresholds and cell cycle timing that determine cell fate.
///
/// There is no explicit fitness function — a cell that can accumulate
/// energy above `division_energy` AND sustain the costly preparation
/// phase will reproduce. One that can't maintain energy above
/// `death_energy` will die. Selection is entirely thermodynamic.
///
/// The division prep phase models DNA replication, organelle duplication,
/// and membrane synthesis — biologically expensive processes that take
/// time. Cells can evolve shorter prep times but pay exponentially
/// more energy to rush.
#[derive(Clone, Debug)]
pub struct FateParams {
    pub division_energy: f32,
    pub death_energy: f32,
    pub quiescence_energy: f32,
    /// How many ticks the cell spends in division prep. Evolvable.
    /// Lower = faster division but higher energy cost during prep.
    pub division_prep_ticks: f32,
}

/// The complete evolvable genotype of a cell.
///
/// Contains all parameters that define the cell's behavior: how it senses,
/// what it transports, which reactions it catalyzes, when it secretes,
/// and at what energy levels it divides/dies. All of these mutate during
/// reproduction, so the ruleset is the unit of evolution.
///
/// Size: ~314 bytes (8 receptors + 8 transporters + 16 reactions +
/// 8 effectors + fate params + mutation/HGT rates).
#[derive(Clone, Debug)]
pub struct Ruleset {
    pub receptors: [ReceptorParams; S_RECEPTORS],
    pub transport: [TransportParams; S_TRANSPORTERS],
    pub reactions: [Reaction; R_MAX],
    pub effectors: [EffectorParams; S_EFFECTORS],
    pub fate: FateParams,
    /// Probability of accepting a horizontal gene transfer event.
    /// Evolvable — evolution can select for or against gene sharing.
    pub hgt_propensity: f32,
    /// Per-parameter probability of point mutation during division.
    /// Also evolvable (meta-evolution: evolution of evolvability).
    pub mutation_rate: f32,
}

/// Events produced by a cell tick — the cell's "decision" for this timestep.
#[derive(Clone, Debug)]
pub enum CellEvent {
    None,
    Division,
    Death,
    Quiescence,
}

/// The complete state of a single cell at one point in time.
///
/// Each cell occupies exactly one voxel in the 3D grid. Its position,
/// internal chemical concentrations, and ruleset together define
/// everything about it. The lineage_id is a random tag assigned at
/// birth for tracking evolutionary lineages.
#[derive(Clone, Debug)]
pub struct CellState {
    pub pos: [u16; 3],
    pub lineage_id: u64,
    pub age: u32,
    pub internal: [f32; M_INT],
    pub ruleset: Ruleset,
    pub quiescent: bool,
    /// Which original starter metabolism this cell descends from.
    /// 0 = phototroph, 1 = chemolithotroph, 2 = anaerobe.
    /// Inherited at division, never mutated — permanent ancestral marker.
    pub starter_type: u8,
    /// Division prep countdown. 0 = not in prep. When > 0, the cell is
    /// preparing to divide and pays extra maintenance. Reaches 0 → divide.
    pub prep_remaining: u16,
}

impl CellState {
    /// Run one complete cell update tick. Returns field deltas and an event.
    /// This is the 5-phase update:
    ///   Phase 1: Receptor pass (read external field, compute Hill activations)
    ///   Phase 2: Transport pass (membrane crossing, external <-> internal)
    ///   Phase 3: Intracellular reactions (catalytic network with epsilon background)
    ///   Phase 4: Effector pass (internal -> external secretion)
    ///   Phase 5: Fate decision (energy thresholds)
    pub fn tick(
        &mut self,
        ext_conc: &[f32; S_EXT],
        light: f32,
        sim: &SimulationConfig,
    ) -> ([f32; S_EXT], CellEvent) {
        let mut field_deltas = [0.0f32; S_EXT];
        // Cell tick runs once per full tick
        let dt = sim.dt;

        // === PHASE 1: RECEPTOR PASS ===
        // Activations modulate transport rates (not yet wired — future extension).
        // Computed here for correctness; will matter when receptor-gated transport is added.
        let mut _activation = [0.0f32; S_RECEPTORS];
        for i in 0..S_RECEPTORS {
            let r = &self.ruleset.receptors[i];
            if i < S_EXT && r.gain.abs() > sim.active_reaction_threshold {
                let c = ext_conc[i];
                // Guard against negative/zero base in powf
                let k = r.k_half.max(1e-6);
                let n = r
                    .n_hill
                    .clamp(sim.hill_exponent_clamp_low, sim.hill_exponent_clamp_high);
                let kn = k.powf(n);
                let cn = c.max(0.0).powf(n);
                _activation[i] = r.gain * cn / (kn + cn + 1e-9);
            }
        }

        // === PHASE 2: TRANSPORT PASS ===
        for i in 0..S_TRANSPORTERS {
            let tp = &self.ruleset.transport[i];
            let ext_idx = tp.ext_species as usize;
            let int_idx = tp.int_species as usize;
            if ext_idx >= S_EXT || int_idx >= M_INT {
                continue;
            }

            let uptake = tp.uptake_rate * ext_conc[ext_idx] / (1.0 + ext_conc[ext_idx]);
            let secretion =
                tp.secrete_rate * self.internal[int_idx] / (1.0 + self.internal[int_idx]);

            self.internal[int_idx] =
                (self.internal[int_idx] + (uptake - secretion) * dt).clamp(0.0, sim.c_max);
            field_deltas[ext_idx] += (secretion - uptake) * dt;
        }

        // Light is stored as a pseudo-internal concentration so reactions can use it as catalyst.
        // Internal species 15 (last slot) = light availability this tick.
        // Photosynthesis reactions reference catalyst=15 to be light-dependent.
        self.internal[M_INT - 1] = light;

        // === PHASE 3: INTRACELLULAR REACTIONS ===
        for rxn in &self.ruleset.reactions {
            if rxn.v_max.abs() < sim.active_reaction_threshold {
                continue;
            } // inactive slot
            if rxn.substrate == rxn.product {
                continue;
            } // no-op (e.g. energy→energy)

            let sub_idx = rxn.substrate as usize;
            let prod_idx = rxn.product as usize;
            let cat_idx = rxn.catalyst as usize;
            if sub_idx >= M_INT || prod_idx >= M_INT || cat_idx >= M_INT {
                continue;
            }

            let s = self.internal[sub_idx];
            let c = self.internal[cat_idx];

            // Michaelis-Menten with epsilon background rate
            let substrate_term = s / (rxn.k_m + s + f32::EPSILON);
            let catalyst_term = sim.epsilon + c / (rxn.k_cat + c + f32::EPSILON);
            let mut rate = rxn.v_max * substrate_term * catalyst_term;

            // Optional cofactor
            if rxn.cofactor != 0xFF {
                let cof_idx = rxn.cofactor as usize;
                if cof_idx < M_INT {
                    let cof = self.internal[cof_idx];
                    rate *= cof / (1.0 + cof);
                    // Consume cofactor at half rate, clamped to available
                    let cof_consumed = (0.5 * rate * dt).min(self.internal[cof_idx]);
                    self.internal[cof_idx] -= cof_consumed;
                }
            }

            // Clamp flux to available substrate — cannot produce more than consumed
            let flux = (rate * dt).min(self.internal[sub_idx]);
            self.internal[sub_idx] -= flux;
            self.internal[prod_idx] = (self.internal[prod_idx] + flux).min(sim.c_max);
        }

        // Maintenance energy drain: each tick, the cell loses lambda_maintenance
        // fraction of its current energy. During division prep, this cost is
        // multiplied — the cell is duplicating its genome, ribosomes, membranes.
        // Cells that evolve shorter prep times pay even more (rush penalty).
        let prep_multiplier = if self.prep_remaining > 0 {
            // Rush penalty: faster prep = more expensive per tick
            let evolved_prep = self.ruleset.fate.division_prep_ticks.max(1.0);
            let rush = (sim.base_division_prep - evolved_prep).max(0.0);
            sim.prep_maintenance_multiplier + rush * sim.rush_penalty_rate
        } else {
            1.0
        };
        self.internal[0] *= 1.0 - sim.lambda_maintenance * prep_multiplier * dt;

        // Protein expression cost: each active enzyme requires transcription,
        // translation, and folding resources. No-op reactions (substrate == product)
        // are skipped — they don't encode a real enzyme.
        let active_rxn_count = self
            .ruleset
            .reactions
            .iter()
            .filter(|r| r.v_max.abs() > sim.active_reaction_threshold && r.substrate != r.product)
            .count();
        self.internal[0] =
            (self.internal[0] - active_rxn_count as f32 * sim.reaction_maintenance * dt).max(0.0);

        // === PHASE 4: EFFECTOR PASS ===
        if !self.quiescent {
            for eff in &self.ruleset.effectors {
                let int_idx = eff.int_species as usize;
                let ext_idx = eff.ext_species as usize;
                if int_idx >= M_INT || ext_idx >= S_EXT {
                    continue;
                }

                if self.internal[int_idx] > eff.threshold {
                    let amount =
                        eff.rate * self.internal[int_idx] / (1.0 + self.internal[int_idx]) * dt;
                    self.internal[int_idx] = (self.internal[int_idx] - amount).max(0.0);
                    field_deltas[ext_idx] += amount;
                }
            }
        }

        // === PHASE 5: FATE DECISION ===
        let energy = self.internal[0];
        self.age += 1;

        // Death check first — hard floor is non-evolvable physics.
        let effective_death = self.ruleset.fate.death_energy.max(sim.hard_death_floor);

        let event = if energy < effective_death {
            // Dead — including cells that ran out of energy during prep.
            // Failed division attempts are a real biological phenomenon.
            self.prep_remaining = 0;
            CellEvent::Death
        } else if self.prep_remaining > 0 {
            // Currently in division prep — counting down.
            // The extra maintenance cost was already applied above.
            self.prep_remaining -= 1;
            if self.prep_remaining == 0 {
                // Prep complete — ready to divide!
                CellEvent::Division
            } else {
                CellEvent::None
            }
        } else if energy > self.ruleset.fate.division_energy {
            // Energy threshold reached — enter division prep phase.
            // The cell doesn't divide yet; it starts the costly prep countdown.
            let prep = self.ruleset.fate.division_prep_ticks.max(1.0) as u16;
            self.prep_remaining = prep;
            self.quiescent = false;
            CellEvent::None // division happens when prep_remaining hits 0
        } else if energy < self.ruleset.fate.quiescence_energy {
            self.quiescent = true;
            CellEvent::Quiescence
        } else {
            self.quiescent = false;
            CellEvent::None
        };

        (field_deltas, event)
    }
}

// ============================================================================
// MUTATION — the engine of evolution
// ============================================================================
//
// When a cell divides, its daughter's ruleset is mutated. Two kinds of
// mutation are implemented:
//
// 1. **Parametric mutation** (common): small Gaussian perturbations to
//    continuous parameters (rates, thresholds, kinetic constants). This
//    is like fine-tuning an enzyme's binding affinity.
//
// 2. **Structural mutation** (rare, 10x less likely): rewiring which
//    species a reaction acts on. This is like evolving a new enzyme
//    that catalyzes a completely different reaction.
//
// The mutation rate itself is evolvable (meta-evolution). Populations
// under strong selection pressure may evolve higher mutation rates to
// explore more of the fitness landscape. This is observed in real
// microbial populations (mutator phenotypes).

use rand::Rng;
use rand_distr::{Distribution, Normal};

impl Ruleset {
    /// Apply random mutations to all evolvable parameters.
    ///
    /// Called on the daughter cell's ruleset after division. Each parameter
    /// independently has a `mutation_rate` probability of being perturbed
    /// by a small Gaussian (mean=0, std=0.1). Parameters are clamped to
    /// non-negative values since rates and concentrations can't be negative.
    ///
    /// Structural mutations (rewiring reaction substrate/product/catalyst)
    /// happen 10x less frequently — they're more disruptive and usually lethal,
    /// but occasionally create entirely new metabolic capabilities.
    pub fn mutate(&mut self, rng: &mut impl Rng, sim: &SimulationConfig) {
        let rate = self.mutation_rate;
        let normal = Normal::new(0.0f32, sim.mutation_stddev).unwrap();

        // Helper: with probability `rate`, add a small Gaussian perturbation.
        fn maybe_mutate(val: &mut f32, rate: f32, normal: &Normal<f32>, rng: &mut impl Rng) {
            if rng.random::<f32>() < rate {
                *val += normal.sample(rng);
                *val = val.max(0.0);
            }
        }

        // Mutate receptor sensitivities
        for r in &mut self.receptors {
            maybe_mutate(&mut r.k_half, rate, &normal, rng);
            maybe_mutate(&mut r.n_hill, rate, &normal, rng);
            r.n_hill = r
                .n_hill
                .clamp(sim.hill_exponent_clamp_low, sim.hill_exponent_clamp_high);
            maybe_mutate(&mut r.gain, rate, &normal, rng);
        }

        // Mutate transport rates (+ rare structural: change which species)
        for t in &mut self.transport {
            maybe_mutate(&mut t.uptake_rate, rate, &normal, rng);
            maybe_mutate(&mut t.secrete_rate, rate, &normal, rng);
            if rng.random::<f32>() < rate * sim.structural_mutation_rate_mult {
                t.ext_species = rng.random_range(0..S_EXT as u8);
            }
            if rng.random::<f32>() < rate * sim.structural_mutation_rate_mult {
                t.int_species = rng.random_range(0..M_INT as u8);
            }
        }

        // Mutate reaction kinetics (+ rare structural: gene duplication model)
        //
        // Gene duplication (Ohno, 1970) is the dominant mode of metabolic
        // innovation in real microbes: copy an existing working enzyme, then
        // let the copy diverge. We model this by copying substrate/product/
        // catalyst from a randomly-chosen ACTIVE reaction, rather than
        // assembling random species indices from scratch.
        //
        // First, collect indices of active reactions for duplication source.
        let active_indices: Vec<usize> = self
            .reactions
            .iter()
            .enumerate()
            .filter(|(_, r)| r.v_max.abs() > sim.active_reaction_threshold)
            .map(|(i, _)| i)
            .collect();

        for i in 0..R_MAX {
            maybe_mutate(&mut self.reactions[i].k_m, rate, &normal, rng);
            maybe_mutate(&mut self.reactions[i].v_max, rate, &normal, rng);
            maybe_mutate(&mut self.reactions[i].k_cat, rate, &normal, rng);

            // Structural mutation: copy topology from an existing active reaction
            // (gene duplication + divergence). Falls back to random if no active
            // reactions exist (shouldn't happen in practice).
            if rng.random::<f32>() < rate * sim.structural_mutation_rate_mult {
                if let Some(&donor) =
                    active_indices.get(rng.random_range(0..active_indices.len().max(1)))
                {
                    if donor != i {
                        // don't copy from self
                        self.reactions[i].substrate = self.reactions[donor].substrate;
                    }
                } else {
                    self.reactions[i].substrate = rng.random_range(0..M_INT as u8);
                }
            }
            if rng.random::<f32>() < rate * sim.structural_mutation_rate_mult {
                if let Some(&donor) =
                    active_indices.get(rng.random_range(0..active_indices.len().max(1)))
                {
                    if donor != i {
                        self.reactions[i].product = self.reactions[donor].product;
                    }
                } else {
                    self.reactions[i].product = rng.random_range(0..M_INT as u8);
                }
            }
            if rng.random::<f32>() < rate * sim.structural_mutation_rate_mult {
                if let Some(&donor) =
                    active_indices.get(rng.random_range(0..active_indices.len().max(1)))
                {
                    if donor != i {
                        self.reactions[i].catalyst = self.reactions[donor].catalyst;
                    }
                } else {
                    self.reactions[i].catalyst = rng.random_range(0..M_INT as u8);
                }
            }
        }

        // Mutate effector thresholds and rates
        for e in &mut self.effectors {
            maybe_mutate(&mut e.threshold, rate, &normal, rng);
            maybe_mutate(&mut e.rate, rate, &normal, rng);
        }

        // Mutate fate decision thresholds and cell cycle timing
        maybe_mutate(&mut self.fate.division_energy, rate, &normal, rng);
        maybe_mutate(&mut self.fate.death_energy, rate, &normal, rng);
        maybe_mutate(&mut self.fate.quiescence_energy, rate, &normal, rng);
        maybe_mutate(&mut self.fate.division_prep_ticks, rate, &normal, rng);
        self.fate.division_prep_ticks = self.fate.division_prep_ticks.max(1.0); // minimum 1 tick
        maybe_mutate(&mut self.hgt_propensity, rate, &normal, rng);

        // Meta-evolution: the mutation rate itself can mutate.
        // This happens at a fixed rate (not gated by the current mutation_rate)
        // to prevent runaway suppression of evolvability.
        if rng.random::<f32>() < sim.meta_mutation_rate {
            self.mutation_rate += normal.sample(rng) * sim.meta_mutation_rate;
            self.mutation_rate = self
                .mutation_rate
                .clamp(sim.meta_mutation_clamp_low, sim.meta_mutation_clamp_high);
        }
    }
}
