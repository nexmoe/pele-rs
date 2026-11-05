//! The sankey layout: the columns the nodes stand in, how thick the bands are, and where
//! everything sits. Ported from pele's `diagrams/sankey/layout.ts`.

use std::cmp::Ordering;

use super::db::SankeyDb;
use crate::DiagramKind;
use crate::error::{Error, ErrorCode};

/// How the nodes are placed in the columns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Alignment {
    Justify,
    Left,
    Right,
    Center,
}

/// What the layout needs to know about the space the chart has.
pub(crate) struct LayoutOptions {
    pub width: f64,
    pub height: f64,
    pub node_width: f64,
    pub node_padding: f64,
    /// The height of a label line; thin nodes are kept at least this far apart where there is
    /// room.
    pub label_height: f64,
    pub alignment: Alignment,
}

/// A node, placed.
#[derive(Debug, Clone)]
pub(crate) struct PlacedNode {
    /// The column the node stands in.
    pub layer: usize,
    /// The total flow through the node, in the diagram's units.
    pub value: f64,
    pub x: f64,
    pub y: f64,
    pub height: f64,
}

/// A band of flow, placed.
#[derive(Debug, Clone)]
pub(crate) struct PlacedLink {
    pub source: usize,
    pub target: usize,
    pub value: f64,
    pub thickness: f64,
    /// The vertical middle of the band where it leaves its source and where it enters its
    /// target.
    pub source_y: f64,
    pub target_y: f64,
}

#[derive(Debug)]
pub(crate) struct SankeyLayout {
    pub nodes: Vec<PlacedNode>,
    pub links: Vec<PlacedLink>,
}

/// How many times the columns settle next to their neighbours.
const PASSES: usize = 6;

/// The state the layout builds up.
struct Builder {
    nodes: Vec<PlacedNode>,
    links: Vec<PlacedLink>,
    outgoing: Vec<Vec<usize>>,
    incoming: Vec<Vec<usize>>,
    /// The offset of each band's middle from the top of the node at either end.
    source_offset: Vec<f64>,
    target_offset: Vec<f64>,
    /// The room a node keeps clear above and below itself.
    margin: Vec<f64>,
    gap: f64,
    height: f64,
}

impl Builder {
    fn center(&self, i: usize) -> f64 {
        self.nodes[i].y + self.nodes[i].height / 2.0
    }

    /// Orders the bands leaving a node by where they go, and where they sit.
    fn stack_out(&mut self, i: usize) {
        let mut list = self.outgoing[i].clone();
        list.sort_by(|&a, &b| {
            let d = self.center(self.links[a].target) - self.center(self.links[b].target);
            if d < 0.0 {
                Ordering::Less
            } else if d > 0.0 {
                Ordering::Greater
            } else {
                a.cmp(&b)
            }
        });
        let mut y = 0.0;
        for &k in &list {
            self.source_offset[k] = y + self.links[k].thickness / 2.0;
            y += self.links[k].thickness;
        }
        self.outgoing[i] = list;
    }

    /// Orders the bands entering a node by where they come from, and where they sit.
    fn stack_in(&mut self, i: usize) {
        let mut list = self.incoming[i].clone();
        list.sort_by(|&a, &b| {
            let d = self.center(self.links[a].source) - self.center(self.links[b].source);
            if d < 0.0 {
                Ordering::Less
            } else if d > 0.0 {
                Ordering::Greater
            } else {
                a.cmp(&b)
            }
        });
        let mut y = 0.0;
        for &k in &list {
            self.target_offset[k] = y + self.links[k].thickness / 2.0;
            y += self.links[k].thickness;
        }
        self.incoming[i] = list;
    }

    fn stack_all(&mut self) {
        for i in 0..self.nodes.len() {
            self.stack_out(i);
            self.stack_in(i);
        }
    }

