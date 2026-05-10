use std::env;
use std::path::PathBuf;

pub(crate) const DEFAULT_OUTPUT_DIR: &str = "output/run_128x128x64";

// ---------------------------------------------------------------------------
// Enums for new viewer modes
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ViewMode {
    Iso,
    Top,
}

impl ViewMode {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            ViewMode::Iso => "iso",
            ViewMode::Top => "top",
        }
    }

    pub(crate) fn all() -> [ViewMode; 2] {
        [ViewMode::Iso, ViewMode::Top]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CellMode {
    Off,
    Starter,
    Energy,
}

impl CellMode {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            CellMode::Off => "off",
            CellMode::Starter => "starter",
            CellMode::Energy => "energy",
        }
    }

    pub(crate) fn all() -> [CellMode; 3] {
        [CellMode::Off, CellMode::Starter, CellMode::Energy]
    }
}

// ---------------------------------------------------------------------------
// ViewerArgs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub(crate) struct ViewerArgs {
    pub(crate) output_dir: PathBuf,
    pub(crate) tick: u64,
    pub(crate) species: u32,
    pub(crate) exposure: f32,
    pub(crate) density_scale: f32,
    pub(crate) steps: u32,
    pub(crate) view_mode: ViewMode,
    pub(crate) cell_mode: CellMode,
    pub(crate) cell_alpha: f32,
}

impl ViewerArgs {
    pub(crate) fn parse() -> Result<Self, String> {
        Self::parse_from(env::args().skip(1))
    }

    pub(crate) fn parse_from<I>(args: I) -> Result<Self, String>
    where
        I: IntoIterator<Item = String>,
    {
        let mut output_dir: Option<PathBuf> = None;
        let mut tick = 0;
        let mut species = 1;
        let mut exposure: f32 = 18.0;
        let mut density_scale: f32 = 2.0;
        let mut steps = 160;
        let mut view_mode = ViewMode::Iso;
        let mut cell_mode = CellMode::Starter;
        let mut cell_alpha: f32 = 0.95;

        let mut iter = args.into_iter();
        while let Some(arg) = iter.next() {
            match arg.as_str() {
                "-h" | "--help" => return Err(usage()),
                "--dir" => {
                    output_dir = Some(PathBuf::from(next_value(&mut iter, "--dir")?));
                }
                "--tick" => {
                    tick = parse_value(&mut iter, "--tick")?;
                }
                "--species" => {
                    species = parse_value(&mut iter, "--species")?;
                }
                "--exposure" => {
                    exposure = parse_value(&mut iter, "--exposure")?;
                }
                "--scale" => {
                    density_scale = parse_value(&mut iter, "--scale")?;
                }
                "--steps" => {
                    steps = parse_value(&mut iter, "--steps")?;
                }
                "--view" => {
                    view_mode = parse_view_mode(&mut iter)?;
                }
                "--cells" => {
                    cell_mode = parse_cell_mode(&mut iter)?;
                }
                "--cell-alpha" => {
                    cell_alpha = parse_value(&mut iter, "--cell-alpha")?;
                }
                _ if arg.starts_with('-') => {
                    return Err(format!("unknown argument: {arg}\n\n{}", usage()));
                }
                _ => {
                    if output_dir.is_some() {
                        return Err(format!("unexpected extra path: {arg}\n\n{}", usage()));
                    }
                    output_dir = Some(PathBuf::from(arg));
                }
            }
        }

        if steps == 0 {
            return Err("--steps must be greater than zero".to_string());
        }
        if !exposure.is_finite() || exposure <= 0.0 {
            return Err("--exposure must be a positive finite number".to_string());
        }
        if !density_scale.is_finite() || density_scale <= 0.0 {
            return Err("--scale must be a positive finite number".to_string());
        }
        if !cell_alpha.is_finite() || cell_alpha <= 0.0 || cell_alpha > 1.0 {
            return Err("--cell-alpha must be in the range (0, 1]".to_string());
        }

        Ok(Self {
            output_dir: output_dir.unwrap_or_else(|| PathBuf::from(DEFAULT_OUTPUT_DIR)),
            tick,
            species,
            exposure,
            density_scale,
            steps,
            view_mode,
            cell_mode,
            cell_alpha,
        })
    }
}

// ---------------------------------------------------------------------------
// Parsing helpers
// ---------------------------------------------------------------------------

