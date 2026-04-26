use std::error::Error;
use std::fs;

use marl_format::RunMeta;

use crate::args::ViewerArgs;

pub(crate) struct FieldPayload {
    pub(crate) meta: RunMeta,
    pub(crate) bytes: Vec<u8>,
    pub(crate) tick: u64,
    pub(crate) species: u32,
    pub(crate) exposure: f32,
    pub(crate) density_scale: f32,
    pub(crate) steps: u32,
}

pub(crate) fn load_field(args: &ViewerArgs) -> Result<FieldPayload, Box<dyn Error>> {
    let meta_path = args.output_dir.join("run_meta.json");
    let meta_bytes =
        fs::read(&meta_path).map_err(|e| format!("failed to read {}: {e}", meta_path.display()))?;
    let meta: RunMeta = serde_json::from_slice(&meta_bytes)
        .map_err(|e| format!("failed to parse {}: {e}", meta_path.display()))?;

    meta.validate()
        .map_err(|e| format!("invalid run_meta.json: {e}"))?;
    if args.species >= meta.s_ext {
        return Err(format!(
            "species {} is out of range for {} external species",
            args.species, meta.s_ext
        )
        .into());
    }

    let field_path = args
        .output_dir
        .join(format!("tick_{}.field.bin", args.tick));
    let bytes = fs::read(&field_path)
        .map_err(|e| format!("failed to read {}: {e}", field_path.display()))?;
    if bytes.len() as u64 != meta.field_byte_len {
        return Err(format!(
            "{} has {} bytes, expected {} from run_meta.json",
            field_path.display(),
            bytes.len(),
            meta.field_byte_len
        )
        .into());
    }

    Ok(FieldPayload {
        meta,
        bytes,
        tick: args.tick,
        species: args.species,
        exposure: args.exposure,
        density_scale: args.density_scale,
        steps: args.steps,
    })
}
