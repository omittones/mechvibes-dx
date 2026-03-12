//! Soundpack loading, saving, caching, validation, and installation.
//!
//! This module contains the logic for:
//! - Loading and saving the soundpack metadata cache
//! - Loading soundpack metadata from config files
//! - Validating soundpack configurations and ZIP files
//! - Installing soundpacks from ZIP archives

pub mod cache;
pub mod format;
pub mod id;
pub mod installer;
pub mod metadata;
pub mod validator;
