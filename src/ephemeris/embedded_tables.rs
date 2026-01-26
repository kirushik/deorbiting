//! Embedded ephemeris table data compiled into the binary.
//!
//! This module provides pre-generated ephemeris tables for all planets,
//! embedded at compile time using `include_bytes!`. This makes the binary
//! fully standalone - no external assets needed at runtime.
//!
//! Binary size impact: ~18MB (8 planets × ~2.3MB each).

/// Mercury ephemeris table (2020-2099, 1-hour steps).
pub const MERCURY: &[u8] = include_bytes!("../../assets/ephemeris/mercury.bin");

/// Venus ephemeris table (2020-2099, 1-hour steps).
pub const VENUS: &[u8] = include_bytes!("../../assets/ephemeris/venus.bin");

/// Earth ephemeris table (2020-2099, 1-hour steps).
pub const EARTH: &[u8] = include_bytes!("../../assets/ephemeris/earth.bin");

/// Mars ephemeris table (2020-2099, 1-hour steps).
pub const MARS: &[u8] = include_bytes!("../../assets/ephemeris/mars.bin");

/// Jupiter ephemeris table (2020-2099, 1-hour steps).
pub const JUPITER: &[u8] = include_bytes!("../../assets/ephemeris/jupiter.bin");

/// Saturn ephemeris table (2020-2099, 1-hour steps).
pub const SATURN: &[u8] = include_bytes!("../../assets/ephemeris/saturn.bin");

/// Uranus ephemeris table (2020-2099, 1-hour steps).
pub const URANUS: &[u8] = include_bytes!("../../assets/ephemeris/uranus.bin");

/// Neptune ephemeris table (2020-2099, 1-hour steps).
pub const NEPTUNE: &[u8] = include_bytes!("../../assets/ephemeris/neptune.bin");
