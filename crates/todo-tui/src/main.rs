use cli_log::{debug, error, init_cli_log};
use color_eyre::eyre::Result;
use crossterm::event;
use crossterm::event::{Event, KeyCode};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use ratatui::layout::{Constraint, Direction, Flex, Layout, Rect};
use ratatui::prelude::Alignment;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph};
use ratatui::{Frame, Terminal};
use ratatui::{prelude::CrosstermBackend, widgets::ListState};
use std::io::stdout;
use todo_common::{Filter, Priority, Task, TaskQuery};
use tokio::sync::mpsc;

#[derive(Default, PartialEq, Debug)]
enum InputMode {
    #[default]
    Normal,
    Editing,
    Filter,
}

enum Action {
    Fetch(Filter),
    Create(String, Filter),
    Delete(i64, Filter),
    Update(i64, Option<String>, Option<bool>, Option<Priority>, Filter),
}

enum TuiEvent {
    TasksFetched(Vec<Task>),
    Error(String),
}

#[derive(Default, Debug)]
struct App {
    tasks: Vec<Task>,
    todo_state: ListState,
    help_state: ListState,
    filter_state: ListState,
    input: String,
    mode: InputMode,
    filter: Filter,
    currently_editing_id: Option<i64>,
    priority: Priority,
    help_size: usize,
    help_mode: InputMode,
}

#[derive(serde::Serialize)]
struct CreateTodo {
    text: String,
}

#[derive(serde::Serialize)]
struct UpdateTodo {
    text: Option<String>,
    done: Option<bool>,
    priority: Option<Priority>,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv()?;

    let mut app = {
        App {
            tasks: fetch_tasks(Filter::default()).await.unwrap_or_default(),
            ..Default::default()
        }
    };

