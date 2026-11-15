//! リネームエンジン本体。GUI やファイルシステム操作には依存しない。

pub mod error;
pub mod model;
pub mod plan;
pub mod platform;
pub mod rule;
pub mod token;
pub mod util;
pub mod validate;
pub mod width;

pub use error::{RenamerError, Result};
pub use model::{CandidateStatus, FileEntry, PlanOptions, RenameCandidate, TargetType};
pub use plan::{PlanSummary, RenamePlan, RuleError, build_plan};
pub use platform::Platform;
pub use rule::{
    ApplyContext, CaseMode, CompiledRule, ExtensionMode, FieldValue, NameParts, NumberPosition,
    RenameRule, RuleEntry, RuleField, WidthDirection, split_extension,
};
pub use token::{MAX_PARENT_LEVEL, expand_tokens, parent_names};
pub use util::{natural_cmp, path_key};
pub use validate::{validate_name, validate_name_for};
