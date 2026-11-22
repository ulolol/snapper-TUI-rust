pub mod prefs;
pub mod workspace;

pub use prefs::AppPrefs;
pub use workspace::{
    ActiveView, InspectorTab, ToastLevel, WorkspaceEvent, WorkspaceModel, is_binary_diff,
};