    /// Moves each node of a column towards where its bands would run level, then restores the
    /// order and the gaps. `forward` looks at the bands coming in, otherwise at those going
    /// out.
    fn settle(&mut self, column: &[usize], pull: f64, forward: bool) {
        for &i in column {
            let empty = if forward {
                self.incoming[i].is_empty()
            } else {
                self.outgoing[i].is_empty()
            };
            if empty {
                continue;
            }
            if forward {
                self.stack_in(i);
            } else {
                self.stack_out(i);
            }
            // The bands were reordered, so the list is read again.
            let list = if forward {
                self.incoming[i].clone()
            } else {
                self.outgoing[i].clone()
            };
            let mut weighted = 0.0;
            let mut weight = 0.0;
            let mut plain = 0.0;
            for &k in &list {
                let level = if forward {
                    self.nodes[self.links[k].source].y + self.source_offset[k]
                        - self.target_offset[k]
                } else {
                    self.nodes[self.links[k].target].y + self.target_offset[k]
                        - self.source_offset[k]
                };
                weighted += level * self.links[k].thickness;
                weight += self.links[k].thickness;
                let other = if forward {
                    self.links[k].source
                } else {
                    self.links[k].target
                };
                plain += self.center(other) - self.nodes[i].height / 2.0;
            }
            let wanted = if weight > 0.0 {
                weighted / weight
            } else {
                plain / list.len() as f64
            };
            self.nodes[i].y += (wanted - self.nodes[i].y) * pull;
        }

        let mut column = column.to_vec();
        column.sort_by(|&a, &b| {
            let d = self.center(a) - self.center(b);
            if d < 0.0 {
                Ordering::Less
            } else if d > 0.0 {
                Ordering::Greater
            } else {
                a.cmp(&b)
            }
        });
        let mut y = 0.0;
        for &i in &column {
            if self.nodes[i].y - self.margin[i] < y {
                self.nodes[i].y = y + self.margin[i];
            }
            y = self.nodes[i].y + self.nodes[i].height + self.margin[i] + self.gap;
        }
        y = self.height;
        for &i in column.iter().rev() {
            if self.nodes[i].y + self.nodes[i].height + self.margin[i] > y {
                self.nodes[i].y = y - self.nodes[i].height - self.margin[i];
            }
            y = self.nodes[i].y - self.margin[i] - self.gap;
        }
    }
}

