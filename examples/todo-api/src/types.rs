use serde::{Deserialize, Serialize};

use crate::entity::Todo;

#[derive(Deserialize, modo::Sanitize, modo::Validate)]
pub(crate) struct CreateTodo {
    #[clean(trim, strip_html_tags)]
    #[validate(required(message = "title is required"), min_length = 5(message = "title must be at least 5 characters"), max_length = 500(message = "title must be at most 500 characters"))]
    pub(crate) title: String,
}

#[derive(Serialize)]
pub(crate) struct TodoResponse {
    id: String,
    title: String,
    completed: bool,
}

impl From<Todo> for TodoResponse {
    fn from(t: Todo) -> Self {
        Self {
            id: t.id,
            title: t.title,
            completed: t.completed,
        }
    }
}
