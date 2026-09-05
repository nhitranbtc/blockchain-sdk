//! Transaction construction, signing, and broadcast.
//!
//! Phase 1 lands only [`sign`]; the builder, broadcast, and summary modules
//! arrive in Phase 2 per the plan's File Structure.

pub mod sign;
