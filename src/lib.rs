#![cfg_attr(not(test), warn(clippy::too_many_lines, unreachable_pub))]

mod command;
mod debug;
mod git;
mod gob;
pub use debug::Diagnostics;
pub use git::GitStatus;
mod navigation;
pub use gob::GobJob;
mod pi_workbench;
pub mod pr_state;
mod scroll;
pub use scroll::ScrollDirection;
mod sidebar;
mod tmux;
pub mod usage;

pub use pi_workbench::{PiContext, PiLocation, PiPlan, PiSession, PiSkill, PiTarget};
pub use sidebar::{RenderInputs, Session, Sidebar, Snapshot, Window};
pub use tmux::{
    scroll_client, scroll_client_for, scroll_worker, scroll_worker_for, Application, Focus, Pane,
    ProcessTmux, Tmux,
};
