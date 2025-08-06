// pattern detection module - defines pattern types and structures

#[derive(Debug, Clone)]
pub struct Pattern {
    pub pattern_type: PatternType,
    pub description: String,
    pub impact: f32,
    pub files_affected: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PatternType {
    NewFilePattern,
    MassModification,
    CrossLayerChange,
    InterfaceEvolution,
    ArchitecturalShift,
    ConfigurationDrift,
    DependencyUpdate,
    RefactoringPattern,
    FeatureAddition,
    BugFixPattern,
    TestEvolution,
    DocumentationUpdate,
    StyleNormalization,
    PerformanceTuning,
    SecurityHardening,
    CiChange,
    Deprecation,
    SecurityFix,
}
