//! The map: entities (and optionally their items) as nodes on a pan / zoom canvas, links
//! painted underneath. Layout comes from `vigia_core::graph`, computed once per data revision.
//! Click a node to open it; drag from one entity onto another to link them.
use crate::app::VigiaApp;
use crate::state::Data;
use crate::theme::{Tokens, tokens};
use crate::ui;
use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_component::button::Button;
use gpui_component::checkbox::Checkbox;
use gpui_component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_component::{IconName, Sizable as _};
use std::cell::Cell;
use std::rc::Rc;
use vigia_core::entity::EntityId;
use vigia_core::graph::{self, Camera, Graph, Layout, NodeRef};

const ENT_W: f32 = 190.;
const ENT_H: f32 = 44.;
const ITEM_W: f32 = 170.;
const ITEM_H: f32 = 30.;

#[derive(Clone, Copy)]
struct Pan {
    start: Point<Pixels>,
    camera: Camera,
}

#[derive(Clone, Copy)]
struct Link {
    from: usize,
    start: Point<Pixels>,
    cur: Point<Pixels>,
    moved: bool,
}

pub struct GraphState {
    pub show_items: bool,
    pub radial: bool,
    /// The entity a radial layout is centred on.
    pub center: Option<EntityId>,
    camera: Rc<Cell<Camera>>,
    need_fit: Rc<Cell<bool>>,
    bounds: Rc<Cell<Bounds<Pixels>>>,
    cache: Option<((u64, bool, Layout), Rc<Graph>)>,
    pan: Option<Pan>,
    link: Option<Link>,
}

impl Default for GraphState {
    fn default() -> Self {
        GraphState { show_items: true, radial: false, center: None, camera: Rc::default(), need_fit: Rc::new(Cell::new(true)), bounds: Rc::default(), cache: None, pan: None, link: None }
    }
}

impl GraphState {
    /// Layout settings changed: lay out again and fit.
    pub fn invalidate(&mut self) {
        self.cache = None;
        self.need_fit.set(true);
        if self.center.is_some() {
            self.radial = true;
        }
    }

    fn layout(&self) -> Layout {
        match (self.radial, self.center) {
            (true, Some(c)) => Layout::Radial(c),
            _ => Layout::Layered,
        }
    }

    fn local(&self, p: Point<Pixels>) -> (f32, f32) {
        let b = self.bounds.get();
        ((p.x - b.origin.x).into(), (p.y - b.origin.y).into())
    }

    fn zoom_by(&mut self, factor: f32, anchor: Option<(f32, f32)>) {
        let b = self.bounds.get();
        let a = anchor.unwrap_or((f32::from(b.size.width) / 2., f32::from(b.size.height) / 2.));
        let mut c = self.camera.get();
        let z = c.zoom * factor;
        c.zoom_around(a, z);
        self.camera.set(c);
    }
}

fn node_size(r: NodeRef) -> (f32, f32) {
    match r {
        NodeRef::Entity(_) => (ENT_W, ENT_H),
        NodeRef::Item(_) => (ITEM_W, ITEM_H),
    }
}

impl VigiaApp {
    fn graph_data(&mut self, cx: &mut Context<Self>) -> Rc<Graph> {
        let d = self.data.read(cx);
        let key = (d.generation, self.graph.show_items, self.graph.layout());
        if let Some((k, g)) = &self.graph.cache
            && *k == key
        {
            return g.clone();
        }
        let g = Rc::new(graph::build(&d.entities, &d.relations, &d.items, self.graph.show_items, self.graph.layout()));
        self.graph.cache = Some((key, g.clone()));
        g
    }

    fn finish_link(&mut self, at: Point<Pixels>, g: &Graph, cx: &mut Context<Self>) {
        let Some(l) = self.graph.link.take() else { return };
        let NodeRef::Entity(from) = g.nodes[l.from].r else { return };
        if !l.moved {
            return self.open_entity(from, cx);
        }
        let (x, y) = self.graph.local(at);
        let (wx, wy) = self.graph.camera.get().to_world(x, y);
        let Some(NodeRef::Entity(to)) = g.hit(wx, wy, ENT_W, ENT_H).map(|k| g.nodes[k].r) else {
            cx.notify();
            return;
        };
        if to == from {
            return cx.notify();
        }
        let d = self.data.read(cx);
        let (Some(a), Some(b)) = (d.entity(from).cloned(), d.entity(to).cloned()) else { return };
        let role = crate::entities::default_role(a.kind, b.kind);
        match self.data.update(cx, |d, cx| d.relate(from, to, role, cx)) {
            Ok(()) => self.toast(i18n::t!("app.graph.linked", from = a.name, to = b.name, role = role.label()), cx),
            Err(e) => self.toast(e, cx),
        }
    }

