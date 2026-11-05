//! The Flows window (View › Flows…): a collection's `.flow` files as node
//! graphs. Nodes are dragged to move them; clicking an outcome's dot and then
//! a node connects them. The panel on the right edits the selected node, and
//! Run sends the flow with the collection's variables, colouring each node by
//! its outcome. The engine is `apix_runner::flow`, the same as `apix flow run`.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use apix_core::flow::{self, Flow, Node, NodeKind, Op};
use apix_core::{Collection, Project, StoreError, project};
use apix_runner::flow::{FlowPlan, FlowProgress, FlowReport, Step};
use apix_runner::{Control, Scope};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState, Textarea, TextareaState};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::{Disableable as _, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::Workspace;
use crate::bidi::{self, DialogBidi as _};
use crate::{net, tr};

const CANVAS_W: f32 = 780.;
const CANVAS_H: f32 = 430.;
const NODE_W: f32 = 170.;
const NODE_H: f32 = 72.;

/// Where a node's outcome dot is, relative to the canvas.
fn port_at(node: &Node, index: usize) -> (f32, f32) {
    (node.x + NODE_W, node.y + 30. + index as f32 * 22.)
}

impl Workspace {
    pub(super) fn open_flows(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(collection) = self.collection.clone() else {
            return self.notify_error(
                tr!("collection.open_first", key = super::key_hint("O")),
                window,
                cx,
            );
        };
        let project = self
            .project
            .clone()
            .filter(|p| collection.root.starts_with(&p.root));
        let (environment, script_vars) = (self.environment.clone(), self.script_vars.clone());
        let workspace = cx.entity().downgrade();
        let view =
            cx.new(|cx| FlowsView::new(collection, project, environment, script_vars, window, cx));
        window.open_dialog(cx, move |dialog, _, _| {
            dialog
                .on_close(super::collections::refocus(workspace.clone()))
                .bidi_title(tr!("flows.title"))
                .width(px(1200.))
                .child(div().pb_2().child(view.clone()))
        });
    }
}

/// The inputs of the selected node.
struct Fields {
    node: String,
    id: Entity<InputState>,
    /// The request path, a check's subject or a wait's milliseconds.
    main: Entity<InputState>,
    /// `name = value` lines: what a request keeps, what a set node sets.
    lines: Entity<TextareaState>,
    /// A check's value.
    value: Entity<InputState>,
    id_taken: bool,
    _subscriptions: Vec<Subscription>,
}

/// A node being moved.
struct Drag {
    node: String,
    start: Point<Pixels>,
    from: (f32, f32),
    moved: bool,
}

/// What a run reported, read by the UI on a timer.
#[derive(Default)]
struct Live {
    current: Option<String>,
    steps: Vec<Step>,
    report: Option<FlowReport>,
}

pub(super) struct FlowsView {
    collection: Collection,
    project: Option<Project>,
    environment: Option<String>,
    script_vars: apix_core::scripts::ScriptVars,
    files: Vec<PathBuf>,
    current: Option<PathBuf>,
    flow: Flow,
    dirty: bool,
    load_error: Option<String>,
    selected: Option<String>,
    fields: Option<Fields>,
    connecting: Option<(String, String)>,
    drag: Option<Drag>,
    new_name: Entity<InputState>,
    /// Requests of the collection, relative to its root.
    requests: Vec<String>,
    // A run.
    control: Option<Arc<Control>>,
    live: Arc<Mutex<Live>>,
    running_node: Option<String>,
    steps: Vec<Step>,
    report: Option<FlowReport>,
    _task: Option<Task<()>>,
}

impl Drop for FlowsView {
    fn drop(&mut self) {
        // Closing the window keeps the edits and ends a run.
        if self.dirty
            && let Some(path) = &self.current
        {
            let _ = flow::save_flow(path, &self.flow);
        }
        if let Some(control) = &self.control {
            control.stop();
        }
    }
}

impl FlowsView {
    fn new(
        collection: Collection,
        project: Option<Project>,
        environment: Option<String>,
        script_vars: apix_core::scripts::ScriptVars,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let files = flow::list_flows(&collection.root);
        let requests = collection
            .request_paths()
            .iter()
            .filter_map(|p| p.strip_prefix(&collection.root).ok())
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .collect();
        let new_name = cx.new(|cx| InputState::new(window, cx).placeholder(tr!("flows.new_name")));
        let mut view = Self {
            collection,
            project,
            environment,
            script_vars,
            current: None,
            flow: Flow::default(),
            dirty: false,
            load_error: None,
            selected: None,
            fields: None,
            connecting: None,
            drag: None,
            new_name,
            requests,
            control: None,
            live: Arc::default(),
            running_node: None,
            steps: Vec::new(),
            report: None,
            _task: None,
            files,
        };
        if let Some(first) = view.files.first().cloned() {
            view.open(first, window, cx);
        }
        view
    }

    fn is_running(&self) -> bool {
        self.control.is_some()
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(path) = &self.current else { return };
        match flow::save_flow(path, &self.flow) {
            Ok(()) => self.dirty = false,
            Err(e) => window.push_notification(Notification::error(e.to_string()), cx),
        }
        cx.notify();
    }

    fn open(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        if self.dirty {
            self.save(window, cx);
        }
        let (flow, error) = match flow::load_flow(&path) {
            Ok(flow) => (flow, None),
            Err(e) => (Flow::default(), Some(e.to_string())),
        };
        self.flow = flow;
        self.load_error = error;
        self.current = Some(path);
        self.dirty = false;
        self.steps.clear();
        self.report = None;
        self.connecting = None;
        self.select(None, window, cx);
    }

    fn create(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.new_name.read(cx).value().trim().to_string();
        if name.is_empty() {
            return;
        }
        let path = self.collection.root.join(format!(
            "{}.{}",
            apix_core::store::slug(&name, "flow"),
            flow::FLOW_EXTENSION
        ));
        if path.exists() {
            window.push_notification(Notification::error(tr!("flows.exists", name = name)), cx);
            return;
        }
        let flow = Flow {
            name,
            ..Default::default()
        };
        if let Err(e) = flow::save_flow(&path, &flow) {
            window.push_notification(Notification::error(e.to_string()), cx);
            return;
        }
        self.new_name
            .update(cx, |s, cx| s.set_value(String::new(), window, cx));
        self.files = flow::list_flows(&self.collection.root);
        self.open(path, window, cx);
    }

    fn changed(&mut self, cx: &mut Context<Self>) {
        self.dirty = true;
        cx.notify();
    }

    // ---------------------------------------------------------------
    // Selection and the node's inputs
    // ---------------------------------------------------------------

    fn select(&mut self, id: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        self.selected = id.clone();
        self.fields = id
            .and_then(|id| self.flow.node(&id).cloned())
            .map(|node| self.fields_for(&node, window, cx));
        cx.notify();
    }

    fn fields_for(&self, node: &Node, window: &mut Window, cx: &mut Context<Self>) -> Fields {
        let input = |value: String,
                     placeholder: SharedString,
                     window: &mut Window,
                     cx: &mut Context<Self>| {
            cx.new(|cx| {
                let mut state = InputState::new(window, cx).placeholder(placeholder);
                state.set_value(value, window, cx);
                state
            })
        };
        let lines_of = |map: &BTreeMap<String, String>| {
            map.iter()
                .map(|(k, v)| format!("{k} = {v}"))
                .collect::<Vec<_>>()
                .join("\n")
        };
        let (main, lines, value) = match &node.kind {
            NodeKind::Request { request, extract } => {
                (request.clone(), lines_of(extract), String::new())
            }
            NodeKind::Check { subject, value, .. } => {
                (subject.clone(), String::new(), value.clone())
            }
            NodeKind::Set { values } => (String::new(), lines_of(values), String::new()),
            NodeKind::Delay { ms } => (ms.to_string(), String::new(), String::new()),
        };
        let main_hint = match &node.kind {
            NodeKind::Request { .. } => tr!("flows.request_hint"),
            NodeKind::Check { .. } => tr!("flows.subject_hint"),
            _ => "1000".into(),
        };
        let id = input(node.id.clone(), "id".into(), window, cx);
        let main = input(main, main_hint, window, cx);
        let value = input(value, tr!("flows.value"), window, cx);
        let lines = cx.new(|cx| {
            let mut state = TextareaState::new(window, cx)
                .auto_grow(3, 8)
                .placeholder(tr!("flows.lines_hint"));
            state.set_value(lines, window, cx);
            state
        });
        let mut subscriptions = Vec::new();
        for field in [&id, &main, &value] {
            subscriptions.push(cx.subscribe_in(
                field,
                window,
                |this, _, event: &InputEvent, _, cx| {
                    if matches!(event, InputEvent::Change) {
                        this.apply_fields(cx);
                    }
                },
            ));
        }
        subscriptions.push(cx.subscribe_in(
            &lines,
            window,
            |this, _, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    this.apply_fields(cx);
                }
            },
        ));
        Fields {
            node: node.id.clone(),
            id,
            main,
            lines,
            value,
            id_taken: false,
            _subscriptions: subscriptions,
        }
    }

    /// Writes the inputs into the selected node.
    fn apply_fields(&mut self, cx: &mut Context<Self>) {
        let Some(fields) = &self.fields else { return };
        let text = |e: &Entity<InputState>| e.read(cx).value().to_string();
        let (new_id, main, lines, value) = (
            text(&fields.id).trim().to_owned(),
            text(&fields.main).trim().to_owned(),
            fields.lines.read(cx).value().to_string(),
            text(&fields.value),
        );
        let old = fields.node.clone();
        let pairs: BTreeMap<String, String> = lines
            .lines()
            .filter_map(|l| l.split_once('='))
            .map(|(k, v)| (k.trim().to_owned(), v.trim().to_owned()))
            .filter(|(k, _)| !k.is_empty())
            .collect();
        let Some(node) = self.flow.node_mut(&old) else {
            return;
        };
        match &mut node.kind {
            NodeKind::Request { request, extract } => {
                *request = main;
                *extract = pairs;
            }
            NodeKind::Check {
                subject, value: v, ..
            } => {
                *subject = main;
                *v = value;
            }
            NodeKind::Set { values } => *values = pairs,
            NodeKind::Delay { ms } => *ms = main.parse().unwrap_or(0),
        }
        let taken = new_id != old && self.flow.node(&new_id).is_some();
        if !new_id.is_empty() && new_id != old && !taken {
            self.rename(&old, &new_id);
        }
        if let Some(fields) = &mut self.fields {
            fields.id_taken = taken;
        }
        self.changed(cx);
    }

    fn rename(&mut self, old: &str, new: &str) {
        if let Some(node) = self.flow.node_mut(old) {
            node.id = new.to_owned();
        }
        for edge in &mut self.flow.edges {
            if edge.from == old {
                edge.from = new.to_owned();
            }
            if edge.to == old {
                edge.to = new.to_owned();
            }
        }
        if self.flow.start.as_deref() == Some(old) {
            self.flow.start = Some(new.to_owned());
        }
        self.selected = Some(new.to_owned());
        if let Some(fields) = &mut self.fields {
            fields.node = new.to_owned();
        }
    }

    fn add(&mut self, kind: NodeKind, window: &mut Window, cx: &mut Context<Self>) {
        let n = self.flow.nodes.len();
        let (x, y) = (
            20. + (n % 4) as f32 * 190.,
            24. + ((n / 4) % 4) as f32 * 108.,
        );
        let id = self.flow.new_id(kind.name());
        self.flow.nodes.push(Node {
            id: id.clone(),
            kind,
            x,
            y,
        });
        self.changed(cx);
        self.select(Some(id), window, cx);
    }

    fn remove_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(id) = self.selected.clone() {
            self.flow.remove(&id);
            self.changed(cx);
            self.select(None, window, cx);
        }
    }

    fn set_op(&mut self, op: Op, cx: &mut Context<Self>) {
        if let Some(id) = &self.selected
            && let Some(Node {
                kind: NodeKind::Check { op: o, .. },
                ..
            }) = self.flow.node_mut(id)
        {
            *o = op;
            self.changed(cx);
        }
    }

    fn pick_request(&mut self, path: String, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(fields) = &self.fields {
            // A node still called after its kind takes the request's name.
            let base = "request";
            let default_id = fields.node == base
                || fields
                    .node
                    .strip_prefix(base)
                    .and_then(|rest| rest.strip_prefix('-'))
                    .is_some_and(|n| n.parse::<u32>().is_ok());
            if default_id {
                let stem = std::path::Path::new(&path)
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let id = self.flow.new_id(&stem);
                fields.id.update(cx, |s, cx| s.set_value(id, window, cx));
            }
            fields
                .main
                .update(cx, |s, cx| s.set_value(path, window, cx));
        }
        self.apply_fields(cx);
    }

    // ---------------------------------------------------------------
    // The canvas: pressing, dragging, connecting
    // ---------------------------------------------------------------

    fn press_node(
        &mut self,
        id: String,
        at: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some((from, port)) = self.connecting.take() {
            self.flow.connect(&from, &port, &id);
            self.changed(cx);
            return;
        }
        if let Some(node) = self.flow.node(&id) {
            self.drag = Some(Drag {
                node: id.clone(),
                start: at,
                from: (node.x, node.y),
                moved: false,
            });
        }
        if self.selected.as_deref() != Some(id.as_str()) {
            self.select(Some(id), window, cx);
        }
    }

    fn move_drag(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        let Some(drag) = &mut self.drag else { return };
        if event.pressed_button != Some(MouseButton::Left) {
            self.drag = None;
            return;
        }
        let dx = f32::from(event.position.x - drag.start.x);
        let dy = f32::from(event.position.y - drag.start.y);
        if dx.abs() + dy.abs() < 2. && !drag.moved {
            return;
        }
        drag.moved = true;
        let (x, y) = (
            (drag.from.0 + dx).clamp(0., CANVAS_W - NODE_W - 8.),
            (drag.from.1 + dy).clamp(0., CANVAS_H - NODE_H),
        );
        let id = drag.node.clone();
        if let Some(node) = self.flow.node_mut(&id) {
            node.x = x.round();
            node.y = y.round();
        }
        cx.notify();
    }

    fn end_drag(&mut self, cx: &mut Context<Self>) {
        if let Some(drag) = self.drag.take()
            && drag.moved
        {
            self.changed(cx);
        }
    }

    // ---------------------------------------------------------------
    // Running
    // ---------------------------------------------------------------

    fn run(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_running() {
            return;
        }
        let problems = self.flow.problems();
        if !problems.is_empty() {
            window.push_notification(Notification::error(problems.join("\n")), cx);
            return;
        }
        if self.dirty {
            self.save(window, cx);
        }
        let (project, collection, env) = (
            self.project.as_ref(),
            Some(&self.collection),
            self.environment.as_deref(),
        );
        let vars = match project::variables(project, collection, env) {
            Err(StoreError::UnknownEnvironment(_)) => project::variables(project, collection, None),
            other => other,
        };
        let vars = match vars {
            Ok(vars) => vars,
            Err(e) => {
                window.push_notification(Notification::error(super::describe(&e)), cx);
                return;
            }
        };
        let plan = FlowPlan {
            flow: self.flow.clone(),
            base: self.collection.root.clone(),
            vars,
            scopes: project::scope_values(project, collection, env).unwrap_or_default(),
            environment: self.environment.clone(),
            state: self.script_vars.clone(),
            ..Default::default()
        };
        let scope = Scope {
            collection: Some(self.collection.clone()),
            project: self.project.clone(),
        };
        self.steps.clear();
        self.report = None;
        self.running_node = None;
        let control = Arc::new(Control::default());
        let live = Arc::new(Mutex::new(Live::default()));
        self.control = Some(control.clone());
        self.live = live.clone();
        let client = net::client().clone();
        let work = async move {
            let report = apix_runner::flow::run(
                &scope,
                &plan,
                &client,
                net::script_sender(),
                &control,
                |progress| {
                    let mut live = live.lock().unwrap_or_else(|e| e.into_inner());
                    match progress {
                        FlowProgress::Node(id) => live.current = Some(id.to_owned()),
                        FlowProgress::Step(step) => {
                            live.current = None;
                            live.steps.push(step.clone());
                        }
                    }
                },
            )
            .await;
            live.lock().unwrap_or_else(|e| e.into_inner()).report = Some(report);
        };
        let running = net::spawn(work);
        self._task = Some(cx.spawn_in(window, async move |this, cx| {
            let _running = running;
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;
                if this.update(cx, |this, cx| this.sync(cx)).unwrap_or(true) {
                    break;
                }
            }
        }));
        cx.notify();
    }

    fn sync(&mut self, cx: &mut Context<Self>) -> bool {
        let mut live = self.live.lock().unwrap_or_else(|e| e.into_inner());
        if live.steps.len() > self.steps.len() {
            self.steps
                .extend(live.steps[self.steps.len()..].iter().cloned());
        }
        self.running_node = live.current.clone();
        let report = live.report.take();
        drop(live);
        let done = report.is_some();
        if report.is_some() {
            self.report = report;
            self.control = None;
            self.running_node = None;
        }
        cx.notify();
        done
    }

    fn stop(&mut self, cx: &mut Context<Self>) {
        if let Some(control) = &self.control {
            control.resume();
            control.stop();
        }
        cx.notify();
    }

    /// A node's colour from the run: running, or its last outcome.
    fn status_color(&self, id: &str, p: &crate::theme::Palette) -> Option<Hsla> {
        if self.running_node.as_deref() == Some(id) {
            return Some(p.accent);
        }
        let step = self.steps.iter().rev().find(|s| s.node == id)?;
        Some(match step.port.as_deref() {
            Some("fail") => p.err,
            Some("no") => p.warn,
            Some(_) => p.ok,
            None => p.text_muted,
        })
    }
}