pub(crate) fn next_value(
    args: &mut impl Iterator<Item = String>,
    flag: &str,
) -> Result<String, String> {
    args.next()
        .ok_or_else(|| format!("missing value for {flag}\n\n{}", usage()))
}

pub(crate) fn parse_value<T: std::str::FromStr>(
    args: &mut impl Iterator<Item = String>,
    flag: &str,
) -> Result<T, String> {
    let raw = next_value(args, flag)?;
    raw.parse()
        .map_err(|_| format!("invalid value for {flag}: {raw}"))
}

fn parse_view_mode(args: &mut impl Iterator<Item = String>) -> Result<ViewMode, String> {
    let raw = next_value(args, "--view")?;
    match raw.as_str() {
        "iso" => Ok(ViewMode::Iso),
        "top" => Ok(ViewMode::Top),
        _ => Err(format!(
            "invalid value for --view: {raw} (expected 'iso' or 'top')\n\n{}",
            usage()
        )),
    }
}

fn parse_cell_mode(args: &mut impl Iterator<Item = String>) -> Result<CellMode, String> {
    let raw = next_value(args, "--cells")?;
    match raw.as_str() {
        "off" => Ok(CellMode::Off),
        "starter" => Ok(CellMode::Starter),
        "energy" => Ok(CellMode::Energy),
        _ => Err(format!(
            "invalid value for --cells: {raw} (expected 'off', 'starter', or 'energy')\n\n{}",
            usage()
        )),
    }
}