/// Places the nodes in columns by their distance from a source, sizes them by their flow, and
/// then lets each column settle next to its neighbours so that the bands cross less.
pub(crate) fn layout_sankey(
    model: &SankeyDb,
    options: &LayoutOptions,
) -> Result<SankeyLayout, Error> {
    let n = model.nodes.len();
    let mut links: Vec<PlacedLink> = model
        .links
        .iter()
        .map(|link| PlacedLink {
            source: link.source,
            target: link.target,
            value: if link.value.is_finite() && link.value > 0.0 {
                link.value
            } else {
                0.0
            },
            thickness: 0.0,
            source_y: 0.0,
            target_y: 0.0,
        })
        .collect();

    let mut outgoing: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut incoming: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (k, link) in links.iter().enumerate() {
        outgoing[link.source].push(k);
        incoming[link.target].push(k);
    }

    // Longest distance from a source, in topological order; what is left over lies on a cycle.
    let mut depth = vec![0i32; n];
    let mut pending: Vec<i32> = incoming.iter().map(|v| v.len() as i32).collect();
    let mut order: Vec<usize> = Vec::new();
    for (i, &waiting) in pending.iter().enumerate() {
        if waiting == 0 {
            order.push(i);
        }
    }
    let mut q = 0;
    while q < order.len() {
        let i = order[q];
        for &k in &outgoing[i] {
            let t = links[k].target;
            if depth[t] < depth[i] + 1 {
                depth[t] = depth[i] + 1;
            }
            pending[t] -= 1;
            if pending[t] == 0 {
                order.push(t);
            }
        }
        q += 1;
    }
    if order.len() < n {
        // Every node left over has a predecessor that is left over too; walking back finds the
        // loop.
        let mut stuck = 0;
        while pending[stuck] == 0 {
            stuck += 1;
        }
        let mut seen = vec![false; n];
        while !seen[stuck] {
            seen[stuck] = true;
            for &k in &incoming[stuck] {
                if pending[links[k].source] > 0 {
                    stuck = links[k].source;
                    break;
                }
            }
        }
        return Err(Error::new(
            ErrorCode::Semantic,
            format!(
                "Sankey diagrams cannot have circular links, and \"{}\" is part of one.",
                model.nodes[stuck].id
            ),
        )
        .with_kind(DiagramKind::Sankey));
    }
    let mut layers = 1usize;
    for &d in depth.iter() {
        if (d + 1) as usize > layers {
            layers = (d + 1) as usize;
        }
    }
    // Longest distance to a sink.
    let mut reach = vec![0i32; n];
    for q in (0..n).rev() {
        let i = order[q];
        for &k in &outgoing[i] {
            if reach[i] < reach[links[k].target] + 1 {
                reach[i] = reach[links[k].target] + 1;
            }
        }
    }

    // Flows are scaled to the largest one, so that sums of huge values stay finite.
    let mut largest = 0.0f64;
    for link in &links {
        if link.value > largest {
            largest = link.value;
        }
    }
    // Divide by the largest flow directly when its reciprocal would overflow. Keeping the
    // original arithmetic for normal values preserves existing layout fixtures.
    let unit = if largest > 0.0 { 1.0 / largest } else { 0.0 };
    let normalize = |value: f64| {
        if unit.is_finite() {
            value * unit
        } else {
            value / largest
        }
    };

    let mut nodes: Vec<PlacedNode> = Vec::with_capacity(n);
    let mut flow = vec![0.0f64; n];
    let mut columns: Vec<Vec<usize>> = vec![Vec::new(); layers];
    let step = if layers > 1 {
        (options.width - options.node_width) / (layers as f64 - 1.0)
    } else {
        0.0
    };

    for i in 0..n {
        let layer = match options.alignment {
            Alignment::Left => depth[i] as usize,
            Alignment::Right => (layers as i32 - 1 - reach[i]).max(0) as usize,
            Alignment::Center => {
                let mut layer = depth[i];
                if incoming[i].is_empty() && !outgoing[i].is_empty() {
                    layer = layers as i32;
                    for &k in &outgoing[i] {
                        if depth[links[k].target] - 1 < layer {
                            layer = depth[links[k].target] - 1;
                        }
                    }
                }
                layer.max(0) as usize
            }
            Alignment::Justify => {
                if !outgoing[i].is_empty() {
                    depth[i] as usize
                } else {
                    layers - 1
                }
            }
        };
        let mut into = 0.0;
        let mut out = 0.0;
        let mut value = 0.0;
        let mut value_out = 0.0;
        for &k in &incoming[i] {
            into += normalize(links[k].value);
            value += links[k].value;
        }
        for &k in &outgoing[i] {
            out += normalize(links[k].value);
            value_out += links[k].value;
        }
        flow[i] = into.max(out);
        nodes.push(PlacedNode {
            layer,
            value: value.max(value_out),
            x: layer as f64 * step,
            y: 0.0,
            height: 0.0,
        });
        columns[layer].push(i);
    }

    let mut tallest = 1usize;
    for column in &columns {
        if column.len() > tallest {
            tallest = column.len();
        }
    }
    let gap = if tallest > 1 {
        options
            .node_padding
            .min(options.height / (tallest as f64 - 1.0))
    } else {
        options.node_padding
    };
    let mut scale = f64::INFINITY;
    for column in &columns {
        let mut sum = 0.0;
        for &i in column {
            sum += flow[i];
        }
        if sum > 0.0 {
            scale = scale.min((options.height - (column.len() as f64 - 1.0) * gap) / sum);
        }
    }
    if !scale.is_finite() || scale < 0.0 {
        scale = 0.0;
    }

    // A thin node still needs room for its label. Where every column can afford it, each node
    // claims at least a label's height, and the flows are scaled down to make that fit.
    let mut claim = (options.label_height - gap).max(0.0);
    let fits = |s: f64, claim: f64| {
        columns.iter().all(|column| {
            let mut extent = (column.len() as f64 - 1.0) * gap;
            for &i in column {
                extent += (flow[i] * s).max(claim);
            }
            extent <= options.height + 1e-6
        })
    };
    if claim > 0.0 && !fits(scale, claim) {
        if fits(0.0, claim) {
            let mut low = 0.0;
            for _ in 0..40 {
                let mid = (low + scale) / 2.0;
                if fits(mid, claim) {
                    low = mid;
                } else {
                    scale = mid;
                }
            }
            scale = low;
        } else {
            claim = 0.0;
        }
    }

    let mut margin = vec![0.0f64; n];
    for link in &mut links {
        link.thickness = normalize(link.value) * scale;
    }
    for column in &columns {
        let mut y = 0.0;
        for &i in column {
            nodes[i].height = flow[i] * scale;
            margin[i] = (claim - nodes[i].height).max(0.0) / 2.0;
            nodes[i].y = y + margin[i];
            y += nodes[i].height + 2.0 * margin[i] + gap;
        }
        // Spare room is shared out between the nodes of a column.
        let spare = (options.height - y + gap) / (column.len() as f64 + 1.0);
        for (k, &i) in column.iter().enumerate() {
            nodes[i].y += spare * (k as f64 + 1.0);
        }
    }

    let mut builder = Builder {
        nodes,
        links,
        outgoing,
        incoming,
        source_offset: vec![0.0; model.links.len()],
        target_offset: vec![0.0; model.links.len()],
        margin,
        gap,
        height: options.height,
    };

    builder.stack_all();
    for pass in 0..PASSES {
        let pull = 0.99f64.powi(pass as i32);
        for l in (0..layers.saturating_sub(1)).rev() {
            builder.settle(&columns[l], pull, false);
        }
        builder.stack_all();
        for column in columns.iter().skip(1) {
            builder.settle(column, pull, true);
        }
        builder.stack_all();
    }

    for k in 0..builder.links.len() {
        let source = builder.links[k].source;
        let target = builder.links[k].target;
        builder.links[k].source_y = builder.nodes[source].y + builder.source_offset[k];
        builder.links[k].target_y = builder.nodes[target].y + builder.target_offset[k];
    }

    Ok(SankeyLayout {
        nodes: builder.nodes,
        links: builder.links,
    })
}
