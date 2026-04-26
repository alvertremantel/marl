//! `marl-format` - Binary schema shared by the MARL engine and viewer.
//!
//! This crate owns the durable on-disk format constants, metadata structs,
//! and validation helpers for the engine's binary field/cell dumps and
//! `run_meta.json`.
//!
//! # Constants
//!
//! - [`ENDIANNESS`]: data endianness (`"little"`)
//! - [`FIELD_DTYPE`]: field element dtype (`"f32"`)
//! - [`FIELD_LAYOUT`]: field memory layout (`"z_y_x_species"`)
//! - [`CELL_RECORD_STRIDE`]: size of one packed cell record in bytes (`25`)
//!
//! # Types
//!
//! - [`RunMeta`]: serializable metadata written to `run_meta.json`
//! - [`ViewerCellRecord`]: packed 25-byte cell record for binary cell dumps
//! - [`FormatError`]: validation error type

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Data endianness for binary field and cell dumps.
pub const ENDIANNESS: &str = "little";

/// Field element dtype as written to disk.
pub const FIELD_DTYPE: &str = "f32";

/// Field memory layout: outer dimension is z, then y, then x, then species.
pub const FIELD_LAYOUT: &str = "z_y_x_species";

/// Size of one packed cell record in bytes.
pub const CELL_RECORD_STRIDE: u32 = 25;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Validation error returned by [`RunMeta::validate`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatError {
    pub message: String,
}

impl FormatError {
    fn new(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
        }
    }
}

impl std::fmt::Display for FormatError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FormatError: {}", self.message)
    }
}

impl std::error::Error for FormatError {}

// ---------------------------------------------------------------------------
// Run metadata
// ---------------------------------------------------------------------------

/// Metadata written to `run_meta.json` at startup when binary output is enabled.
///
/// All field names and value shapes are preserved for compatibility with
/// downstream binary-dump consumers.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RunMeta {
    #[serde(rename = "grid_x")]
    pub grid_x: u32,
    #[serde(rename = "grid_y")]
    pub grid_y: u32,
    #[serde(rename = "grid_z")]
    pub grid_z: u32,
    #[serde(rename = "s_ext")]
    pub s_ext: u32,
    #[serde(default, rename = "m_int")]
    pub m_int: u32,
    #[serde(rename = "field_dtype")]
    pub field_dtype: String,
    #[serde(rename = "field_layout")]
    pub field_layout: String,
    #[serde(rename = "field_byte_len")]
    pub field_byte_len: u64,
    #[serde(rename = "cell_record_stride")]
    pub cell_record_stride: u32,
    #[serde(rename = "endianness")]
    pub endianness: String,
    #[serde(rename = "write_binary_field")]
    pub write_binary_field: bool,
    #[serde(rename = "write_binary_cells")]
    pub write_binary_cells: bool,
}

impl RunMeta {
    /// Build a new `RunMeta` from grid dimensions, species counts, and output toggles.
    pub fn new(
        grid_x: u32,
        grid_y: u32,
        grid_z: u32,
        s_ext: u32,
        m_int: u32,
        write_binary_field: bool,
        write_binary_cells: bool,
    ) -> Self {
        let field_byte_len = field_byte_len(grid_x, grid_y, grid_z, s_ext).unwrap_or(0);
        Self {
            grid_x,
            grid_y,
            grid_z,
            s_ext,
            m_int,
            field_dtype: FIELD_DTYPE.to_string(),
            field_layout: FIELD_LAYOUT.to_string(),
            field_byte_len,
            cell_record_stride: CELL_RECORD_STRIDE,
            endianness: ENDIANNESS.to_string(),
            write_binary_field,
            write_binary_cells,
        }
    }

