//! The route window (View › Route and map…, or the link in the Network
//! tab): the hops to a server from the system's traceroute, each placed with
//! an IP location database on disk, and drawn on a world map. The database
//! is optional; the window offers to download DB-IP's free one.

use std::net::IpAddr;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use apix_route::{Geo, Hop, Place};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{IconName, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::{ResponseState, Workspace};
use crate::bidi::{self, Bidi as _, DialogBidi as _};
use crate::{net, tr};

const MAP_W: f32 = 600.;
const MAP_H: f32 = 300.;

/// The database, read once per run of the app (it can be 130 MB).
fn geo_cache() -> &'static Mutex<Option<Arc<Geo>>> {
    static GEO: OnceLock<Mutex<Option<Arc<Geo>>>> = OnceLock::new();
    GEO.get_or_init(|| Mutex::new(None))
}

/// The database: the one read before, or the one on disk now.
fn load_geo() -> Result<Option<Arc<Geo>>, String> {
    if let Some(geo) = geo_cache().lock().ok().and_then(|g| g.clone()) {
        return Ok(Some(geo));
    }
    match apix_route::default_geo() {
        None => Ok(None),
        Some(Err(e)) => Err(e),
        Some(Ok(geo)) => {
            let geo = Arc::new(geo);
            if let Ok(mut cache) = geo_cache().lock() {
                *cache = Some(geo.clone());
            }
            Ok(Some(geo))
        }
    }
}

#[derive(Clone)]
struct Line {
    hop: Hop,
    place: Option<Place>,
}

#[derive(Default)]
struct Run {
    lines: Vec<Line>,
    done: bool,
    error: Option<String>,
}

enum GeoState {
    Loading,
    Missing,
    Ready(Arc<Geo>),
    Failed(String),
    Downloading,
}

impl Workspace {
    pub(super) fn open_route(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // The server of the last response, or the host in the URL bar.
        let target = match &self.response {
            ResponseState::Done(done) => {
                done.response.trace.remote_addr.map(|a| a.ip().to_string())
            }
            _ => None,
        }
        .or_else(|| {
            let url = self.url.read(cx).value().to_string();
            (!url.contains("{{") && !url.trim().is_empty()).then(|| apix_route::host_of(&url))
        })
        .unwrap_or_default();
        let workspace = cx.entity().downgrade();
        let view = cx.new(|cx| RouteView::new(target, window, cx));
        window.open_dialog(cx, move |dialog, _, _| {
            dialog
                .on_close(super::collections::refocus(workspace.clone()))
                .bidi_title(tr!("route.title"))
                .width(px(960.))
                .child(div().pb_2().child(view.clone()))
        });
    }
}

pub(super) struct RouteView {
    target: Entity<InputState>,
    run: Arc<Mutex<Run>>,
    stopper: Arc<Mutex<Option<apix_route::Stopper>>>,
    running: bool,
    geo: GeoState,
    _clock: Option<Task<()>>,
    _geo_task: Option<Task<()>>,
}