    let (action_tx, mut action_rx) = mpsc::unbounded_channel();
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        while let Some(action) = action_rx.recv().await {
            match action {
                Action::Fetch(filter) => match fetch_tasks(filter).await {
                    Ok(tasks) => event_tx.send(TuiEvent::TasksFetched(tasks)).unwrap(),
                    Err(e) => event_tx.send(TuiEvent::Error(e.to_string())).unwrap(),
                },
                Action::Create(text, filter) => {
                    if let Err(e) = create_task(text).await {
                        event_tx.send(TuiEvent::Error(e.to_string())).unwrap();
                    } else {
                        match fetch_tasks(filter).await {
                            Ok(tasks) => event_tx.send(TuiEvent::TasksFetched(tasks)).unwrap(),
                            Err(e) => event_tx.send(TuiEvent::Error(e.to_string())).unwrap(),
                        }
                    }
                }
                Action::Delete(id, filter) => {
                    if let Err(e) = delete_task(id).await {
                        event_tx.send(TuiEvent::Error(e.to_string())).unwrap();
                    } else {
                        match fetch_tasks(filter).await {
                            Ok(tasks) => event_tx.send(TuiEvent::TasksFetched(tasks)).unwrap(),
                            Err(e) => event_tx.send(TuiEvent::Error(e.to_string())).unwrap(),
                        }
                    }
                }
                Action::Update(id, text, done, priority, filter) => {
                    if let Err(e) = update_task(id, text, done, priority).await {
                        event_tx.send(TuiEvent::Error(e.to_string())).unwrap();
                    } else {
                        match fetch_tasks(filter).await {
                            Ok(tasks) => event_tx.send(TuiEvent::TasksFetched(tasks)).unwrap(),
                            Err(e) => event_tx.send(TuiEvent::Error(e.to_string())).unwrap(),
                        }
                    }
                }
            }
        }
    });

    enable_raw_mode().unwrap();
    init_cli_log!();
    color_eyre::install()?;
    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    loop {
        while let Ok(event) = event_rx.try_recv() {
            match event {
                TuiEvent::TasksFetched(tasks) => app.tasks = tasks,
                TuiEvent::Error(msg) => error!("event error: {msg}"),
            }
        }
        terminal.draw(|f| ui(f, &mut app))?;

        if event::poll(std::time::Duration::from_millis(50))?
            && let Event::Key(key) = event::read()?
        {
            match app.mode {
                InputMode::Normal => match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Char('r') => {
                        action_tx.send(Action::Fetch(app.filter))?;
                    }
                    KeyCode::Char('i') => app.mode = InputMode::Editing,
                    KeyCode::Char('e') => {
                        if let Some(index) = app.todo_state.selected()
                            && let Some(task) = app.tasks.get(index)
                        {
                            app.currently_editing_id = Some(task.id);
                            app.mode = InputMode::Editing;
                            app.input.push_str(&task.text); // append task text
                            debug!("current editing id: {}", app.currently_editing_id.unwrap());
                        }
                    }
                    KeyCode::Char('d') => {
                        if let Some(index) = app.todo_state.selected()
                            && let Some(task) = app.tasks.get(index)
                            && let Err(e) = action_tx.send(Action::Delete(task.id, app.filter))
                        {
                            error!("failed to send delete action: {e}");
                        }
                    }
                    KeyCode::Char('f') => {
                        app.mode = InputMode::Filter;
                        app.filter_state.select(Some(0));
                    }
                    }
                    KeyCode::Enter => {
                        if let Some(index) = app.todo_state.selected()
                            && let Some(task) = app.tasks.get(index)
                            && let Err(e) = action_tx.send(Action::Update(
                                task.id,
                                None,
                                Some(!task.done),
                                None,
                                app.filter,
                            ))
                        {
                            error!("failed to send toggle (update) action: {e}");
                        }
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        let i = match app.todo_state.selected() {
                            Some(i) => {
                                if i == 0 {
                                    app.tasks.len() - 1
                                } else {
                                    i - 1
                                }
                            }
                            None => 0,
                        };
                        app.todo_state.select(Some(i));
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        let i = match app.todo_state.selected() {
                            Some(i) => {
                                if i >= app.tasks.len() - 1 {
                                    0
                                } else {
                                    i + 1
                                }
                            }
                            None => 0,
                        };
                        app.todo_state.select(Some(i));
                    }
                    KeyCode::Char('x') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        debug!("lower priority");
                        if let Some(index) = app.todo_state.selected()
                            && let Some(task) = app.tasks.get(index)
                        {
                            let new_prio = match task.priority {
                                Priority::Low => Priority::High,
                                Priority::Medium => Priority::Low,
                                Priority::High => Priority::Medium,
                            };
                            debug!("new_prio: {new_prio}");
                            if let Err(e) = action_tx.send(Action::Update(
                                task.id,
                                None,
                                None,
                                Some(new_prio),
                                app.filter,
                            )) {
                                error!("failed to lower priority: {e}");
                            }
                        }
                    }
                    KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        debug!("increase priority");
                        if let Some(index) = app.todo_state.selected()
                            && let Some(task) = app.tasks.get(index)
                        {
                            let new_prio = match task.priority {
                                Priority::Low => Priority::Medium,
                                Priority::Medium => Priority::High,
                                Priority::High => Priority::Low,
                            };
                            debug!("new_prio: {new_prio}");
                            if let Err(e) = action_tx.send(Action::Update(
                                task.id,
                                None,
                                None,
                                Some(new_prio),
                                app.filter,
                            )) {
                                error!("failed to increase priority: {e}");
                            }
                        }
                    }
                    _ => {}
                },
                InputMode::Editing => match key.code {
                    KeyCode::Esc => {
                        app.mode = InputMode::Normal;
                        app.input.clear(); // clear buf
                    }
                    KeyCode::Char(c) => {
                        app.input.push(c);
                    }
                    KeyCode::Backspace => {
                        app.input.pop();
                    }
                    KeyCode::Enter => {
                        if app.currently_editing_id.is_some() {
                            let task = app
                                .tasks
                                .iter()
                                .find(|t| t.id == app.currently_editing_id.unwrap());
                            let task = task.unwrap();
                            debug!("update: {task}");
                            if let Err(e) = action_tx.send(Action::Update(
                                task.id,
                                Some(app.input.clone()),
                                Some(task.done),
                                None,
                                app.filter,
                            )) {
                                error!("failed to send update action: {e}");
                            }
                            app.currently_editing_id = None;
                        } else {
                            debug!("create");
                            if let Err(e) =
                                action_tx.send(Action::Create(app.input.clone(), app.filter))
                            {
                                error!("failed to send create action: {e}");
                            }
                        }
                        // reset state
                        app.input.clear();
                        app.mode = InputMode::Normal;
                    }
                    _ => {}
                },
                InputMode::Filter => match key.code {
                    KeyCode::Esc => {
                        app.mode = InputMode::Normal;
                    }
                    KeyCode::Enter => {
                        if let Some(index) = app.filter_state.selected()
                            && let Some(filter) = get_menu_filters(app.priority).get(index)
                        {
                            debug!("setting filter to {filter}");
                            app.filter = *filter;

                            if let Err(e) = action_tx.send(Action::Fetch(app.filter)) {
                                error!("failed to send fetch action: {e}");
                            }
                        }
                        app.mode = InputMode::Normal;
                        debug!("{}", app.filter);
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        let i = match app.filter_state.selected() {
                            Some(i) => {
                                if i == 0 {
                                    get_menu_filters(app.priority).len() - 1
                                } else {
                                    i - 1
                                }
                            }
                            None => 0,
                        };
                        app.filter_state.select(Some(i));
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        let i = match app.filter_state.selected() {
                            Some(i) => {
                                if i >= get_menu_filters(app.priority).len() - 1 {
                                    0
                                } else {
                                    i + 1
                                }
                            }
                            None => 0,
                        };
                        app.filter_state.select(Some(i));
                    }
                    KeyCode::Char('x') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        if let Some(index) = app.filter_state.selected()
                            && let Some(filter) = get_menu_filters(app.priority).get(index)
                            && let Filter::Priority(priority) = filter
                        {
                            debug!("{priority}");
                            match app.priority {
                                Priority::Low => app.priority = Priority::High,
                                Priority::Medium => app.priority = Priority::Low,
                                Priority::High => app.priority = Priority::Medium,
                            }
                        }
                    }
                    KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        if let Some(index) = app.filter_state.selected()
                            && let Some(filter) = get_menu_filters(app.priority).get(index)
                            && let Filter::Priority(priority) = filter
                        {
                            debug!("{priority}");
                            match app.priority {
                                Priority::Low => app.priority = Priority::Medium,
                                Priority::Medium => app.priority = Priority::High,
                                Priority::High => app.priority = Priority::Low,
                            }
                        }
                    }
                    _ => {}
                },
            }
        }
    }
    disable_raw_mode()?;
    Ok(())
}