    /// Validate that stored schema constants match the loaded metadata.
    ///
    /// Returns `Ok(())` if all constants are consistent, or a [`FormatError`]
    /// describing the first mismatch.
    pub fn validate(&self) -> Result<(), FormatError> {
        if self.endianness != ENDIANNESS {
            return Err(FormatError::new(format!(
                "endianness mismatch: expected {}, got {}",
                ENDIANNESS, self.endianness
            )));
        }
        if self.field_dtype != FIELD_DTYPE {
            return Err(FormatError::new(format!(
                "field_dtype mismatch: expected {}, got {}",
                FIELD_DTYPE, self.field_dtype
            )));
        }
        if self.field_layout != FIELD_LAYOUT {
            return Err(FormatError::new(format!(
                "field_layout mismatch: expected {}, got {}",
                FIELD_LAYOUT, self.field_layout
            )));
        }
        if self.cell_record_stride != CELL_RECORD_STRIDE {
            return Err(FormatError::new(format!(
                "cell_record_stride mismatch: expected {}, got {}",
                CELL_RECORD_STRIDE, self.cell_record_stride
            )));
        }
        if let Some(expected_len) =
            field_byte_len(self.grid_x, self.grid_y, self.grid_z, self.s_ext)
        {
            if self.field_byte_len != expected_len {
                return Err(FormatError::new(format!(
                    "field_byte_len mismatch: expected {}, got {}",
                    expected_len, self.field_byte_len
                )));
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Field byte length helper
// ---------------------------------------------------------------------------

/// Compute the expected byte length of a raw field dump.
///
/// Returns `None` if any argument is zero (avoids panics from multiply-add).
#[inline]
pub fn field_byte_len(grid_x: u32, grid_y: u32, grid_z: u32, s_ext: u32) -> Option<u64> {
    if grid_x == 0 || grid_y == 0 || grid_z == 0 || s_ext == 0 {
        return None;
    }
    let count = u64::from(grid_x)
        .checked_mul(u64::from(grid_y))?
        .checked_mul(u64::from(grid_z))?
        .checked_mul(u64::from(s_ext))?;
    Some(count.checked_mul(4)?)
}

// ---------------------------------------------------------------------------
// Packed viewer cell record
// ---------------------------------------------------------------------------

/// Packed 25-byte cell record written to `tick_<T>.cells.bin`.
///
/// Each record contains position (3 × f32), lineage_id (u64), starter_type (u8),
/// and energy (f32) in little-endian byte order.
///
/// # Layout
///
/// | Offset | Size | Type       | Name          |
/// |--------|------|------------|---------------|
/// | 0      | 12   | f32[3]     | pos           |
/// | 12     | 8    | u64        | lineage_id    |
/// | 20     | 1    | u8         | starter_type  |
/// | 21     | 4    | f32        | energy        |
/// | 25     | —    | —          | total = 25    |
///
/// The struct is `#[repr(C, packed)]` to match the binary file layout.
/// Do not take references to multi-byte fields of a packed record;
/// copy values by value instead.
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct ViewerCellRecord {
    /// World position of the cell (x, y, z).
    pub pos: [f32; 3],
    /// Unique lineage identifier.
    pub lineage_id: u64,
    /// Starter metabolism type encoded as a small integer.
    pub starter_type: u8,
    /// Current energy reserve.
    pub energy: f32,
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_field_byte_len_basic() {
        // 128 * 128 * 64 * 12 * 4 bytes
        let len = field_byte_len(128, 128, 64, 12);
        assert_eq!(len, Some(50_331_648));
    }

    #[test]
    fn test_field_byte_len_zero_arg() {
        assert_eq!(field_byte_len(0, 128, 64, 12), None);
        assert_eq!(field_byte_len(128, 0, 64, 12), None);
        assert_eq!(field_byte_len(128, 128, 0, 12), None);
        assert_eq!(field_byte_len(128, 128, 64, 0), None);
    }

    #[test]
    fn test_field_byte_len_overflow() {
        // very large values should not panic
        assert_eq!(field_byte_len(u32::MAX, u32::MAX, u32::MAX, u32::MAX), None);
    }

    #[test]
    fn test_run_meta_new_round_trip() {
        let meta = RunMeta::new(128, 128, 64, 12, 8, true, true);
        assert_eq!(meta.grid_x, 128);
        assert_eq!(meta.grid_y, 128);
        assert_eq!(meta.grid_z, 64);
        assert_eq!(meta.s_ext, 12);
        assert_eq!(meta.m_int, 8);
        assert_eq!(meta.field_dtype, "f32");
        assert_eq!(meta.field_layout, "z_y_x_species");
        assert_eq!(meta.endianness, "little");
        assert_eq!(meta.cell_record_stride, 25);
        assert!(meta.write_binary_field);
        assert!(meta.write_binary_cells);
        assert_eq!(meta.field_byte_len, 50_331_648);
    }

    #[test]
    fn test_run_meta_validate_ok() {
        let meta = RunMeta::new(128, 128, 64, 12, 8, true, true);
        assert!(meta.validate().is_ok());
    }

    #[test]
    fn test_run_meta_validate_bad_endianness() {
        let mut meta = RunMeta::new(128, 128, 64, 12, 8, true, true);
        meta.endianness = "big".to_string();
        let err = meta.validate().unwrap_err();
        assert!(err.message.contains("endianness"));
    }

    #[test]
    fn test_run_meta_validate_bad_layout() {
        let mut meta = RunMeta::new(128, 128, 64, 12, 8, true, true);
        meta.field_layout = "x_y_z_species".to_string();
        let err = meta.validate().unwrap_err();
        assert!(err.message.contains("field_layout"));
    }

    #[test]
    fn test_run_meta_validate_bad_byte_len() {
        let mut meta = RunMeta::new(128, 128, 64, 12, 8, true, true);
        meta.field_byte_len = 1;
        let err = meta.validate().unwrap_err();
        assert!(err.message.contains("field_byte_len"));
    }

    #[test]
    fn test_run_meta_serde_round_trip() {
        let meta = RunMeta::new(128, 128, 64, 12, 8, true, false);
        let json = serde_json::to_string_pretty(&meta).unwrap();
        let back: RunMeta = serde_json::from_str(&json).unwrap();
        assert_eq!(meta.grid_x, back.grid_x);
        assert_eq!(meta.grid_y, back.grid_y);
        assert_eq!(meta.grid_z, back.grid_z);
        assert_eq!(meta.s_ext, back.s_ext);
        assert_eq!(meta.m_int, back.m_int);
        assert_eq!(meta.write_binary_field, back.write_binary_field);
        assert_eq!(meta.write_binary_cells, back.write_binary_cells);
        // field names match JSON
        assert!(json.contains("\"grid_x\""));
        assert!(json.contains("\"field_dtype\""));
        assert!(json.contains("\"cell_record_stride\""));
    }

    #[test]
    fn test_viewer_cell_record_size() {
        use std::mem::size_of;
        assert_eq!(size_of::<ViewerCellRecord>(), 25);
    }

    #[test]
    fn test_constants() {
        assert_eq!(ENDIANNESS, "little");
        assert_eq!(FIELD_DTYPE, "f32");
        assert_eq!(FIELD_LAYOUT, "z_y_x_species");
        assert_eq!(CELL_RECORD_STRIDE, 25);
    }
}