impl RouteView {
    fn new(target: String, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| {
            let mut state = InputState::new(window, cx).placeholder("example.com");
            state.set_value(target.clone(), window, cx);
            state
        });
        let mut view = Self {
            target: input,
            run: Arc::default(),
            stopper: Arc::default(),
            running: false,
            geo: GeoState::Loading,
            _clock: None,
            _geo_task: None,
        };
        view.read_geo(cx);
        if !target.is_empty() {
            view.start(cx);
        }
        view
    }

    fn read_geo(&mut self, cx: &mut Context<Self>) {
        self.geo = GeoState::Loading;
        self._geo_task = Some(cx.spawn(async move |this, cx| {
            let found = cx.background_spawn(async move { load_geo() }).await;
            this.update(cx, |this, cx| {
                this.geo = match found {
                    Ok(Some(geo)) => GeoState::Ready(geo),
                    Ok(None) => GeoState::Missing,
                    Err(e) => GeoState::Failed(e),
                };
                this.place_known(cx);
                cx.notify();
            })
            .ok();
        }));
    }

    /// Places hops that arrived before the database was read.
    fn place_known(&mut self, _cx: &mut Context<Self>) {
        let GeoState::Ready(geo) = &self.geo else {
            return;
        };
        if let Ok(mut run) = self.run.lock() {
            for line in run.lines.iter_mut().filter(|l| l.place.is_none()) {
                line.place = line.hop.ip.and_then(|ip| geo.locate(ip));
            }
        }
    }

    fn start(&mut self, cx: &mut Context<Self>) {
        let target = apix_route::host_of(&self.target.read(cx).value());
        if target.is_empty() || self.running {
            return;
        }
        self.stop();
        let run = Arc::new(Mutex::new(Run::default()));
        self.run = run.clone();
        self.running = true;
        let geo = match &self.geo {
            GeoState::Ready(geo) => Some(geo.clone()),
            _ => None,
        };
        let stopper = self.stopper.clone();
        // The hops arrive on a thread of their own; the clock repaints.
        std::thread::spawn(move || {
            let trace = match apix_route::Trace::start(&target, 30) {
                Ok(trace) => trace,
                Err(e) => {
                    if let Ok(mut run) = run.lock() {
                        run.error = Some(e);
                        run.done = true;
                    }
                    return;
                }
            };
            if let Ok(mut s) = stopper.lock() {
                *s = Some(trace.stopper());
            }
            for hop in trace {
                let place = match (&geo, hop.ip) {
                    (Some(geo), Some(ip)) => geo.locate(ip),
                    _ => None,
                };
                if let Ok(mut run) = run.lock() {
                    run.lines.push(Line { hop, place });
                }
            }
            if let Ok(mut run) = run.lock() {
                run.done = true;
            }
        });
        self._clock = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(300))
                    .await;
                let finished = this
                    .update(cx, |this, cx| {
                        let done = this.run.lock().map(|r| r.done).unwrap_or(true);
                        if done {
                            this.running = false;
                        }
                        this.place_known(cx);
                        cx.notify();
                        done
                    })
                    .unwrap_or(true);
                if finished {
                    break;
                }
            }
        }));
        cx.notify();
    }

    fn stop(&mut self) {
        if let Some(stopper) = self.stopper.lock().ok().and_then(|mut s| s.take()) {
            stopper.stop();
        }
    }

    fn download(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(dir) = apix_route::geo_dir() else {
            return;
        };
        self.geo = GeoState::Downloading;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or_default();
        let urls = apix_route::dbip_urls(now);
        self._geo_task = Some(cx.spawn_in(window, async move |this, cx| {
            let mut result = Err(String::new());
            for url in urls {
                let name = url
                    .rsplit('/')
                    .next()
                    .unwrap_or("dbip.mmdb.gz")
                    .trim_end_matches(".gz")
                    .to_owned();
                let mut request = apix_core::Request::new(apix_core::Method::Get, url.clone());
                request.settings.timeout_ms = Some(600_000);
                result = match net::send(request).await {
                    Ok(r) if r.is_success() => {
                        let (body, dir) = (r.body.clone(), dir.clone());
                        cx.background_spawn(async move { apix_route::install(&body, &dir, &name) })
                            .await
                    }
                    Ok(r) => Err(format!("{url}: HTTP {}", r.status)),
                    Err(e) => Err(format!("{url}: {e}")),
                };
                if result.is_ok() {
                    break;
                }
            }
            this.update_in(cx, |this, window, cx| match result {
                Ok(path) => {
                    if let Ok(mut cache) = geo_cache().lock() {
                        *cache = None;
                    }
                    window.push_notification(
                        gpui_kit::component::notification::Notification::success(tr!(
                            "route.downloaded",
                            file = path
                                .file_name()
                                .map(|n| n.to_string_lossy().into_owned())
                                .unwrap_or_default()
                        )),
                        cx,
                    );
                    this.read_geo(cx);
                }
                Err(e) => {
                    this.geo = GeoState::Failed(e);
                    cx.notify();
                }
            })
            .ok();
        }));
        cx.notify();
    }

    fn render_map(&self, lines: &[Line]) -> impl IntoElement {
        let p = crate::theme::current();
        let points: Vec<(f32, f32)> = lines
            .iter()
            .filter_map(|l| l.place.as_ref())
            .map(|place| {
                let (x, y) = apix_route::project(
                    place.latitude,
                    place.longitude,
                    f64::from(MAP_W),
                    f64::from(MAP_H),
                );
                (x as f32, y as f32)
            })
            .collect();
        let (water, land, grid, route) = (p.surface, p.hover, p.hairline, p.accent);
        let dots = points.clone();
        let map = canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                let origin = bounds.origin;
                let at = |x: f32, y: f32| point(origin.x + px(x), origin.y + px(y));
                let shape = |outline: &[(f32, f32)]| {
                    let mut path = PathBuilder::fill();
                    for (i, (lon, lat)) in outline.iter().enumerate() {
                        let (x, y) = apix_route::project(
                            f64::from(*lat),
                            f64::from(*lon),
                            f64::from(MAP_W),
                            f64::from(MAP_H),
                        );
                        let to = at(x as f32, y as f32);
                        if i == 0 {
                            path.move_to(to);
                        } else {
                            path.line_to(to);
                        }
                    }
                    path.close();
                    path.build().ok()
                };
                window.paint_quad(fill(bounds, water));
                for lat in [-60., -30., 0., 30., 60.] {
                    let y = (90. - lat) / 180. * MAP_H;
                    let mut line = PathBuilder::stroke(px(0.5));
                    line.move_to(at(0., y));
                    line.line_to(at(MAP_W, y));
                    if let Ok(path) = line.build() {
                        window.paint_path(path, grid);
                    }
                }
                for outline in apix_route::land::LAND {
                    if let Some(path) = shape(outline) {
                        window.paint_path(path, land);
                    }
                }
                for outline in apix_route::land::WATER {
                    if let Some(path) = shape(outline) {
                        window.paint_path(path, water);
                    }
                }
                if dots.len() > 1 {
                    let mut line = PathBuilder::stroke(px(2.));
                    line.move_to(at(dots[0].0, dots[0].1));
                    for (x, y) in &dots[1..] {
                        line.line_to(at(*x, *y));
                    }
                    if let Ok(path) = line.build() {
                        window.paint_path(path, route);
                    }
                }
                for (i, (x, y)) in dots.iter().enumerate() {
                    let r = if i + 1 == dots.len() { 5. } else { 3.5 };
                    window.paint_quad(
                        fill(
                            Bounds::new(at(x - r, y - r), size(px(2. * r), px(2. * r))),
                            route,
                        )
                        .corner_radii(px(r)),
                    );
                }
            },
        )
        .w(px(MAP_W))
        .h(px(MAP_H));
        div()
            .relative()
            .flex_none()
            .w(px(MAP_W))
            .h(px(MAP_H))
            .rounded_md()
            .overflow_hidden()
            .border_1()
            .border_color(p.hairline)
            .child(map)
    }
}

