pub mod analysis;
pub mod evidence;
pub mod policy;
pub mod scheduler;
pub mod scoring;
pub mod source;
pub mod types;

pub use analysis::run_analysis_pipeline;
pub use evidence::{EvidenceCue, EvidenceIndex};
pub use policy::{AutoClippingPolicy, RoomPolicyOverride};
pub use scheduler::AutomationService;
pub use scoring::{CandidateRanker, RawHighlightCandidate};
pub use source::SourceManifest;
pub use types::{
    AutomationAnalysisRow, AutomationCandidateRow, AutomationCandidateStatus, AutomationOutputMode,
    AutomationOutputRow, AutomationOutputStatus, AutomationRoomPolicyRow, AutomationRunRow,
    AutomationRunStatus, AutomationStage, ClipOutputMode, EditPlan, EditPlanClip, EditPlanNarration,
    EditPlanOverlay, LiveContentType, SourceManifestItem,
};
