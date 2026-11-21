//! The map of who owns what: entities (and optionally their items) as nodes, relations and
//! item → entity links as edges, laid out without UI. Layered (people → companies → assets →
//! items) or radial around one entity. Deterministic: the same data gives the same picture
//! whatever order it was loaded in. Plus the pan / zoom math the canvas uses.
use crate::entity::{Entity, EntityId, EntityKind, Relation, Role};
use crate::model::{Item, ItemId, State};
use std::collections::{HashMap, VecDeque};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum NodeRef {
    Entity(EntityId),
    Item(ItemId),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub r: NodeRef,
    /// Centre, in world units.
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Edge {
    pub from: usize,
    pub to: usize,
    /// `None` for an item → entity link.
    pub role: Option<Role>,
    pub relation: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Graph {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Layout {
    #[default]
    Layered,
    Radial(EntityId),
}

pub const COL_GAP: f32 = 280.;
pub const ROW_GAP: f32 = 84.;
pub const RING_GAP: f32 = 200.;

fn layer(k: EntityKind) -> usize {
    match k {
        EntityKind::Person => 0,
        EntityKind::Company => 1,
        _ => 2,
    }
}

/// Builds the graph of `entities`, `relations` and (when `show_items`) the items that belong to
/// an entity and aren't archived.
pub fn build(entities: &[Entity], relations: &[Relation], items: &[Item], show_items: bool, layout: Layout) -> Graph {
    let mut ents: Vec<&Entity> = entities.iter().collect();
    ents.sort_by_key(|e| (layer(e.kind), e.kind, e.name.to_lowercase(), e.id));
    let mut its: Vec<&Item> = if show_items { items.iter().filter(|i| i.state != State::Archived && i.entity.is_some_and(|e| entities.iter().any(|x| x.id == e))).collect() } else { Vec::new() };
    its.sort_by_key(|i| (i.entity, i.name.to_lowercase(), i.id));
    let mut nodes: Vec<Node> = ents.iter().map(|e| Node { r: NodeRef::Entity(e.id), x: 0., y: 0. }).chain(its.iter().map(|i| Node { r: NodeRef::Item(i.id), x: 0., y: 0. })).collect();
    let index: HashMap<NodeRef, usize> = nodes.iter().enumerate().map(|(k, n)| (n.r, k)).collect();
    let mut rels: Vec<&Relation> = relations.iter().collect();
    rels.sort_by_key(|r| (r.from, r.to, r.id));
    let mut edges: Vec<Edge> = rels
        .iter()
        .filter_map(|r| Some(Edge { from: *index.get(&NodeRef::Entity(r.from))?, to: *index.get(&NodeRef::Entity(r.to))?, role: Some(r.role), relation: Some(r.id) }))
        .collect();
    for i in &its {
        if let (Some(&a), Some(&b)) = (index.get(&NodeRef::Item(i.id)), i.entity.and_then(|e| index.get(&NodeRef::Entity(e)))) {
            edges.push(Edge { from: b, to: a, role: None, relation: None });
        }
    }
    let kinds: Vec<Option<EntityKind>> = nodes.iter().map(|n| if let NodeRef::Entity(id) = n.r { ents.iter().find(|e| e.id == id).map(|e| e.kind) } else { None }).collect();
    let pos = match layout {
        Layout::Layered => layered(&kinds, &edges),
        Layout::Radial(center) => match index.get(&NodeRef::Entity(center)) {
            Some(&c) => radial(nodes.len(), &edges, c),
            None => layered(&kinds, &edges),
        },
    };
    for (n, (x, y)) in nodes.iter_mut().zip(pos) {
        n.x = x;
        n.y = y;
    }
    Graph { nodes, edges }
}

fn neighbours(n: usize, edges: &[Edge]) -> Vec<Vec<usize>> {
    let mut adj = vec![Vec::new(); n];
    for e in edges {
        adj[e.from].push(e.to);
        adj[e.to].push(e.from);
    }
    for a in &mut adj {
        a.sort_unstable();
        a.dedup();
    }
    adj
}

/// Columns: people, companies, the other kinds, items. Each column is ordered by the mean row
/// of its neighbours in the columns before it (one sweep), so links run mostly straight.
fn layered(kinds: &[Option<EntityKind>], edges: &[Edge]) -> Vec<(f32, f32)> {
    let n = kinds.len();
    let adj = neighbours(n, edges);
    let col = |k: usize| kinds[k].map_or(3, layer);
    let mut row = vec![0f32; n];
    let mut out = vec![(0., 0.); n];
    for c in 0..4 {
        let mut members: Vec<usize> = (0..n).filter(|&k| col(k) == c).collect();
        if c > 0 {
            let key = |k: usize| {
                let before: Vec<f32> = adj[k].iter().filter(|&&o| col(o) < c).map(|&o| row[o]).collect();
                if before.is_empty() { f32::MAX } else { before.iter().sum::<f32>() / before.len() as f32 }
            };
            // stable: ties keep the name order
            members.sort_by(|&a, &b| key(a).total_cmp(&key(b)));
        }
        let h = (members.len().max(1) - 1) as f32 * ROW_GAP;
        for (r, &k) in members.iter().enumerate() {
            row[k] = r as f32;
            out[k] = (c as f32 * COL_GAP, r as f32 * ROW_GAP - h / 2.);
        }
    }
    out
}

/// Rings by distance from `center`; each ring follows the angles of the nodes that reached it,
/// unconnected nodes on the outer ring.
fn radial(n: usize, edges: &[Edge], center: usize) -> Vec<(f32, f32)> {
    let adj = neighbours(n, edges);
    let mut depth = vec![usize::MAX; n];
    let mut parent = vec![usize::MAX; n];
    depth[center] = 0;
    let mut q = VecDeque::from([center]);
    while let Some(k) = q.pop_front() {
        for &o in &adj[k] {
            if depth[o] == usize::MAX {
                depth[o] = depth[k] + 1;
                parent[o] = k;
                q.push_back(o);
            }
        }
    }
    let max = depth.iter().filter(|d| **d != usize::MAX).max().copied().unwrap_or(0);
    for d in depth.iter_mut().filter(|d| **d == usize::MAX) {
        *d = max + 1;
    }
    let mut angle = vec![0f32; n];
    let mut out = vec![(0., 0.); n];
    for ring in 1..=max + 1 {
        let mut members: Vec<usize> = (0..n).filter(|&k| depth[k] == ring).collect();
        members.sort_by(|&a, &b| {
            let pa = if parent[a] == usize::MAX { f32::MAX } else { angle[parent[a]] };
            let pb = if parent[b] == usize::MAX { f32::MAX } else { angle[parent[b]] };
            pa.total_cmp(&pb).then(a.cmp(&b))
        });
        let m = members.len() as f32;
        let radius = ring as f32 * RING_GAP;
        for (j, &k) in members.iter().enumerate() {
            let a = -std::f32::consts::FRAC_PI_2 + std::f32::consts::TAU * j as f32 / m;
            angle[k] = a;
            out[k] = (radius * a.cos(), radius * a.sin());
        }
    }
    out
}

impl Graph {
    pub fn index_of(&self, r: NodeRef) -> Option<usize> {
        self.nodes.iter().position(|n| n.r == r)
    }

    /// min x, min y, max x, max y of the node centres.
    pub fn bounds(&self) -> (f32, f32, f32, f32) {
        if self.nodes.is_empty() {
            return (0., 0., 0., 0.);
        }
        self.nodes.iter().fold((f32::MAX, f32::MAX, f32::MIN, f32::MIN), |(a, b, c, d), n| (a.min(n.x), b.min(n.y), c.max(n.x), d.max(n.y)))
    }

    /// The node whose box (`w` × `h` around its centre) holds the world point.
    pub fn hit(&self, x: f32, y: f32, w: f32, h: f32) -> Option<usize> {
        self.nodes.iter().rposition(|n| (x - n.x).abs() <= w / 2. && (y - n.y).abs() <= h / 2.)
    }
}

pub const ZOOM_MIN: f32 = 0.25;
pub const ZOOM_MAX: f32 = 2.5;

/// Pan / zoom: screen = pan + world × zoom (screen relative to the canvas origin).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    pub pan: (f32, f32),
    pub zoom: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Camera { pan: (0., 0.), zoom: 1. }
    }
}

impl Camera {
    pub fn to_screen(self, x: f32, y: f32) -> (f32, f32) {
        (self.pan.0 + x * self.zoom, self.pan.1 + y * self.zoom)
    }

    pub fn to_world(self, x: f32, y: f32) -> (f32, f32) {
        ((x - self.pan.0) / self.zoom, (y - self.pan.1) / self.zoom)
    }

    /// Zooms keeping the world point under the screen `anchor` fixed.
    pub fn zoom_around(&mut self, anchor: (f32, f32), zoom: f32) {
        let w = self.to_world(anchor.0, anchor.1);
        self.zoom = zoom.clamp(ZOOM_MIN, ZOOM_MAX);
        self.pan = (anchor.0 - w.0 * self.zoom, anchor.1 - w.1 * self.zoom);
    }

    /// The whole graph in a viewport of `size`, `margin` px around, never past 100 %.
    pub fn fit(g: &Graph, size: (f32, f32), margin: f32) -> Camera {
        let (x0, y0, x1, y1) = g.bounds();
        let (bw, bh) = (x1 - x0 + 220., y1 - y0 + 80.);
        let zoom = ((size.0 - 2. * margin).max(1.) / bw).min((size.1 - 2. * margin).max(1.) / bh).clamp(ZOOM_MIN, 1.);
        let (cx, cy) = ((x0 + x1) / 2., (y0 + y1) / 2.);
        Camera { pan: (size.0 / 2. - cx * zoom, size.1 / 2. - cy * zoom), zoom }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Kind;

    fn data() -> (Vec<Entity>, Vec<Relation>, Vec<Item>) {
        let e = |id, kind, name: &str| Entity { id, kind, name: name.into(), ..Default::default() };
        let ents = vec![e(1, EntityKind::Person, "Juan"), e(2, EntityKind::Person, "Ana"), e(3, EntityKind::Company, "Otherwise SAS"), e(4, EntityKind::Company, "Otherwise Technologies LLC"), e(5, EntityKind::Vehicle, "Mazda ABC123"), e(6, EntityKind::Digital, "Workspace")];
        let r = |id, from, to, role| Relation { id, from, to, role };
        let rels = vec![r(1, 1, 3, Role::Owner), r(2, 1, 4, Role::Member), r(3, 1, 5, Role::Owner), r(4, 3, 6, Role::Owner)];
        let it = |id, ent, kind| Item { id, entity: Some(ent), ..Item::new(kind, format!("item {id}")) };
        let items = vec![it(10, 2, Kind::Passport), it(11, 5, Kind::Soat), it(12, 3, Kind::Domain), it(13, 3, Kind::Certificate), Item { state: State::Archived, ..it(14, 3, Kind::Rut) }, Item { id: 15, ..Item::new(Kind::Custom, "loose") }];
        (ents, rels, items)
    }

    #[test]
    fn layered_columns_and_edges() {
        let (e, r, i) = data();
        let g = build(&e, &r, &i, true, Layout::Layered);
        // 6 entities + 4 items (archived and ownerless left out)
        assert_eq!(g.nodes.len(), 10);
        assert_eq!(g.edges.len(), 4 + 4);
        let at = |r: NodeRef| &g.nodes[g.index_of(r).unwrap()];
        assert_eq!(at(NodeRef::Entity(1)).x, 0.);
        assert_eq!(at(NodeRef::Entity(3)).x, COL_GAP);
        assert_eq!(at(NodeRef::Entity(5)).x, 2. * COL_GAP);
        assert_eq!(at(NodeRef::Item(12)).x, 3. * COL_GAP);
        // columns are centred on y = 0
        let people: Vec<f32> = g.nodes.iter().filter(|n| n.x == 0.).map(|n| n.y).collect();
        assert_eq!(people.iter().sum::<f32>(), 0.);
        assert_eq!(g.edges.iter().filter(|e| e.role.is_none()).count(), 4);
        let without = build(&e, &r, &i, false, Layout::Layered);
        assert_eq!((without.nodes.len(), without.edges.len()), (6, 4));
    }

    #[test]
    fn deterministic_whatever_the_order() {
        let (mut e, mut r, mut i) = data();
        for layout in [Layout::Layered, Layout::Radial(1), Layout::Radial(3)] {
            let a = build(&e, &r, &i, true, layout);
            e.reverse();
            r.reverse();
            i.reverse();
            let b = build(&e, &r, &i, true, layout);
            assert_eq!(a, b, "{layout:?}");
            assert_eq!(a, build(&e, &r, &i, true, layout));
        }
    }

    #[test]
    fn radial_rings() {
        let (e, r, i) = data();
        let g = build(&e, &r, &i, false, Layout::Radial(1));
        let c = &g.nodes[g.index_of(NodeRef::Entity(1)).unwrap()];
        assert_eq!((c.x, c.y), (0., 0.));
        let dist = |id| {
            let n = &g.nodes[g.index_of(NodeRef::Entity(id)).unwrap()];
            (n.x.hypot(n.y) / RING_GAP).round() as usize
        };
        assert_eq!([dist(3), dist(4), dist(5)], [1, 1, 1]);
        assert_eq!(dist(6), 2);
        // Ana isn't linked to Juan: outer ring
        assert_eq!(dist(2), 3);
        // an unknown centre falls back to the layered layout
        assert_eq!(build(&e, &r, &i, false, Layout::Radial(99)), build(&e, &r, &i, false, Layout::Layered));
    }

    #[test]
    fn camera_math() {
        let mut c = Camera::default();
        c.zoom_around((100., 50.), 2.);
        assert_eq!(c.to_screen(100., 50.), (100., 50.));
        assert_eq!(c.to_world(102., 52.), (101., 51.));
        c.zoom_around((0., 0.), 100.);
        assert_eq!(c.zoom, ZOOM_MAX);
        let (e, r, i) = data();
        let g = build(&e, &r, &i, true, Layout::Layered);
        let f = Camera::fit(&g, (1200., 800.), 40.);
        assert!(f.zoom <= 1. && f.zoom >= ZOOM_MIN);
        let (x0, y0, x1, y1) = g.bounds();
        let (sx, sy) = f.to_screen((x0 + x1) / 2., (y0 + y1) / 2.);
        assert!((sx - 600.).abs() < 0.01 && (sy - 400.).abs() < 0.01);
        assert_eq!(g.hit(g.nodes[0].x + 5., g.nodes[0].y, 40., 40.), Some(0));
        assert_eq!(g.hit(10_000., 0., 40., 40.), None);
    }
}
