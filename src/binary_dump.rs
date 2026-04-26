//! Raw binary outputs for high-throughput viewer ingestion.

use crate::cell::CellState;
use crate::config::*;
use crate::field::Field;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::mem;
use std::path::Path;

#[cfg(not(target_endian = "little"))]
compile_error!("binary dumps declare little-endian layout and require a little-endian target");

// SAFETY NOTE: `ViewerCell` is packed for a stable viewer-facing byte layout.
// Do not take references to multi-byte fields of this type; copy values out.
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct ViewerCell {
    pub pos: [f32; 3],
    pub lineage_id: u64,
    pub starter_type: u8,
    pub energy: f32,
}

impl From<&CellState> for ViewerCell {
    fn from(cell: &CellState) -> Self {
        Self {
            pos: [cell.pos[0] as f32, cell.pos[1] as f32, cell.pos[2] as f32],
            lineage_id: cell.lineage_id,
            starter_type: cell.starter_type,
            energy: cell.internal[0],
        }
    }
}

fn as_bytes<T>(slice: &[T]) -> &[u8] {
    let len = mem::size_of_val(slice);
    let ptr = slice.as_ptr().cast::<u8>();
    // SAFETY: `slice` is valid for `len` bytes, and u8 has alignment 1.
    unsafe { std::slice::from_raw_parts(ptr, len) }
}

pub fn write_field_dump(field: &Field, tick: u64, out_dir: &str) -> std::io::Result<()> {
    fs::create_dir_all(out_dir)?;
    let path = Path::new(out_dir).join(format!("tick_{}.field.bin", tick));
    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);
    writer.write_all(as_bytes(&field.data))?;
    writer.flush()
}

pub fn write_cell_dump(cells: &[CellState], tick: u64, out_dir: &str) -> std::io::Result<()> {
    fs::create_dir_all(out_dir)?;
    let viewer_cells: Vec<ViewerCell> = cells.iter().map(ViewerCell::from).collect();
    let path = Path::new(out_dir).join(format!("tick_{}.cells.bin", tick));
    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);
    writer.write_all(as_bytes(&viewer_cells))?;
    writer.flush()
}

pub fn write_run_meta(out: &OutputConfig) -> std::io::Result<()> {
    fs::create_dir_all(&out.output_dir)?;
    let path = Path::new(&out.output_dir).join("run_meta.json");
    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);
    let field_count = GRID_X * GRID_Y * GRID_Z * S_EXT;
    let field_byte_len = field_count * mem::size_of::<f32>();
    let cell_record_stride = mem::size_of::<ViewerCell>();

    writeln!(writer, "{{")?;
    writeln!(writer, "  \"grid_x\": {},", GRID_X)?;
    writeln!(writer, "  \"grid_y\": {},", GRID_Y)?;
    writeln!(writer, "  \"grid_z\": {},", GRID_Z)?;
    writeln!(writer, "  \"s_ext\": {},", S_EXT)?;
    writeln!(
        writer,
        "  \"snapshot_interval\": {},",
        out.snapshot_interval
    )?;
    writeln!(writer, "  \"max_ticks\": {},", out.max_ticks)?;
    writeln!(
        writer,
        "  \"write_binary_field\": {},",
        out.write_binary_field
    )?;
    writeln!(
        writer,
        "  \"write_binary_cells\": {},",
        out.write_binary_cells
    )?;
    writeln!(writer, "  \"endianness\": \"little\",")?;
    writeln!(writer, "  \"field_dtype\": \"f32\",")?;
    writeln!(writer, "  \"field_layout\": \"z_y_x_species\",")?;
    writeln!(writer, "  \"field_count\": {},", field_count)?;
    writeln!(writer, "  \"field_byte_len\": {},", field_byte_len)?;
    writeln!(writer, "  \"field_file_pattern\": \"tick_<T>.field.bin\",")?;
    writeln!(writer, "  \"cell_file_pattern\": \"tick_<T>.cells.bin\",")?;
    writeln!(writer, "  \"cell_record_stride\": {},", cell_record_stride)?;
    writeln!(writer, "  \"cell_header_byte_len\": 0,")?;
    writeln!(
        writer,
        "  \"cell_count_source\": \"file_size_divided_by_cell_record_stride\","
    )?;
    writeln!(writer, "  \"cell_pos_units\": \"grid_voxel_indices\",")?;
    writeln!(
        writer,
        "  \"cell_record_layout\": \"pos:f32[3],lineage_id:u64,starter_type:u8,energy:f32\""
    )?;
    writeln!(writer, "}}")?;
    writer.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewer_cell_record_layout_is_stable() {
        assert_eq!(mem::size_of::<ViewerCell>(), 25);
        assert_eq!(mem::align_of::<ViewerCell>(), 1);
    }

    #[test]
    fn viewer_cell_record_bytes_match_metadata_layout() {
        let cell = ViewerCell {
            pos: [1.0, 2.0, 3.0],
            lineage_id: 0x0102_0304_0506_0708,
            starter_type: 2,
            energy: 4.5,
        };
        let bytes = as_bytes(std::slice::from_ref(&cell));

        assert_eq!(bytes.len(), 25);
        assert_eq!(&bytes[0..4], &1.0f32.to_le_bytes());
        assert_eq!(&bytes[4..8], &2.0f32.to_le_bytes());
        assert_eq!(&bytes[8..12], &3.0f32.to_le_bytes());
        assert_eq!(&bytes[12..20], &0x0102_0304_0506_0708u64.to_le_bytes());
        assert_eq!(bytes[20], 2);
        assert_eq!(&bytes[21..25], &4.5f32.to_le_bytes());
    }
}