const TITLE_INDEX: usize = 0;
const LIST_INDEX: usize = 1;
const FOOTER_INDEX: usize = 2;

fn ui(frame: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // title
            Constraint::Min(1),    // list
            Constraint::Length(1), // footer
        ])
        .split(frame.area());

    // render title

    let title = Paragraph::new(Text::styled("todo", Style::default().fg(Color::LightBlue)))
        .alignment(Alignment::Center);

    frame.render_widget(title, chunks[TITLE_INDEX]);

    // render list

    let list_filter = match app.filter {
        Filter::Priority(_) => format!("Priority {}", app.priority),
        _ => app.filter.to_string(),
    };
    let list_title = format!("Tasks ({list_filter})");
    let list_block = Block::default().borders(Borders::ALL).title(list_title);
    let list = List::new(app.tasks.iter().map(|t| t.to_listitem()))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .block(list_block);
    if app.mode == InputMode::Filter {
        frame.render_widget(list, chunks[LIST_INDEX]);
    } else {
        frame.render_stateful_widget(list, chunks[LIST_INDEX], &mut app.todo_state);
    }

    // render input

    match app.mode {
        InputMode::Normal => {}
        InputMode::Editing => {
            let input_block = Block::default().borders(Borders::ALL).title("Add Task");
            let input_style = Style::default().fg(Color::Yellow);

            let input = Paragraph::new(app.input.as_str())
                .style(input_style)
                .block(input_block);
            let area = popup_area(chunks[LIST_INDEX], 50, 3);

            frame.render_widget(Clear, area);
            frame.render_widget(input, area);
        }
        InputMode::Filter => {
            let filter_block = Block::default().borders(Borders::ALL).title("Filter by");
            let filters: Vec<String> = get_menu_filters(app.priority)
                .iter()
                .map(std::string::ToString::to_string)
                .collect();

            let input = List::new(filters)
                .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
                .block(filter_block);
            let area = popup_area(chunks[LIST_INDEX], 15, 6);
            frame.render_stateful_widget(input, area, &mut app.filter_state);
        }
    }

    // render footer

    let help_text = match app.mode {
        InputMode::Normal => {
            "q: quit | <CR>: toggle done | d: delete task | i: add task | e: edit task | r: refresh"
        }
        InputMode::Editing => "esc: exit editing mode | <CR>: submit",
        InputMode::Filter => "esc: exit filter mode | left/right: change priority | <CR>: filter",
    };
    let footer = Paragraph::new(help_text).alignment(Alignment::Center);
    frame.render_widget(footer, chunks[FOOTER_INDEX]);
}

