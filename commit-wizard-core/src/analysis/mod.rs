// analysis module for deterministic change insights

pub mod scope_candidates;
pub mod scope_policy;
pub mod ai_control;
pub mod guardrails;

pub use scope_candidates::{
    calculate_scope_coverage, calculate_scope_coverage_with_rules, derive_scope_candidates,
    derive_scope_candidates_with_options, derive_scope_candidates_with_rules, load_scope_rules,
    ScopeCandidateOptions, ScopeRuleSet,
};
pub use ai_control::AiControl;
pub use scope_policy::ScopeStrategy;
pub use guardrails::{analyse_guardrails, determine_forced_commit_type, GuardrailResult};
