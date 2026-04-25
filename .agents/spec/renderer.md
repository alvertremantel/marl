# Module: renderer

## Purpose

Visualizes the chemical field and cell population state. Decoupled from the simulation tick loop — runs at display framerate independently of simulation rate. Provides interactive slice-plane navigation through the 3D volume and per-species color mapping for chemical concentrations.

## Public Interface

```
renderer_frame(
    field:      [N³ × S] float buffer,  // chemical concentrations (read-only)
    cells:      CellRegistry,           // sparse cell positions and state
    light:      [N³] float buffer,      // light availability field
    view:       ViewState,              // slice plane, camera, active species
) -> framebuffer
```

## Visualization Modes (planned)

- **Chemical slice:** 2D cross-section of the 3D field at a user-controlled Z (horizontal), X, or Y plane. Each chemical species mapped to a distinct color channel; concentration mapped to intensity. Multiple species overlaid with per-species opacity.
- **Cell overlay:** Live cell positions rendered as colored points on the slice plane. Color encodes ruleset cluster ID (derived from parameter-space clustering of live rulesets) — allows visual tracking of lineage diversity.
- **Light field overlay:** Light availability as a grayscale gradient, toggleable.
- **Diversity metric:** Real-time display of ruleset diversity (e.g. mean pairwise parameter distance across population). Observable proxy for evolutionary state.

## Dependencies

- Reads: [[Modules/field-update]] output, [[Modules/cell-agent]] registry, [[Modules/light-engine]] output
- No write-back to any simulation state — purely observational

## Known Limitations / Planned

- Volumetric rendering (ray-marched 3D view rather than 2D slices) is a stretch goal. 2D slices are sufficient for initial research use.
- Ruleset cluster coloring requires a lightweight online clustering step (k-means or similar) on the live ruleset parameter vectors. Frequency and cost TBD.
- Export: field snapshots to HDF5 or similar for offline analysis. High priority for research use — a paper needs reproducible quantitative outputs, not just screenshots.
