use crate::ephemeris::data::CelestialBodyId;
#[cfg(feature = "embedded-ephemeris")]
use crate::ephemeris::embedded_tables;
use crate::ephemeris::table::{EphemerisTable, EphemerisTableError, State2};
use bevy::math::DVec2;
use std::collections::HashMap;

/// Number of bodies with tables (8 planets, excludes Sun and moons).
const TABLE_BODY_COUNT: usize = 8;

/// Time range covered by a loaded ephemeris table.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TableCoverage {
    pub start: f64,
    pub end: f64,
}

/// Mapping from in-game `CelestialBodyId` to the stable numeric IDs used in
/// `scripts/export_horizons_ephemeris.py`.
fn stable_body_id(id: CelestialBodyId) -> Option<u32> {
    // Only planets have ephemeris tables. Moons use Kepler (visual-only).
    Some(match id {
        CelestialBodyId::Mercury => 1,
        CelestialBodyId::Venus => 2,
        CelestialBodyId::Earth => 3,
        CelestialBodyId::Mars => 4,
        CelestialBodyId::Jupiter => 5,
        CelestialBodyId::Saturn => 6,
        CelestialBodyId::Uranus => 7,
        CelestialBodyId::Neptune => 8,
        // Sun is always at origin, moons use Kepler
        _ => return None,
    })
}

#[derive(Default)]
pub struct HorizonsTables {
    tables: HashMap<CelestialBodyId, EphemerisTable>,
}

impl HorizonsTables {
    /// Load all ephemeris tables.
    ///
    /// With `embedded-ephemeris` feature: loads from compiled-in binary data.
    /// Without feature: loads from `assets/ephemeris/*.bin` files.
    ///
    /// Only planets have ephemeris tables - moons use Kepler approximation (visual-only).
    pub fn load_from_assets_dir() -> Result<Self, EphemerisTableError> {
        #[cfg(feature = "embedded-ephemeris")]
        {
            Self::load_embedded()
        }
        #[cfg(not(feature = "embedded-ephemeris"))]
        {
            Self::load_from_filesystem()
        }
    }

    /// Load tables from embedded binary data (standalone distribution).
    #[cfg(feature = "embedded-ephemeris")]
    fn load_embedded() -> Result<Self, EphemerisTableError> {
        let mut tables: HashMap<CelestialBodyId, EphemerisTable> = HashMap::new();

        let candidates: &[(CelestialBodyId, &[u8])] = &[
            (CelestialBodyId::Mercury, embedded_tables::MERCURY),
            (CelestialBodyId::Venus, embedded_tables::VENUS),
            (CelestialBodyId::Earth, embedded_tables::EARTH),
            (CelestialBodyId::Mars, embedded_tables::MARS),
            (CelestialBodyId::Jupiter, embedded_tables::JUPITER),
            (CelestialBodyId::Saturn, embedded_tables::SATURN),
            (CelestialBodyId::Uranus, embedded_tables::URANUS),
            (CelestialBodyId::Neptune, embedded_tables::NEPTUNE),
        ];

        for (id, bytes) in candidates {
            let table = EphemerisTable::from_bytes(bytes)?;

            if let Some(expected) = stable_body_id(*id)
                && table.body_id != expected
            {
                return Err(EphemerisTableError::BodyIdMismatch {
                    expected,
                    got: table.body_id,
                });
            }

            tables.insert(*id, table);
        }

        Ok(Self { tables })
    }

    /// Load tables from filesystem (development builds).
    #[cfg(not(feature = "embedded-ephemeris"))]
    fn load_from_filesystem() -> Result<Self, EphemerisTableError> {
        let dir = std::path::Path::new("assets/ephemeris");
        let mut tables: HashMap<CelestialBodyId, EphemerisTable> = HashMap::new();

        let candidates: &[(CelestialBodyId, &str)] = &[
            (CelestialBodyId::Mercury, "mercury.bin"),
            (CelestialBodyId::Venus, "venus.bin"),
            (CelestialBodyId::Earth, "earth.bin"),
            (CelestialBodyId::Mars, "mars.bin"),
            (CelestialBodyId::Jupiter, "jupiter.bin"),
            (CelestialBodyId::Saturn, "saturn.bin"),
            (CelestialBodyId::Uranus, "uranus.bin"),
            (CelestialBodyId::Neptune, "neptune.bin"),
        ];

        for (id, file) in candidates {
            let path = dir.join(file);
            if !path.exists() {
                continue;
            }

            let table = EphemerisTable::load(&path)?;

            if let Some(expected) = stable_body_id(*id)
                && table.body_id != expected
            {
                return Err(EphemerisTableError::BodyIdMismatch {
                    expected,
                    got: table.body_id,
                });
            }

            tables.insert(*id, table);
        }

        Ok(Self { tables })
    }

    pub fn has(&self, id: CelestialBodyId) -> bool {
        self.tables.contains_key(&id)
    }

    /// Returns the coverage window for a body's table, if present.
    pub fn coverage(&self, id: CelestialBodyId) -> Option<TableCoverage> {
        let tbl = self.tables.get(&id)?;
        Some(TableCoverage {
            start: tbl.start_time(),
            end: tbl.end_time(),
        })
    }

    /// Returns a reference to the underlying table, if present.
    ///
    /// This is useful for continuity logic (e.g. sample-at-end and compute offsets).
    pub fn table(&self, id: CelestialBodyId) -> Option<&EphemerisTable> {
        self.tables.get(&id)
    }

    pub fn sample(
        &self,
        id: CelestialBodyId,
        t: f64,
    ) -> Option<Result<State2, EphemerisTableError>> {
        self.tables.get(&id).map(|tbl| tbl.sample(t))
    }

    /// Sample positions for all bodies at once, with better cache locality.
    ///
    /// Returns positions for bodies in standard order:
    /// Mercury, Venus, Earth, Mars, Jupiter, Saturn, Uranus, Neptune.
    ///
    /// Bodies without tables or outside table range return None.
    pub fn sample_all_positions(&self, t: f64) -> [Option<DVec2>; TABLE_BODY_COUNT] {
        const BODY_ORDER: [CelestialBodyId; TABLE_BODY_COUNT] = [
            CelestialBodyId::Mercury,
            CelestialBodyId::Venus,
            CelestialBodyId::Earth,
            CelestialBodyId::Mars,
            CelestialBodyId::Jupiter,
            CelestialBodyId::Saturn,
            CelestialBodyId::Uranus,
            CelestialBodyId::Neptune,
        ];

        let mut result = [None; TABLE_BODY_COUNT];

        for (i, &id) in BODY_ORDER.iter().enumerate() {
            if let Some(tbl) = self.tables.get(&id)
                && let Ok(pos) = tbl.sample_position(t)
            {
                result[i] = Some(pos);
            }
        }

        result
    }
}
