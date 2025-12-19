// analysis module for deterministic change insights

pub mod scope_candidates;
pub mod scope_policy;
pub mod ai_control;
pub mod guardrails;

pub use scope_candidates::{
    derive_scope_candidates, derive_scope_candidates_with_options, ScopeCandidateOptions,
};
pub use ai_control::AiControl;
pub use scope_policy::ScopeStrategy;
pub use guardrails::determine_forced_commit_type;