pub(crate) fn usage() -> String {
    format!(
        "Usage: cargo run -p marl-viewer-rs --release -- [output-dir] [options]\n\n\
         Options:\n\
           --dir <path>         Output directory containing run_meta.json\n\
           --tick <n>           Tick to load (default: 0)\n\
           --species <n>        External species index to render (default: 1)\n\
           --exposure <f>       Raymarch opacity multiplier (default: 18.0)\n\
           --scale <f>          Concentration-to-density scale (default: 2.0)\n\
           --steps <n>          Raymarch samples (default: 160)\n\
           --view <iso|top>     View mode: isometric (default) or top-down\n\
           --cells <off|starter|energy>  Cell rendering mode (default: starter)\n\
           --cell-alpha <f>     Opacity of cell voxel markers (default: 0.95)\n\n\
         For legacy field-only top-down rendering:\n\
           --view top --species 1 --cells off\n\n\
         If no output directory is supplied, `{DEFAULT_OUTPUT_DIR}` is used."
    )
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn args(v: &[&str]) -> impl Iterator<Item = String> {
        v.iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
            .into_iter()
    }

    #[test]
    fn defaults() {
        let parsed = ViewerArgs::parse_from(std::iter::empty::<String>()).unwrap();
        assert_eq!(parsed.view_mode, ViewMode::Iso);
        assert_eq!(parsed.cell_mode, CellMode::Starter);
        assert!((parsed.cell_alpha - 0.95).abs() < 0.001);
        assert_eq!(parsed.tick, 0);
        assert_eq!(parsed.species, 1);
        assert_eq!(parsed.steps, 160);
    }

    #[test]
    fn view_iso() {
        let parsed = ViewerArgs::parse_from(args(&["--view", "iso"])).unwrap();
        assert_eq!(parsed.view_mode, ViewMode::Iso);
    }

    #[test]
    fn view_top() {
        let parsed = ViewerArgs::parse_from(args(&["--view", "top"])).unwrap();
        assert_eq!(parsed.view_mode, ViewMode::Top);
    }

    #[test]
    fn view_invalid() {
        let err = ViewerArgs::parse_from(args(&["--view", "side"])).unwrap_err();
        assert!(err.contains("invalid value for --view"), "got: {err}");
    }

    #[test]
    fn cells_off() {
        let parsed = ViewerArgs::parse_from(args(&["--cells", "off"])).unwrap();
        assert_eq!(parsed.cell_mode, CellMode::Off);
    }

    #[test]
    fn cells_starter() {
        let parsed = ViewerArgs::parse_from(args(&["--cells", "starter"])).unwrap();
        assert_eq!(parsed.cell_mode, CellMode::Starter);
    }

    #[test]
    fn cells_energy() {
        let parsed = ViewerArgs::parse_from(args(&["--cells", "energy"])).unwrap();
        assert_eq!(parsed.cell_mode, CellMode::Energy);
    }

    #[test]
    fn cells_invalid() {
        let err = ViewerArgs::parse_from(args(&["--cells", "lineage"])).unwrap_err();
        assert!(err.contains("invalid value for --cells"), "got: {err}");
    }

    #[test]
    fn cell_alpha_valid() {
        let parsed = ViewerArgs::parse_from(args(&["--cell-alpha", "0.5"])).unwrap();
        assert!((parsed.cell_alpha - 0.5).abs() < 0.001);
    }

    #[test]
    fn cell_alpha_zero() {
        let err = ViewerArgs::parse_from(args(&["--cell-alpha", "0"])).unwrap_err();
        assert!(err.contains("--cell-alpha"), "got: {err}");
    }

    #[test]
    fn cell_alpha_negative() {
        let err = ViewerArgs::parse_from(args(&["--cell-alpha", "-0.1"])).unwrap_err();
        assert!(err.contains("--cell-alpha"), "got: {err}");
    }

    #[test]
    fn cell_alpha_above_one() {
        let err = ViewerArgs::parse_from(args(&["--cell-alpha", "1.5"])).unwrap_err();
        assert!(err.contains("--cell-alpha"), "got: {err}");
    }

    #[test]
    fn cell_alpha_one_exact() {
        let parsed = ViewerArgs::parse_from(args(&["--cell-alpha", "1.0"])).unwrap();
        assert!((parsed.cell_alpha - 1.0).abs() < 0.001);
    }

    #[test]
    fn legacy_top_cells_off() {
        let parsed =
            ViewerArgs::parse_from(args(&["--view", "top", "--cells", "off", "--species", "1"]))
                .unwrap();
        assert_eq!(parsed.view_mode, ViewMode::Top);
        assert_eq!(parsed.cell_mode, CellMode::Off);
        assert_eq!(parsed.species, 1);
    }

    #[test]
    fn combined_flags() {
        let parsed = ViewerArgs::parse_from(args(&[
            "some/dir",
            "--tick",
            "42",
            "--view",
            "iso",
            "--cells",
            "energy",
            "--cell-alpha",
            "0.8",
            "--species",
            "3",
            "--steps",
            "300",
            "--exposure",
            "25.0",
            "--scale",
            "3.0",
        ]))
        .unwrap();
        assert_eq!(parsed.output_dir, PathBuf::from("some/dir"));
        assert_eq!(parsed.tick, 42);
        assert_eq!(parsed.view_mode, ViewMode::Iso);
        assert_eq!(parsed.cell_mode, CellMode::Energy);
        assert!((parsed.cell_alpha - 0.8).abs() < 0.001);
        assert_eq!(parsed.species, 3);
        assert_eq!(parsed.steps, 300);
        assert!((parsed.exposure - 25.0).abs() < 0.001);
        assert!((parsed.density_scale - 3.0).abs() < 0.001);
    }

    #[test]
    fn help_returns_err_containing_usage() {
        let err = ViewerArgs::parse_from(args(&["--help"])).unwrap_err();
        assert!(err.contains("Usage:"), "got: {err}");
    }

    #[test]
    fn view_mode_debug_and_eq() {
        assert_eq!(ViewMode::Iso, ViewMode::Iso);
        assert_ne!(ViewMode::Iso, ViewMode::Top);
        assert_eq!(CellMode::Off, CellMode::Off);
        assert_ne!(CellMode::Starter, CellMode::Energy);
    }

    #[test]
    fn view_mode_as_str() {
        assert_eq!(ViewMode::Iso.as_str(), "iso");
        assert_eq!(ViewMode::Top.as_str(), "top");
    }

    #[test]
    fn cell_mode_as_str() {
        assert_eq!(CellMode::Off.as_str(), "off");
        assert_eq!(CellMode::Starter.as_str(), "starter");
        assert_eq!(CellMode::Energy.as_str(), "energy");
    }

    #[test]
    fn view_mode_all_contains_both() {
        let all = ViewMode::all();
        assert_eq!(all.len(), 2);
        assert!(all.contains(&ViewMode::Iso));
        assert!(all.contains(&ViewMode::Top));
    }

    #[test]
    fn cell_mode_all_contains_three() {
        let all = CellMode::all();
        assert_eq!(all.len(), 3);
        assert!(all.contains(&CellMode::Off));
        assert!(all.contains(&CellMode::Starter));
        assert!(all.contains(&CellMode::Energy));
    }
}
