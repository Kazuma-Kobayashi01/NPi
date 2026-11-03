//! The Chromium processes behind the panel: which browser to run (the one
//! bundled with the app), starting and stopping it, the
//! private files of this app instance, and reaping what a crashed earlier
//! run left behind.

mod bundled;
mod instance;
mod launch;
mod pipe;
mod reap;
mod stderr;

pub use instance::{prepare_instance_dir, prune_dead, remove_dir_retrying, write_overlay};
pub use launch::Chromium;
pub use bundled::BundledChromium;
