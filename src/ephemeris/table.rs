use bevy::math::DVec2;
use std::fs::File;
use std::io::{Read, Result as IoResult};
use std::path::Path;

/// Binary ephemeris table file format constants.
const MAGIC: &[u8; 8] = b"DEOEPH1\0";
const VERSION: u32 = 1;

#[derive(Clone, Debug)]
pub struct EphemerisTable {
    pub body_id: u32,
    pub step_seconds: f64,
    pub start_t0: f64,
    pub samples: Vec<State2>,
}

#[derive(Clone, Copy, Debug)]
pub struct State2 {
    pub pos: DVec2,
    pub vel: DVec2,
}

#[derive(thiserror::Error, Debug)]
pub enum EphemerisTableError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("invalid magic header")]
    BadMagic,

    #[error("unsupported ephemeris table version {0}")]
    UnsupportedVersion(u32),

    #[error("ephemeris table body id mismatch (expected {expected}, got {got})")]
    BodyIdMismatch { expected: u32, got: u32 },

    #[error("invalid ephemeris table (empty samples)")]
    Empty,

    #[error("requested time {time} outside table range [{start}, {end}]")]
    OutOfRange { time: f64, start: f64, end: f64 },
}

impl EphemerisTable {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, EphemerisTableError> {
        let mut f = File::open(path)?;
        let mut buf = Vec::new();
        f.read_to_end(&mut buf)?;
        Self::from_bytes(&buf)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, EphemerisTableError> {
        let mut r = Cursor::new(bytes);

        let mut magic = [0u8; 8];
        r.read_exact(&mut magic)?;
        if &magic != MAGIC {
            return Err(EphemerisTableError::BadMagic);
        }

        let version = r.read_u32_le()?;
        if version != VERSION {
            return Err(EphemerisTableError::UnsupportedVersion(version));
        }

        let body_id = r.read_u32_le()?;
        let step_seconds = r.read_f64_le()?;
        let start_t0 = r.read_f64_le()?;
        let count = r.read_u32_le()? as usize;
        let _reserved = r.read_u32_le()?;

        if count == 0 {
            return Err(EphemerisTableError::Empty);
        }

        let mut samples = Vec::with_capacity(count);
        for _ in 0..count {
            let x = r.read_f64_le()?;
            let y = r.read_f64_le()?;
            let vx = r.read_f64_le()?;
            let vy = r.read_f64_le()?;
            samples.push(State2 {
                pos: DVec2::new(x, y),
                vel: DVec2::new(vx, vy),
            });
        }

        Ok(Self {
            body_id,
            step_seconds,
            start_t0,
            samples,
        })
    }

    pub fn start_time(&self) -> f64 {
        self.start_t0
    }

    pub fn end_time(&self) -> f64 {
        self.start_t0 + self.step_seconds * (self.samples.len() as f64 - 1.0)
    }

    /// Interpolate state at `t` using cubic Hermite interpolation.
    ///
    /// Requires that the table includes both position and velocity for each sample.
    pub fn sample(&self, t: f64) -> Result<State2, EphemerisTableError> {
        let start = self.start_time();
        let end = self.end_time();
        if t < start || t > end {
            return Err(EphemerisTableError::OutOfRange {
                time: t,
                start,
                end,
            });
        }

        let u = (t - self.start_t0) / self.step_seconds;
        let mut i = u.floor() as isize;

        // Clamp to a valid segment [i, i+1]
        if i < 0 {
            i = 0;
        }
        if i as usize >= self.samples.len() - 1 {
            i = (self.samples.len() - 2) as isize;
        }

        let i0 = i as usize;
        let i1 = i0 + 1;

        let t0 = self.start_t0 + self.step_seconds * i0 as f64;
        let s = (t - t0) / self.step_seconds;

        let p0 = self.samples[i0].pos;
        let p1 = self.samples[i1].pos;
        let m0 = self.samples[i0].vel * self.step_seconds;
        let m1 = self.samples[i1].vel * self.step_seconds;

        let h00 = 2.0 * s * s * s - 3.0 * s * s + 1.0;
        let h10 = s * s * s - 2.0 * s * s + s;
        let h01 = -2.0 * s * s * s + 3.0 * s * s;
        let h11 = s * s * s - s * s;

        let pos = p0 * h00 + m0 * h10 + p1 * h01 + m1 * h11;

        // Derivative of Hermite basis gives velocity.
        let dh00 = 6.0 * s * s - 6.0 * s;
        let dh10 = 3.0 * s * s - 4.0 * s + 1.0;
        let dh01 = -6.0 * s * s + 6.0 * s;
        let dh11 = 3.0 * s * s - 2.0 * s;

        let dpos_ds = p0 * dh00 + m0 * dh10 + p1 * dh01 + m1 * dh11;
        let vel = dpos_ds / self.step_seconds;

        Ok(State2 { pos, vel })
    }
}

struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn read_exact(&mut self, out: &mut [u8]) -> IoResult<()> {
        let end = self.offset + out.len();
        out.copy_from_slice(&self.bytes[self.offset..end]);
        self.offset = end;
        Ok(())
    }

    fn read_u32_le(&mut self) -> IoResult<u32> {
        let mut b = [0u8; 4];
        self.read_exact(&mut b)?;
        Ok(u32::from_le_bytes(b))
    }

    fn read_f64_le(&mut self) -> IoResult<f64> {
        let mut b = [0u8; 8];
        self.read_exact(&mut b)?;
        Ok(f64::from_le_bytes(b))
    }
}
