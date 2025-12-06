use serde::{Deserialize, Serialize};
use sqlx::Type;
use thiserror::Error;

#[derive(Default, Clone, Deserialize, Serialize, Debug)]
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

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct TodoList {
    pub tasks: Vec<Task>,
    pub next_id: i64,
}

impl Default for TodoList {
    fn default() -> Self {
        Self {
            tasks: Default::default(),
            next_id: 1,
        }
    }
}

impl TodoList {
    pub fn add(&mut self, text: String) -> &Task {
        let id = self.next_id;
        self.tasks.push(Task {
            id,
            text,
            done: false,
            priority: Priority::default(),
        });
        self.next_id = id + 1;
        self.tasks.last().unwrap()
    }

    pub fn print_list(&self) {
        for task in &self.tasks {
            println!("{task}");
        }
    }

    // pub fn get_list(&self) -> Json<Vec<Task>> {
    pub fn get_list(&self) -> Vec<Task> {
        self.tasks.clone()
    }

    pub fn mark_done(&mut self, id: i64) -> Result<&Task, TodoError> {
        if let Some(task) = self.tasks.iter_mut().find(|t| t.id == id) {
            task.done = true;
            Ok(task)
        } else {
            Err(TodoError::TaskNotFound)
        }
    }

    pub fn print_done(&self) {
        for task in self.tasks.iter().filter(|t| t.done) {
            println!("{task}");
        }
    }

    pub fn print_todo(&self) {
        for task in self.tasks.iter().filter(|t| !t.done) {
            println!("{task}");
        }
    }

    pub fn print_by_priority(&self, priority: Priority) {
        for task in self.tasks.iter().filter(|t| t.priority == priority) {
            println!("{task}");
        }
    }

    pub fn set_priority(&mut self, id: i64, priority: Priority) -> Result<&Task, TodoError> {
        if let Some(task) = self.tasks.iter_mut().find(|t| t.id == id) {
            task.priority = priority;
            Ok(task)
        } else {
            Err(TodoError::TaskNotFound)
        }
    }
}

#[derive(Clone, Copy, Default, Deserialize, Serialize, Debug, PartialEq, Type)]
#[sqlx(type_name = "TEXT")]
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
