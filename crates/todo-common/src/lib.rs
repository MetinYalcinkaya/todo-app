use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Serialize, Deserialize)]
pub struct TaskQuery {
    pub done: Option<bool>,
    pub priority: Option<Priority>,
}

impl From<Filter> for TaskQuery {
    fn from(filter: Filter) -> Self {
        match filter {
            Filter::All => TaskQuery {
                done: None,
                priority: None,
            },
            Filter::Todo => TaskQuery {
                done: Some(false),
                priority: None,
            },
            Filter::Done => TaskQuery {
                done: Some(true),
                priority: None,
            },
            Filter::Priority(priority) => TaskQuery {
                done: None,
                priority: Some(priority),
            },
        }
    }
}

#[derive(Default, Clone, Deserialize, Serialize, Debug)]
#[cfg_attr(feature = "backend", derive(sqlx::FromRow))]
pub struct Task {
    pub id: i64,
    pub text: String,
    pub done: bool,
    pub priority: Priority,
}

impl std::fmt::Display for Task {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let status = if self.done { "[x]" } else { "[ ]" };
        write!(f, "{status} {} {}: {}", self.priority, self.id, self.text)
    }
}

#[derive(Clone, Copy, Default, Deserialize, Serialize, Debug, PartialEq)]
#[cfg_attr(feature = "backend", derive(sqlx::Type))]
#[cfg_attr(feature = "backend", sqlx(type_name = "TEXT"))]
pub enum Priority {
    #[default]
    Low,
    Medium,
    High,
}

impl std::fmt::Display for Priority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Priority::Low => write!(f, "(L)"),
            Priority::Medium => write!(f, "(M)"),
            Priority::High => write!(f, "(H)"),
        }
    }
}

#[derive(Debug, Error)]
pub enum TodoError {
    #[error("invalid command")]
    UnknownCommand,
    #[error("invalid arguments")]
    MissingArgument,
    #[error("task with that id was not found")]
    TaskNotFound,
    #[error("task id must be a positive integer")]
    InvalidId(#[from] std::num::ParseIntError),
    #[error("failed to save todo list")]
    SaveError(#[source] std::io::Error),
    #[error("unknown priority")]
    PriorityError,
}

#[derive(Copy, Clone, Debug, Deserialize, Default, Serialize, PartialEq)]
pub enum Filter {
    #[default]
    All,
    Todo,
    Done,
    Priority(Priority),
}

impl std::fmt::Display for Filter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Filter::All => write!(f, "All"),
            Filter::Todo => write!(f, "Todo"),
            Filter::Done => write!(f, "Done"),
            Filter::Priority(priority) => write!(f, "Priority {priority}"),
        }
    }
}
