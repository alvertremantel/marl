use std::env;
use std::path::PathBuf;

pub(crate) const DEFAULT_OUTPUT_DIR: &str = "output/run_128x128x64";

#[derive(Debug, Clone)]
pub(crate) struct ViewerArgs {
    pub(crate) output_dir: PathBuf,
    pub(crate) tick: u64,
    pub(crate) species: u32,
    pub(crate) exposure: f32,
    pub(crate) density_scale: f32,
    pub(crate) steps: u32,
}

impl ViewerArgs {
    pub(crate) fn parse() -> Result<Self, String> {
        let mut output_dir: Option<PathBuf> = None;
        let mut tick = 0;
        let mut species = 1;
        let mut exposure: f32 = 18.0;
        let mut density_scale: f32 = 2.0;
        let mut steps = 160;

        let mut args = env::args().skip(1);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "-h" | "--help" => return Err(usage()),
                "--dir" => {
                    output_dir = Some(PathBuf::from(next_value(&mut args, "--dir")?));
                }
                "--tick" => {
                    tick = parse_value(&mut args, "--tick")?;
                }
                "--species" => {
                    species = parse_value(&mut args, "--species")?;
                }
                "--exposure" => {
                    exposure = parse_value(&mut args, "--exposure")?;
                }
                "--scale" => {
                    density_scale = parse_value(&mut args, "--scale")?;
                }
                "--steps" => {
                    steps = parse_value(&mut args, "--steps")?;
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

        Ok(Self {
            output_dir: output_dir.unwrap_or_else(|| PathBuf::from(DEFAULT_OUTPUT_DIR)),
            tick,
            species,
            exposure,
            density_scale,
            steps,
        })
    }
}

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

pub(crate) fn usage() -> String {
    format!(
        "Usage: cargo run -p marl-viewer-rs --release -- [output-dir] [options]\n\n\
         Options:\n\
           --dir <path>       Output directory containing run_meta.json\n\
           --tick <n>         Field tick to load (default: 0)\n\
           --species <n>      Species index to render (default: 1)\n\
           --exposure <f>     Raymarch opacity multiplier (default: 18.0)\n\
           --scale <f>        Concentration-to-density scale (default: 2.0)\n\
           --steps <n>        Raymarch samples through z (default: 160)\n\n\
         If no output directory is supplied, `{DEFAULT_OUTPUT_DIR}` is used."
    )
}