/// A short line under a node's id.
fn summary(kind: &NodeKind) -> String {
    match kind {
        NodeKind::Request { request, .. } if request.is_empty() => "?".into(),
        NodeKind::Request { request, .. } => request.clone(),
        NodeKind::Check { subject, op, value } if op.takes_value() => {
            format!("{subject} {} {value}", op_symbol(*op))
        }
        NodeKind::Check { subject, op, .. } => format!("{subject} {}", op_symbol(*op)),
        NodeKind::Set { values } => values.keys().cloned().collect::<Vec<_>>().join(", "),
        NodeKind::Delay { ms } => format!("{ms} ms"),
    }
}

fn op_symbol(op: Op) -> &'static str {
    match op {
        Op::Equals => "=",
        Op::NotEquals => "≠",
        Op::Contains => "∋",
        Op::Greater => ">",
        Op::Less => "<",
        Op::Exists => "exists",
        Op::Missing => "missing",
    }
}

fn op_label(op: Op) -> SharedString {
    match op {
        Op::Equals => tr!("flows.op.equals"),
        Op::NotEquals => tr!("flows.op.not_equals"),
        Op::Contains => tr!("flows.op.contains"),
        Op::Greater => tr!("flows.op.greater"),
        Op::Less => tr!("flows.op.less"),
        Op::Exists => tr!("flows.op.exists"),
        Op::Missing => tr!("flows.op.missing"),
    }
}