fn get_menu_filters(cur_priority: Priority) -> Vec<Filter> {
    vec![
        Filter::All,
        Filter::Todo,
        Filter::Done,
        Filter::Priority(cur_priority),
    ]
}

fn popup_area(area: Rect, percent_x: u16, percent_y: u16) -> Rect {
    let vertical = Layout::vertical([Constraint::Length(percent_y)]).flex(Flex::Center);
    let horizontal = Layout::horizontal([Constraint::Length(percent_x)]).flex(Flex::Center);
    let [area] = vertical.areas(area);
    let [area] = horizontal.areas(area);
    area
}

async fn fetch_tasks(filter: Filter) -> Result<Vec<Task>, Box<dyn std::error::Error>> {
    debug!("fetch_tasks: {filter}");
    let params = TaskQuery::from(filter);

    let client = reqwest::Client::new();
    let response = client
        .get("http://localhost:3000/todos")
        .query(&params)
        .send()
        .await?
        .json::<Vec<Task>>()
        .await?;

    Ok(response)
}

async fn create_task(text: String) -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    client
        .post("http://localhost:3000/todos")
        .json(&CreateTodo { text })
        .send()
        .await?;
    Ok(())
}

async fn delete_task(id: i64) -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    client
        .delete(format!("http://localhost:3000/todos/{id}"))
        .send()
        .await?;
    Ok(())
}

async fn update_task(
    id: i64,
    text: Option<String>,
    done: Option<bool>,
    priority: Option<Priority>,
) -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    client
        .patch(format!("http://localhost:3000/todos/{id}"))
        .json(&UpdateTodo {
            text,
            done,
            priority,
        })
        .send()
        .await?;
    Ok(())
}

trait TaskExt {
    fn to_listitem(&'_ self) -> ListItem<'_>;
}

impl TaskExt for Task {
    fn to_listitem(&'_ self) -> ListItem<'_> {
        let color = if self.done {
            Color::Green
        } else {
            Color::Yellow
        };
        let status_text = if self.done { "[x]" } else { "[ ]" };
        let line = Line::from(vec![
            Span::styled(status_text, Style::default().fg(color)),
            Span::raw(format!(" {} ", self.text)),
            Span::styled(
                format!("{}", self.priority),
                Style::default().fg(Color::Gray),
            ),
        ]);
        ListItem::new(line)
    }
}
