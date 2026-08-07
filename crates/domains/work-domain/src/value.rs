//! Value types for the Work (Task) bounded context.

use mission_domain::MissionId;
use platform_kernel::ObjectId;
use serde::{Deserialize, Serialize};

/// The identity of a Task aggregate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TaskId(pub ObjectId);

impl TaskId {
    /// Generates a new random task identifier.
    pub fn new_random() -> Self {
        Self(ObjectId::new_random())
    }
}

/// The kind of relationship a [`Dependency`] expresses between two tasks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DependencyType {
    /// The dependent task cannot start until the dependency finishes.
    FinishToStart,
    /// The dependent task cannot start until the dependency starts.
    StartToStart,
    /// The dependent task cannot finish until the dependency finishes.
    FinishToFinish,
}

/// A reference to another task that this task depends on.
///
/// # Increment 1 Ruling (D)
/// Placeholder struct, per ruling. `AddDependency`'s acyclicity invariant
/// is deferred to a later increment (a single aggregate cannot see the
/// full dependency graph); Increment 1 rejects only self-dependency and
/// exact duplicates (ruling B).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dependency {
    /// The task this dependency points to.
    pub task_id: TaskId,
    /// The kind of dependency relationship.
    pub dependency_type: DependencyType,
}

/// The relative priority of a task.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize)]
pub enum TaskPriority {
    /// Lowest priority.
    Low,
    /// Default priority.
    #[default]
    Normal,
    /// Elevated priority.
    High,
    /// Highest priority; time-critical.
    Urgent,
}

/// The mission this task belongs to. Every task belongs to exactly one
/// mission (Handover Document invariant).
pub type TaskMissionRef = MissionId;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_id_random_generation_is_unique() {
        let a = TaskId::new_random();
        let b = TaskId::new_random();
        assert_ne!(a, b);
    }

    #[test]
    fn task_priority_default_is_normal() {
        assert_eq!(TaskPriority::default(), TaskPriority::Normal);
    }

    #[test]
    fn task_priority_orders_low_to_urgent() {
        assert!(TaskPriority::Low < TaskPriority::Normal);
        assert!(TaskPriority::Normal < TaskPriority::High);
        assert!(TaskPriority::High < TaskPriority::Urgent);
    }

    #[test]
    fn dependency_constructs_with_task_id_and_type() {
        let dep = Dependency {
            task_id: TaskId::new_random(),
            dependency_type: DependencyType::FinishToStart,
        };
        assert_eq!(dep.dependency_type, DependencyType::FinishToStart);
    }
}
