# 002 — Tick Timescale Abstraction: 1 Tick ≈ 1 Day

**Date:** 2026-03-15  
**Status:** Accepted

## Context

Reaction-diffusion systems have a numerical stability constraint (CFL/von Neumann condition) relating timestep size, voxel size, and diffusivity: Δt ≤ Δx² / 6D in 3D. Choosing a large timestep (1 day) requires diffusivity D to be small enough to satisfy this, or an implicit integration scheme. The biological regime being modeled (microbial mat / biofilm in a hydrogel-like extracellular matrix) has gel-phase diffusion coefficients 10–100× slower than free solution, which naturally satisfies this constraint for day-scale timesteps without implicit integration.

## Options Considered

- **Option A — Short tick (seconds/minutes), many ticks per rendered frame:** Accurate molecular timescales, but evolution requires enormous tick counts before anything interesting happens. Simulation fidelity vs. observability tradeoff is poor.
- **Option B — Long tick (1 day), timestep matches biological meaningful unit:** Evolution events (reproduction, mutation) happen at reasonable tick frequencies. Diffusion parameters set to gel-phase values satisfy stability. Brownian motion and advection are correctly negligible at this timescale.
- **Option C — Variable tick / subcycle:** Field updates subcycled at short intervals within a longer "biological day." More accurate but significantly more complex.

## Decision

Option B. 1 tick ≈ 1 day. Diffusivity parameters are set to gel-phase values. Explicit forward-Euler integration is used for the field update; stability is guaranteed by the physical parameter regime rather than by numerical scheme complexity.

## Consequences

- Forward-Euler field integration is valid and simple to implement on GPU.
- Diffusion gradients at the 10–100 voxel scale establish within tens to hundreds of ticks — observable at interactive simulation speeds.
- Evolution events (reproduction ≈ cell division, which in fast bacteria is ~20min but in a nutrient-limited mat is realistically 1–7 days) occur at 1–7 tick intervals for the fastest-dividing cells. Population-scale evolution is observable within thousands of ticks.
- Molecular timescale phenomena (signal transduction kinetics faster than 1 day) are not representable — this is a known and accepted limitation.
- Optional: tick granularity can be exposed as a simulation parameter (Δt = 1 day default, reducible to 0.25 day for higher fidelity at cost of tick rate).

## Notes

A quarter-day or eighth-day option is worth exposing as a parameter for users who want finer chemical dynamics. The stability constraint is just Δt ≤ Δx² / 6D — at smaller Δt the constraint is easier to satisfy, not harder.