fn kind_label(kind: &NodeKind) -> SharedString {
    match kind {
        NodeKind::Request { .. } => tr!("flows.kind.request"),
        NodeKind::Check { .. } => tr!("flows.kind.check"),
        NodeKind::Set { .. } => tr!("flows.kind.set"),
        NodeKind::Delay { .. } => tr!("flows.kind.delay"),
    }
}

impl Render for FlowsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex().gap_3().child(self.render_toolbar(cx)).child(
            h_flex()
                .gap_3()
                .items_start()
                .child(
                    v_flex()
                        .gap_2()
                        .flex_none()
                        .child(self.render_hint(cx))
                        .child(self.render_canvas(cx))
                        .child(self.render_log(cx)),
                )
                .child(self.render_panel(cx)),
        )
    }
}

impl FlowsView {
    fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = crate::theme::current();
        let tabs = self.files.iter().enumerate().map(|(i, path)| {
            let label = path
                .strip_prefix(&self.collection.root)
                .unwrap_or(path)
                .with_extension("")
                .to_string_lossy()
                .replace('\\', "/");
            let current = self.current.as_ref() == Some(path);
            let path = path.clone();
            Button::new(("flow-file", i))
                .small()
                .map(|b| if current { b.primary() } else { b.ghost() })
                .label(label)
                .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                    if !this.is_running() {
                        this.open(path.clone(), window, cx)
                    }
                }))
        });
        let status = match (&self.report, self.is_running()) {
            (_, true) => Some((
                tr!(
                    "flows.running",
                    node = self.running_node.clone().unwrap_or_default()
                ),
                p.accent,
            )),
            (Some(r), false) if r.stopped => Some((tr!("flows.stopped"), p.warn)),
            (Some(r), false) if r.passed => Some((
                tr!("flows.passed", steps = r.steps.len(), ms = r.duration_ms),
                p.ok,
            )),
            (Some(r), false) => Some((
                match &r.ended {
                    Some(why) => why.clone().into(),
                    None => tr!("flows.failed", steps = r.steps.len()),
                },
                p.err,
            )),
            _ => None,
        };
        h_flex()
            .gap_2()
            .items_center()
            .flex_wrap()
            .children(tabs)
            .child(div().w(px(180.)).child(Input::new(&self.new_name).small()))
            .child(
                Button::new("flow-create")
                    .small()
                    .outline()
                    .label(tr!("flows.create"))
                    .on_click(
                        cx.listener(|this, _: &ClickEvent, window, cx| this.create(window, cx)),
                    ),
            )
            .child(div().flex_1())
            .children(status.map(|(text, color)| div().text_sm().text_color(color).child(text)))
            .when(self.current.is_some(), |row| {
                row.child(
                    Button::new("flow-save")
                        .small()
                        .ghost()
                        .label(if self.dirty {
                            tr!("flows.save_dirty")
                        } else {
                            tr!("flows.save")
                        })
                        .disabled(!self.dirty)
                        .on_click(
                            cx.listener(|this, _: &ClickEvent, window, cx| this.save(window, cx)),
                        ),
                )
                .child(if self.is_running() {
                    Button::new("flow-stop")
                        .small()
                        .danger()
                        .label(tr!("flows.stop"))
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.stop(cx)))
                } else {
                    Button::new("flow-run")
                        .small()
                        .primary()
                        .label(tr!("flows.run"))
                        .on_click(
                            cx.listener(|this, _: &ClickEvent, window, cx| this.run(window, cx)),
                        )
                })
            })
    }

    fn render_hint(&self, _: &mut Context<Self>) -> impl IntoElement {
        let p = crate::theme::current();
        let text = match (&self.connecting, &self.load_error, self.current.is_some()) {
            (Some((from, port)), _, _) => tr!(
                "flows.connect_hint",
                node = from.clone(),
                port = port.clone()
            ),
            (None, Some(error), _) => error.clone().into(),
            (None, None, false) => tr!("flows.none"),
            (None, None, true) => tr!("flows.canvas_hint"),
        };
        let color = if self.load_error.is_some() && self.connecting.is_none() {
            p.err
        } else if self.connecting.is_some() {
            p.accent
        } else {
            p.text_muted
        };
        div()
            .w(px(CANVAS_W))
            .text_xs()
            .text_color(color)
            .child(bidi::para(text))
    }

    fn render_canvas(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = crate::theme::current();
        // Edges: a curve from the outcome's dot to the left middle of the target.
        // Forward edges are curves; one going back (to a node on its left) runs
        // around the nodes, through the gap between rows.
        let mut lines: Vec<(Vec<(f32, f32)>, Hsla)> = Vec::new();
        for edge in &self.flow.edges {
            let (Some(from), Some(to)) = (self.flow.node(&edge.from), self.flow.node(&edge.to))
            else {
                continue;
            };
            let index = from
                .kind
                .ports()
                .iter()
                .position(|p| *p == edge.port)
                .unwrap_or(0);
            let color = match edge.port.as_str() {
                "fail" => p.err.opacity(0.8),
                "no" => p.warn.opacity(0.8),
                _ => p.text_muted,
            };
            let (x1, y1) = port_at(from, index);
            let (x2, y2) = (to.x, to.y + NODE_H / 2.);
            let route = if x2 >= x1 + 4. {
                vec![(x1, y1), (x2, y2)]
            } else {
                let (src_bottom, dst_bottom) = (from.y + NODE_H, to.y + NODE_H);
                let lane = if to.y > src_bottom + 12. {
                    (src_bottom + to.y) / 2.
                } else if from.y > dst_bottom + 12. {
                    (dst_bottom + from.y) / 2.
                } else {
                    src_bottom.max(dst_bottom) + 14. + index as f32 * 6.
                }
                .min(CANVAS_H - 4.);
                vec![
                    (x1, y1),
                    (x1 + 14., y1),
                    (x1 + 14., lane),
                    (x2 - 16., lane),
                    (x2 - 16., y2),
                    (x2, y2),
                ]
            };
            lines.push((route, color));
        }
        let edges = canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                let o = bounds.origin;
                let at = |x: f32, y: f32| point(o.x + px(x), o.y + px(y));
                for (route, color) in &lines {
                    let (x1, y1) = route[0];
                    let (x2, y2) = route[route.len() - 1];
                    let mut path = PathBuilder::stroke(px(1.5));
                    path.move_to(at(x1, y1));
                    if route.len() == 2 {
                        let reach = ((x2 - x1).abs() / 2.).max(30.);
                        path.cubic_bezier_to(
                            at(x2 - 6., y2),
                            at(x1 + reach, y1),
                            at(x2 - reach, y2),
                        );
                    } else {
                        for (x, y) in &route[1..route.len() - 1] {
                            path.line_to(at(*x, *y));
                        }
                        path.line_to(at(x2 - 6., y2));
                    }
                    if let Ok(path) = path.build() {
                        window.paint_path(path, *color);
                    }
                    let mut head = PathBuilder::fill();
                    head.move_to(at(x2, y2));
                    head.line_to(at(x2 - 8., y2 - 4.5));
                    head.line_to(at(x2 - 8., y2 + 4.5));
                    head.close();
                    if let Ok(head) = head.build() {
                        window.paint_path(head, *color);
                    }
                }
            },
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full();
        let start = self.flow.start_node().map(str::to_owned);
        let nodes = self.flow.nodes.iter().map(|node| {
            let id = node.id.clone();
            let selected = self.selected.as_deref() == Some(id.as_str());
            let status = self.status_color(&id, &p);
            let border = status.unwrap_or(if selected { p.accent } else { p.hairline });
            let ports = node.kind.ports().iter().enumerate().map(|(i, port)| {
                let (from, port_name) = (id.clone(), (*port).to_owned());
                let connected = self.flow.next(&id, port).is_some();
                let active = self
                    .connecting
                    .as_ref()
                    .is_some_and(|(n, pt)| *n == id && pt == port);
                let dot = match *port {
                    "fail" => p.err,
                    "no" => p.warn,
                    _ => p.ok,
                };
                div()
                    .absolute()
                    .top(px(30. + i as f32 * 22. - 8.))
                    .right(px(-7.))
                    .h(px(16.))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_1()
                    .child(div().text_xs().text_color(p.text_muted).child(*port))
                    .child(
                        div()
                            .id(SharedString::from(format!("port-{id}-{port}")))
                            .size(px(13.))
                            .rounded_full()
                            .border_2()
                            .border_color(dot)
                            .when(connected || active, |d| d.bg(dot))
                            .when(!connected && !active, |d| d.bg(p.surface))
                            .cursor_pointer()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                                    cx.stop_propagation();
                                    this.connecting = Some((from.clone(), port_name.clone()));
                                    cx.notify();
                                }),
                            ),
                    )
            });
            let press = id.clone();
            div()
                .id(SharedString::from(format!("node-{id}")))
                .absolute()
                .left(px(node.x))
                .top(px(node.y))
                .w(px(NODE_W))
                .h(px(NODE_H))
                .rounded_md()
                .border_2()
                .border_color(border)
                .bg(if selected {
                    p.accent.opacity(0.12)
                } else {
                    p.surface_raised
                })
                .px_2()
                .py_1()
                .cursor_pointer()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, e: &MouseDownEvent, window, cx| {
                        cx.stop_propagation();
                        this.press_node(press.clone(), e.position, window, cx);
                    }),
                )
                .child(
                    h_flex()
                        .gap_1()
                        .items_center()
                        .child(
                            div()
                                .text_xs()
                                .text_color(p.accent)
                                .child(kind_label(&node.kind)),
                        )
                        .when(start.as_deref() == Some(id.as_str()), |row| {
                            row.child(
                                div()
                                    .text_xs()
                                    .px_1()
                                    .rounded_sm()
                                    .bg(p.accent.opacity(0.18))
                                    .child(tr!("flows.start")),
                            )
                        }),
                )
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .w(px(NODE_W - 50.))
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .child(id.clone()),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(p.text_muted)
                        .w(px(NODE_W - 50.))
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .child(summary(&node.kind)),
                )
                .children(ports)
        });
        div()
            .id("flow-canvas")
            .relative()
            .flex_none()
            .w(px(CANVAS_W))
            .h(px(CANVAS_H))
            .rounded_md()
            .border_1()
            .border_color(p.hairline)
            .bg(p.surface)
            .overflow_hidden()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _: &MouseDownEvent, window, cx| {
                    if this.connecting.take().is_some() {
                        cx.notify();
                    } else if this.selected.is_some() {
                        this.select(None, window, cx);
                    }
                }),
            )
            .on_mouse_move(cx.listener(|this, e: &MouseMoveEvent, _, cx| this.move_drag(e, cx)))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _: &MouseUpEvent, _, cx| this.end_drag(cx)),
            )
            .child(edges)
            .children(nodes)
    }

    fn render_log(&self, _: &mut Context<Self>) -> impl IntoElement {
        let p = crate::theme::current();
        let problems = if self.current.is_some() && !self.flow.nodes.is_empty() {
            self.flow.problems()
        } else {
            Vec::new()
        };
        let rows = self.steps.iter().map(|step| {
            let color = match step.port.as_deref() {
                Some("fail") => p.err,
                Some("no") => p.warn,
                Some(_) => p.ok,
                None => p.text_muted,
            };
            let status = step
                .item
                .as_ref()
                .map(|i| match i.status {
                    Some(s) => format!("{s} · {:.0} ms", i.time_ms),
                    None => i.error.clone().unwrap_or_default(),
                })
                .unwrap_or_default();
            h_flex()
                .gap_2()
                .text_xs()
                .child(
                    div()
                        .w(px(120.))
                        .flex_none()
                        .overflow_hidden()
                        .child(step.node.clone()),
                )
                .child(
                    div()
                        .w(px(40.))
                        .flex_none()
                        .text_color(color)
                        .child(step.port.clone().unwrap_or_default()),
                )
                .child(
                    div()
                        .w(px(140.))
                        .flex_none()
                        .text_color(p.text_muted)
                        .child(status),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .text_color(p.text_muted)
                        .child(step.note.clone()),
                )
        });
        v_flex()
            .id("flow-log")
            .w(px(CANVAS_W))
            .max_h(px(140.))
            .overflow_y_scroll()
            .gap_0p5()
            .children(
                problems
                    .into_iter()
                    .map(|problem| div().text_xs().text_color(p.err).child(problem)),
            )
            .children(rows)
    }

    fn render_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = crate::theme::current();
        let label = |text: SharedString| div().text_xs().text_color(p.text_muted).child(text);
        let add = |id: &'static str, text: SharedString, kind: NodeKind| {
            Button::new(id)
                .small()
                .outline()
                .label(text)
                .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                    this.add(kind.clone(), window, cx)
                }))
        };
        let mut panel = v_flex().flex_1().min_w_0().gap_2();
        if self.current.is_none() {
            return panel;
        }
        panel = panel.child(label(tr!("flows.add"))).child(
            h_flex()
                .gap_1()
                .flex_wrap()
                .child(add(
                    "flow-add-request",
                    tr!("flows.kind.request"),
                    NodeKind::Request {
                        request: String::new(),
                        extract: BTreeMap::new(),
                    },
                ))
                .child(add(
                    "flow-add-check",
                    tr!("flows.kind.check"),
                    NodeKind::Check {
                        subject: "status".into(),
                        op: Op::Equals,
                        value: "200".into(),
                    },
                ))
                .child(add(
                    "flow-add-set",
                    tr!("flows.kind.set"),
                    NodeKind::Set {
                        values: BTreeMap::new(),
                    },
                ))
                .child(add(
                    "flow-add-delay",
                    tr!("flows.kind.delay"),
                    NodeKind::Delay { ms: 1000 },
                )),
        );
        let (Some(fields), Some(node)) = (
            &self.fields,
            self.selected.as_deref().and_then(|id| self.flow.node(id)),
        ) else {
            return panel.child(
                div()
                    .pt_2()
                    .text_sm()
                    .text_color(p.text_muted)
                    .child(bidi::para(tr!("flows.select_hint"))),
            );
        };
        panel = panel
            .child(div().h(px(1.)).bg(p.hairline))
            .child(label(tr!("flows.id")))
            .child(Input::new(&fields.id).small())
            .when(fields.id_taken, |d| {
                d.child(
                    div()
                        .text_xs()
                        .text_color(p.err)
                        .child(tr!("flows.id_taken")),
                )
            });
        match &node.kind {
            NodeKind::Request { request, .. } => {
                let typed = request.to_lowercase();
                let choices: Vec<String> = self
                    .requests
                    .iter()
                    .filter(|r| typed.is_empty() || r.to_lowercase().contains(&typed))
                    .filter(|r| **r != *request)
                    .take(40)
                    .cloned()
                    .collect();
                panel = panel
                    .child(label(tr!("flows.request")))
                    .child(Input::new(&fields.main).small())
                    .when(!choices.is_empty(), |d| {
                        d.child(
                            v_flex()
                                .id("flow-requests")
                                .max_h(px(110.))
                                .overflow_y_scroll()
                                .rounded_md()
                                .border_1()
                                .border_color(p.hairline)
                                .children(choices.into_iter().enumerate().map(|(i, path)| {
                                    let pick = path.clone();
                                    div()
                                        .id(("flow-pick", i))
                                        .px_2()
                                        .text_xs()
                                        .cursor_pointer()
                                        .hover(|s| s.bg(p.hover))
                                        .child(path)
                                        .on_click(cx.listener(
                                            move |this, _: &ClickEvent, window, cx| {
                                                this.pick_request(pick.clone(), window, cx)
                                            },
                                        ))
                                })),
                        )
                    })
                    .child(label(tr!("flows.extract")))
                    .child(Textarea::new(&fields.lines).small());
            }
            NodeKind::Check { op, .. } => {
                let current = *op;
                panel = panel
                    .child(label(tr!("flows.subject")))
                    .child(Input::new(&fields.main).small())
                    .child(
                        h_flex()
                            .gap_1()
                            .flex_wrap()
                            .children(Op::ALL.iter().map(|op| {
                                let op = *op;
                                Button::new(SharedString::from(format!("flow-op-{}", op.name())))
                                    .xsmall()
                                    .map(|b| {
                                        if op == current {
                                            b.primary()
                                        } else {
                                            b.ghost()
                                        }
                                    })
                                    .label(op_label(op))
                                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                        this.set_op(op, cx)
                                    }))
                            })),
                    )
                    .when(current.takes_value(), |d| {
                        d.child(label(tr!("flows.value")))
                            .child(Input::new(&fields.value).small())
                    });
            }
            NodeKind::Set { .. } => {
                panel = panel
                    .child(label(tr!("flows.values")))
                    .child(Textarea::new(&fields.lines).small());
            }
            NodeKind::Delay { .. } => {
                panel = panel
                    .child(label(tr!("flows.ms")))
                    .child(Input::new(&fields.main).small());
            }
        }
        // Where each outcome leads.
        let id = node.id.clone();
        let outcomes = node.kind.ports().iter().enumerate().map(|(i, port)| {
            let target = self.flow.next(&id, port).map(str::to_owned);
            let (from, port_name) = (id.clone(), (*port).to_owned());
            h_flex()
                .gap_2()
                .items_center()
                .text_xs()
                .child(div().w(px(40.)).child(*port))
                .child(div().flex_1().child(match &target {
                    Some(t) => format!("→ {t}"),
                    None => format!("→ {}", tr!("flows.end")),
                }))
                .when(target.is_some(), |row| {
                    row.child(
                        Button::new(("flow-unlink", i))
                            .xsmall()
                            .ghost()
                            .label("×")
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.flow
                                    .edges
                                    .retain(|e| !(e.from == from && e.port == port_name));
                                this.changed(cx);
                            })),
                    )
                })
        });
        let is_start = self.flow.start_node() == Some(node.id.as_str());
        let start_id = node.id.clone();
        panel
            .child(label(tr!("flows.outcomes")))
            .children(outcomes)
            .child(
                h_flex()
                    .gap_2()
                    .pt_1()
                    .when(!is_start, |row| {
                        row.child(
                            Button::new("flow-start-here")
                                .small()
                                .outline()
                                .label(tr!("flows.start_here"))
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    this.flow.start = Some(start_id.clone());
                                    this.changed(cx);
                                })),
                        )
                    })
                    .child(
                        Button::new("flow-remove-node")
                            .small()
                            .ghost()
                            .label(tr!("flows.remove"))
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.remove_selected(window, cx)
                            })),
                    ),
            )
    }
}