fn hop_time(hop: &Hop) -> String {
    match hop.rtt_ms {
        Some(ms) if ms < 1.0 => "<1 ms".into(),
        Some(ms) => format!("{ms:.0} ms"),
        None => String::new(),
    }
}

impl Render for RouteView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let _ = window;
        let p = crate::theme::current();
        let (lines, error) = self
            .run
            .lock()
            .map(|r| (r.lines.clone(), r.error.clone()))
            .unwrap_or_default();
        let running = self.running;

        let controls = h_flex()
            .dir_row()
            .gap_2()
            .items_center()
            .child(
                div()
                    .text_xs()
                    .text_color(p.text_muted)
                    .child(tr!("route.target")),
            )
            .child(div().flex_1().child(Input::new(&self.target).small()))
            .child(if running {
                Button::new("route-stop")
                    .outline()
                    .small()
                    .icon(IconName::Square)
                    .label(tr!("mock.stop"))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.stop();
                        cx.notify();
                    }))
            } else {
                Button::new("route-start")
                    .primary()
                    .small()
                    .icon(IconName::Play)
                    .label(tr!("route.trace"))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.start(cx)))
            });

        let list = lines.iter().map(|line| {
            let hop = &line.hop;
            let place = match hop.ip {
                Some(ip) if apix_route::is_local(ip) => tr!("route.local").to_string(),
                Some(_) => line.place.as_ref().map(Place::label).unwrap_or_default(),
                None => String::new(),
            };
            h_flex()
                .gap_2()
                .text_xs()
                .py(px(2.))
                .child(
                    div()
                        .w(px(20.))
                        .flex_none()
                        .text_color(p.text_muted)
                        .child(hop.ttl.to_string()),
                )
                .child(
                    div()
                        .w(px(120.))
                        .flex_none()
                        .truncate()
                        .font_family(crate::fonts::MONO)
                        .child(
                            hop.ip
                                .map_or_else(|| "*".into(), |ip: IpAddr| ip.to_string()),
                        ),
                )
                .child(div().w(px(46.)).flex_none().text_end().child(hop_time(hop)))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_color(p.text_muted)
                        .child(place),
                )
        });
        let left = v_flex().w(px(300.)).flex_none().gap_1().child(
            div()
                .id("route-hops")
                .h(px(MAP_H))
                .overflow_y_scroll()
                .children(list)
                .when(lines.is_empty(), |d| {
                    d.child(div().text_sm().text_color(p.text_muted).child(if running {
                        tr!("route.waiting")
                    } else {
                        tr!("route.empty")
                    }))
                }),
        );

        let geo_line: AnyElement = match &self.geo {
            GeoState::Loading => div().child(tr!("route.geo_loading")).into_any_element(),
            GeoState::Downloading => div().child(tr!("route.downloading")).into_any_element(),
            GeoState::Ready(geo) => div()
                .child(
                    if geo
                        .path
                        .to_string_lossy()
                        .to_ascii_lowercase()
                        .contains("dbip")
                    {
                        SharedString::from(apix_route::DBIP_CREDIT)
                    } else {
                        tr!(
                            "route.geo_file",
                            file = geo
                                .path
                                .file_name()
                                .map(|n| n.to_string_lossy().into_owned())
                                .unwrap_or_default()
                        )
                    },
                )
                .into_any_element(),
            GeoState::Missing | GeoState::Failed(_) => {
                let dir = apix_route::geo_dir().unwrap_or_default();
                let failed = match &self.geo {
                    GeoState::Failed(e) => Some(e.clone()),
                    _ => None,
                };
                v_flex()
                    .gap_1()
                    .children(failed.map(|e| div().text_color(p.err).child(e)))
                    .child(bidi::para(tr!(
                        "route.no_geo",
                        folder = dir.display().to_string()
                    )))
                    .child(
                        h_flex()
                            .dir_row()
                            .gap_2()
                            .child(
                                Button::new("route-download")
                                    .small()
                                    .outline()
                                    .icon(IconName::ArrowDown)
                                    .label(tr!("route.download"))
                                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                        this.download(window, cx)
                                    })),
                            )
                            .child(
                                Button::new("route-folder")
                                    .small()
                                    .ghost()
                                    .label(tr!("route.open_folder"))
                                    .on_click(move |_, _, cx| {
                                        let _ = std::fs::create_dir_all(&dir);
                                        cx.open_url(&format!("file:///{}", dir.display()));
                                    }),
                            ),
                    )
                    .into_any_element()
            }
        };

        v_flex()
            .gap_3()
            .child(
                div()
                    .text_xs()
                    .text_color(p.text_muted)
                    .child(bidi::para(tr!("route.about"))),
            )
            .child(controls)
            .children(error.map(|e| div().text_sm().text_color(p.err).child(e)))
            .child(
                h_flex()
                    .dir_row()
                    .gap_3()
                    .items_start()
                    .child(left)
                    .child(self.render_map(&lines)),
            )
            .child(div().text_xs().text_color(p.text_muted).child(geo_line))
            .when(running, |d| {
                d.child(
                    div()
                        .text_xs()
                        .text_color(p.text_muted)
                        .child(tr!("route.running")),
                )
            })
    }
}
