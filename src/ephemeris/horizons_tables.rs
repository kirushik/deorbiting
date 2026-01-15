use crate::ephemeris::data::CelestialBodyId;
use crate::ephemeris::table::{EphemerisTable, EphemerisTableError, State2};
use bevy::math::DVec2;
use std::collections::HashMap;

/// Number of bodies with tables (excludes Sun which is at origin).
const TABLE_BODY_COUNT: usize = 14;

/// Time range covered by a loaded ephemeris table.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TableCoverage {
    pub start: f64,
    pub end: f64,
}

/// Mapping from in-game `CelestialBodyId` to the stable numeric IDs used in
/// `scripts/export_horizons_ephemeris.py`.
fn stable_body_id(id: CelestialBodyId) -> Option<u32> {
    Some(match id {
        CelestialBodyId::Sun => return None, // Sun is always origin
        CelestialBodyId::Mercury => 1,
        CelestialBodyId::Venus => 2,
        CelestialBodyId::Earth => 3,
        CelestialBodyId::Mars => 4,
        CelestialBodyId::Jupiter => 5,
        CelestialBodyId::Saturn => 6,
        CelestialBodyId::Uranus => 7,
        CelestialBodyId::Neptune => 8,
        CelestialBodyId::Moon => 9,
        CelestialBodyId::Io => 10,
        CelestialBodyId::Europa => 11,
        CelestialBodyId::Ganymede => 12,
        CelestialBodyId::Callisto => 13,
        CelestialBodyId::Titan => 14,
    })
}

#[derive(Default)]
pub struct HorizonsTables {
    tables: HashMap<CelestialBodyId, EphemerisTable>,
}

impl HorizonsTables {
    /// Load all available tables from `assets/ephemeris/`.
    ///
    /// Missing files are ignored; callers can fall back to Kepler for those bodies.
    pub fn load_from_assets_dir() -> Result<Self, EphemerisTableError> {
        let dir = std::path::Path::new("assets/ephemeris");
        let mut tables: HashMap<CelestialBodyId, EphemerisTable> = HashMap::new();

        // We intentionally hardcode names so the build doesn't depend on directory listing.
        let candidates: &[(CelestialBodyId, &str)] = &[
            (CelestialBodyId::Mercury, "mercury.bin"),
            (CelestialBodyId::Venus, "venus.bin"),
            (CelestialBodyId::Earth, "earth.bin"),
            (CelestialBodyId::Mars, "mars.bin"),
            (CelestialBodyId::Jupiter, "jupiter.bin"),
            (CelestialBodyId::Saturn, "saturn.bin"),
            (CelestialBodyId::Uranus, "uranus.bin"),
            (CelestialBodyId::Neptune, "neptune.bin"),
            (CelestialBodyId::Moon, "moon.bin"),
            (CelestialBodyId::Io, "io.bin"),
            (CelestialBodyId::Europa, "europa.bin"),
            (CelestialBodyId::Ganymede, "ganymede.bin"),
            (CelestialBodyId::Callisto, "callisto.bin"),
            (CelestialBodyId::Titan, "titan.bin"),
        ];

        for (id, file) in candidates {
            let path = dir.join(file);
            if !path.exists() {
                continue;
            }

            let table = EphemerisTable::load(&path)?;

            if let Some(expected) = stable_body_id(*id) {
                if table.body_id != expected {
                    // Hard error: indicates wrong file ↔ body mapping (e.g. wrong filename or stale export).
                    return Err(EphemerisTableError::BodyIdMismatch {
                        expected,
                        got: table.body_id,
                    });
                }
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
    /// Mercury, Venus, Earth, Mars, Jupiter, Saturn, Uranus, Neptune,
    /// Moon, Io, Europa, Ganymede, Callisto, Titan.
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
            CelestialBodyId::Moon,
            CelestialBodyId::Io,
            CelestialBodyId::Europa,
            CelestialBodyId::Ganymede,
            CelestialBodyId::Callisto,
            CelestialBodyId::Titan,
        ];

        let mut result = [None; TABLE_BODY_COUNT];

        for (i, &id) in BODY_ORDER.iter().enumerate() {
            if let Some(tbl) = self.tables.get(&id) {
                if let Ok(pos) = tbl.sample_position(t) {
                    result[i] = Some(pos);
                }
            }
        }

        result
    }
}