    pub fn render_graph(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let t = tokens(cx);
        let g = self.graph_data(cx);
        let today = Data::today();
        let d = self.data.read(cx);
        if d.entities.is_empty() {
            return div().flex().flex_col().items_center().justify_center().gap(px(10.)).size_full().child(ui::svg_icon(ui::GRAPH_SVG, 34., t.ink_muted)).child(div().text_size(px(15.)).font_weight(FontWeight::SEMIBOLD).child(i18n::t!("app.graph.empty_title"))).child(div().max_w(px(460.)).text_center().text_size(px(13.)).text_color(t.ink_muted).child(i18n::t!("app.graph.empty_body"))).into_any_element();
        }
        let cam = self.graph.camera.get();
        let z = cam.zoom;
        let screen = |x: f32, y: f32| cam.to_screen(x, y);

        // edges, in canvas-local pixels
        let mut rel_lines: Vec<((f32, f32), (f32, f32))> = Vec::new();
        let mut item_lines: Vec<((f32, f32), (f32, f32))> = Vec::new();
        let mut labels: Vec<AnyElement> = Vec::new();
        for (k, e) in g.edges.iter().enumerate() {
            let (a, b) = (&g.nodes[e.from], &g.nodes[e.to]);
            let (pa, pb) = (screen(a.x, a.y), screen(b.x, b.y));
            match e.role {
                Some(role) => {
                    rel_lines.push((pa, pb));
                    if z >= 0.55 {
                        let (mx, my) = ((pa.0 + pb.0) / 2., (pa.1 + pb.1) / 2.);
                        labels.push(div().id(("g-role", k)).absolute().left(px(mx - 40.)).top(px(my - 9.)).w(px(80.)).flex().justify_center().child(div().px(px(6.)).rounded(px(6.)).bg(t.canvas).border_1().border_color(t.border).text_size(px(10.5 * z.min(1.2))).text_color(t.ink_muted).child(role.label())).into_any_element());
                    }
                }
                None => item_lines.push((pa, pb)),
            }
        }
        let link_line = self.graph.link.filter(|l| l.moved).map(|l| {
            let n = &g.nodes[l.from];
            (screen(n.x, n.y), self.graph.local(l.cur))
        });

        // nodes
        let mut nodes: Vec<AnyElement> = Vec::new();
        for (k, n) in g.nodes.iter().enumerate() {
            let (w, h) = node_size(n.r);
            let (sx, sy) = screen(n.x, n.y);
            let (w, h) = (w * z, h * z);
            let hover = t.border_strong;
            match n.r {
                NodeRef::Entity(id) => {
                    let Some(e) = d.entity(id) else { continue };
                    let ring = vigia_core::view::worst_status(&d.items, id, today).map(|s| t.status(s)).unwrap_or(t.border);
                    let count = d.items.iter().filter(|i| i.entity == Some(id) && i.state != vigia_core::model::State::Archived).count();
                    let center = self.graph.center == Some(id) && self.graph.radial;
                    let gg = g.clone();
                    nodes.push(
                        div()
                            .id(("g-ent", k))
                            .absolute()
                            .left(px(sx - w / 2.))
                            .top(px(sy - h / 2.))
                            .w(px(w))
                            .h(px(h))
                            .flex()
                            .items_center()
                            .gap(px(8. * z))
                            .px(px(8. * z))
                            .rounded(px(h / 2.))
                            .bg(t.surface)
                            .border(px(2.))
                            .border_color(ring)
                            .when(center, |d| d.shadow_md())
                            .cursor_pointer()
                            .hover(move |s| s.shadow_md())
                            .child(div().flex().flex_none().items_center().justify_center().size(px(28. * z)).rounded_full().bg(ring.opacity(0.16)).child(ui::entity_icon(e.kind, 15. * z, t.ink)))
                            .child(div().flex().flex_col().flex_1().min_w_0().child(div().truncate().text_size(px(12.5 * z)).font_weight(FontWeight::SEMIBOLD).child(e.name.clone())).child(div().truncate().text_size(px(10.5 * z)).text_color(t.ink_muted).child(e.kind.label())))
                            .when(!self.graph.show_items && count > 0, |d| d.child(ui::chip(count.to_string(), t.ink_muted, 10.5 * z)))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                                    this.graph.link = Some(Link { from: k, start: ev.position, cur: ev.position, moved: false });
                                    cx.stop_propagation();
                                }),
                            )
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(move |this, ev: &MouseUpEvent, _, cx| {
                                    this.finish_link(ev.position, &gg, cx);
                                    cx.stop_propagation();
                                }),
                            )
                            .into_any_element(),
                    );
                }
                NodeRef::Item(iid) => {
                    let Some(it) = d.item(iid) else { continue };
                    let color = t.status(it.status(today));
                    let sel = self.selected == Some(iid);
                    nodes.push(
                        div()
                            .id(("g-item", k))
                            .absolute()
                            .left(px(sx - w / 2.))
                            .top(px(sy - h / 2.))
                            .w(px(w))
                            .h(px(h))
                            .flex()
                            .items_center()
                            .gap(px(6. * z))
                            .px(px(8. * z))
                            .rounded(px(8. * z))
                            .bg(t.surface)
                            .border_1()
                            .border_color(if sel { t.accent } else { t.border })
                            .cursor_pointer()
                            .hover(move |s| s.border_color(hover))
                            .child(div().flex_none().size(px(8. * z)).rounded_full().bg(color))
                            .child(div().flex_1().min_w_0().truncate().text_size(px(11.5 * z)).child(it.name.clone()))
                            .child(div().flex_none().text_size(px(10.5 * z)).text_color(color).child(crate::views::days_text(it.days_left(today))))
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .on_click(cx.listener(move |this, _, _, cx| this.select(iid, cx)))
                            .into_any_element(),
                    );
                }
            }
        }

        let bounds_cell = self.graph.bounds.clone();
        let need_fit = self.graph.need_fit.clone();
        let cam_cell = self.graph.camera.clone();
        let gfit = g.clone();
        let weak = cx.entity().downgrade();
        let tracking = self.graph.pan.is_some() || self.graph.link.is_some();
        let gup = g.clone();
        let (rel_color, item_color, link_color) = (t.accent.opacity(0.55), t.border_strong, t.accent);
        let bg = canvas(
            move |b, _, _| bounds_cell.set(b),
            move |b, _, window, _| {
                if need_fit.get() && b.size.width > px(10.) {
                    need_fit.set(false);
                    cam_cell.set(Camera::fit(&gfit, (b.size.width.into(), b.size.height.into()), 40.));
                    window.refresh();
                }
                let o = b.origin;
                let pt = |p: (f32, f32)| o + point(px(p.0), px(p.1));
                for (lines, color, width, dashed) in [(&item_lines, item_color, 1., true), (&rel_lines, rel_color, 1.8, false)] {
                    if lines.is_empty() {
                        continue;
                    }
                    let mut pb = PathBuilder::stroke(px(width));
                    if dashed {
                        pb = pb.dash_array(&[px(4.), px(4.)]);
                    }
                    for (a, c) in lines.iter() {
                        pb.move_to(pt(*a));
                        pb.line_to(pt(*c));
                    }
                    if let Ok(p) = pb.build() {
                        window.paint_path(p, color);
                    }
                }
                if let Some((a, c)) = link_line {
                    let mut pb = PathBuilder::stroke(px(2.)).dash_array(&[px(6.), px(4.)]);
                    pb.move_to(pt(a));
                    pb.line_to(pt(c));
                    if let Ok(p) = pb.build() {
                        window.paint_path(p, link_color);
                    }
                }
                if tracking {
                    let w = weak.clone();
                    window.on_mouse_event(move |e: &MouseMoveEvent, phase, _, cx| {
                        if phase != DispatchPhase::Bubble {
                            return;
                        }
                        w.update(cx, |this, cx| {
                            if e.pressed_button != Some(MouseButton::Left) {
                                this.graph.pan = None;
                                this.graph.link = None;
                            } else if let Some(p) = this.graph.pan {
                                let (dx, dy): (f32, f32) = ((e.position.x - p.start.x).into(), (e.position.y - p.start.y).into());
                                this.graph.camera.set(Camera { pan: (p.camera.pan.0 + dx, p.camera.pan.1 + dy), ..p.camera });
                            } else if let Some(l) = this.graph.link.as_mut() {
                                l.cur = e.position;
                                let (dx, dy): (f32, f32) = ((e.position.x - l.start.x).into(), (e.position.y - l.start.y).into());
                                l.moved |= dx.hypot(dy) > 5.;
                            }
                            cx.notify();
                        })
                        .ok();
                    });
                    let w = weak.clone();
                    let gup = gup.clone();
                    window.on_mouse_event(move |e: &MouseUpEvent, phase, _, cx| {
                        if phase == DispatchPhase::Bubble {
                            w.update(cx, |this, cx| {
                                this.graph.pan = None;
                                if this.graph.link.is_some() {
                                    this.finish_link(e.position, &gup, cx);
                                }
                                cx.notify();
                            })
                            .ok();
                        }
                    });
                }
            },
        )
        .absolute()
        .inset_0();

        div()
            .id("graph-canvas")
            .relative()
            .size_full()
            .overflow_hidden()
            .bg(t.canvas)
            .when(self.graph.pan.is_some(), |d| d.cursor(CursorStyle::ClosedHand))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, e: &MouseDownEvent, _, cx| {
                    this.graph.pan = Some(Pan { start: e.position, camera: this.graph.camera.get() });
                    cx.notify();
                }),
            )
            .on_scroll_wheel(cx.listener(|this, e: &ScrollWheelEvent, _, cx| {
                let d = e.delta.pixel_delta(px(20.));
                let (dx, dy): (f32, f32) = (d.x.into(), d.y.into());
                if e.modifiers.platform || e.modifiers.control {
                    let a = this.graph.local(e.position);
                    this.graph.zoom_by((dy * 0.01).exp(), Some(a));
                } else {
                    let mut c = this.graph.camera.get();
                    c.pan = (c.pan.0 + dx, c.pan.1 + dy);
                    this.graph.camera.set(c);
                }
                cx.notify();
            }))
            .on_pinch(cx.listener(|this, e: &PinchEvent, _, cx| {
                let a = this.graph.local(e.position);
                this.graph.zoom_by(1. + e.delta, Some(a));
                cx.notify();
            }))
            .child(bg)
            .children(labels)
            .children(nodes)
            .child(self.render_graph_toolbar(&t, cx))
            .into_any_element()
    }

    fn render_graph_toolbar(&self, t: &Tokens, cx: &mut Context<Self>) -> impl IntoElement {
        let d = self.data.read(cx);
        let radial = self.graph.radial;
        let center_name = self.graph.center.and_then(|c| d.entity(c)).map(|e| e.name.clone()).unwrap_or_else(|| i18n::t!("app.graph.pick_center"));
        let ents: Vec<(EntityId, String)> = d.entities.iter().map(|e| (e.id, e.name.clone())).collect();
        let cur = self.graph.center;
        let weak = cx.entity().downgrade();
        let center_btn = Button::new("g-center").outline().small().label(center_name).dropdown_menu(move |mut menu, _, _| {
            for (id, name) in &ents {
                let (w, id) = (weak.clone(), *id);
                menu = menu.item(PopupMenuItem::new(name.clone()).checked(cur == Some(id)).on_click(move |_, _, cx| {
                    w.update(cx, |this, cx| {
                        this.graph.center = Some(id);
                        this.graph.invalidate();
                        cx.notify();
                    })
                    .ok();
                }));
            }
            menu
        });
        let seg = ui::seg_group(t)
            .child(ui::seg(t, "g-layered", i18n::t!("app.graph.layered"), !radial).on_click(cx.listener(|this, _, _, cx| {
                this.graph.radial = false;
                this.graph.cache = None;
                this.graph.need_fit.set(true);
                cx.notify();
            })))
            .child(ui::seg(t, "g-radial", i18n::t!("app.graph.radial"), radial).on_click(cx.listener(|this, _, _, cx| {
                this.graph.radial = true;
                if this.graph.center.is_none() {
                    this.graph.center = this.data.read(cx).entities.first().map(|e| e.id);
                }
                this.graph.invalidate();
                cx.notify();
            })));
        div()
            .absolute()
            .top(px(14.))
            .left(px(14.))
            .right(px(14.))
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(8.))
            .child(div().px(px(10.)).py(px(4.)).rounded(px(8.)).bg(t.surface).border_1().border_color(t.border).text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).child(i18n::t!("app.view.graph")))
            .child(seg)
            .when(radial, |d| d.child(center_btn))
            .child(div().px(px(8.)).py(px(3.)).rounded(px(8.)).bg(t.surface).child(Checkbox::new("g-items").small().label(i18n::t!("app.graph.show_items")).checked(self.graph.show_items).on_click(cx.listener(|this, v: &bool, _, cx| {
                this.graph.show_items = *v;
                this.graph.invalidate();
                cx.notify();
            }))))
            .child(div().flex_1())
            .child(div().text_size(px(11.5)).text_color(t.ink_muted).child(i18n::t!("app.graph.hint")))
            .child(Button::new("g-zoom-out").outline().small().icon(IconName::Minus).on_click(cx.listener(|this, _, _, cx| {
                this.graph.zoom_by(1. / 1.2, None);
                cx.notify();
            })))
            .child(Button::new("g-zoom-in").outline().small().icon(IconName::Plus).on_click(cx.listener(|this, _, _, cx| {
                this.graph.zoom_by(1.2, None);
                cx.notify();
            })))
            .child(Button::new("g-fit").outline().small().icon(IconName::Maximize).label(i18n::t!("app.graph.fit")).on_click(cx.listener(|this, _, _, cx| {
                this.graph.need_fit.set(true);
                cx.notify();
            })))
    }
}
