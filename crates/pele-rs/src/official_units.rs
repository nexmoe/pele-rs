//! Pure utility and layout calls captured from the pinned official test suites.
use crate::{preprocess as pre, svg, text::Style};
use serde_json::{Value, json};
use std::io::BufRead;
fn number(v: &Value) -> f64 {
    v.as_f64().unwrap_or_else(|| match v["$number"].as_str() {
        Some("Infinity") => f64::INFINITY,
        Some("-Infinity") => f64::NEG_INFINITY,
        _ => f64::NAN,
    })
}
fn encoded(n: f64) -> Value {
    if n.is_finite() {
        json!(n)
    } else {
        json!({"$number":crate::util::js_number(n)})
    }
}
fn config(v: &Value) -> pre::Value {
    match v {
        Value::Null => pre::Value::Null,
        Value::Bool(b) => pre::Value::Bool(*b),
        Value::Number(_) => pre::Value::Number(number(v)),
        Value::String(s) => pre::Value::String(s.clone()),
        Value::Array(a) => pre::Value::Seq(a.iter().map(config).collect()),
        Value::Object(m) => {
            if m.len() == 1 && m.contains_key("$number") {
                pre::Value::Number(number(v))
            } else {
                pre::Value::Map(m.iter().map(|(k, v)| (k.clone(), config(v))).collect())
            }
        }
    }
}
fn value(v: &pre::Value) -> Value {
    match v {
        pre::Value::Null => Value::Null,
        pre::Value::Bool(b) => json!(b),
        pre::Value::Number(n) => encoded(*n),
        pre::Value::String(s) => json!(s),
        pre::Value::Seq(a) => json!(a.iter().map(value).collect::<Vec<_>>()),
        pre::Value::Map(m) => Value::Object(m.iter().map(|(k, v)| (k.clone(), value(v))).collect()),
    }
}
fn style(bits: u64) -> Style {
    [
        Style::BOLD,
        Style::ITALIC,
        Style::MONO,
        Style::SUB,
        Style::SUP,
        Style::UNDERLINE,
    ]
    .into_iter()
    .enumerate()
    .fold(
        Style::NONE,
        |s, (i, flag)| if bits & (1 << i) != 0 { s | flag } else { s },
    )
}
fn kind(name: &str) -> crate::DiagramKind {
    use crate::DiagramKind::*;
    match name {
        "pie" => Pie,
        "flowchart" => Flowchart,
        "sequence" => Sequence,
        "state" => State,
        "class" => Class,
        "er" => Er,
        "requirement" => Requirement,
        "gantt" => Gantt,
        "timeline" => Timeline,
        "journey" => Journey,
        "kanban" => Kanban,
        "mindmap" => Mindmap,
        "sankey" => Sankey,
        "xychart" => XyChart,
        "quadrantChart" => QuadrantChart,
        "gitGraph" => GitGraph,
        "block" => Block,
        "architecture" => Architecture,
        "packet" => Packet,
        "radar" => Radar,
        "treemap" => Treemap,
        "c4" => C4,
        "usecase" => Usecase,
        "info" => Info,
        "venn" => Venn,
        "treeView" => TreeView,
        "cynefin" => Cynefin,
        "ishikawa" => Ishikawa,
        "eventmodeling" => EventModeling,
        "wardley" => Wardley,
        "railroad" => Railroad,
        "swimlane" => Swimlane,
        "agentflow" => Agentflow,
        _ => panic!("unknown type: {name}"),
    }
}
fn options(v: &Value) -> crate::layout::layered::LayeredOptions {
    use crate::layout::layered::LayeredOptions;
    let n = |k: &str| v[k].as_f64().unwrap_or(0.0);
    let b = |k: &str| v[k].as_bool().unwrap_or(false);
    LayeredOptions {
        node_sep: n("nodeSep"),
        edge_sep: n("edgeSep"),
        rank_sep: n("rankSep"),
        port_sep: n("portSep"),
        head_room: n("headRoom"),
        tracks: b("tracks"),
        straight: b("straight"),
        apart: b("apart"),
        order_only: b("orderOnly"),
        groups: v["groups"].as_object().map(|_| {
            let a = |k: &str| {
                v["groups"][k]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|n| n.as_i64().unwrap() as i32)
                    .collect()
            };
            (a("of"), a("parent"))
        }),
    }
}
fn layout(entry: &Value) -> Value {
    use crate::layout::{compound as c, layered as l};
    let a = &entry["args"];
    let mut after = a.clone();
    let result = if entry["name"] == "layout/layered.ts:layered" {
        let mut nodes: Vec<_> = a[0]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| {
                let k = match v["kind"].as_u64().unwrap() {
                    0 => l::Kind::Node,
                    1 => l::Kind::Dummy,
                    2 => l::Kind::Label,
                    3 => l::Kind::StartPort,
                    4 => l::Kind::EndPort,
                    _ => panic!("unknown layout kind"),
                };
                let mut n = l::lnode(number(&v["w"]), number(&v["h"]), k);
                n.pin = v["pin"].as_i64().unwrap() as i32;
                n.hint = number(&v["hint"]);
                n.key = number(&v["key"]);
                n.span = number(&v["span"]);
                n.x = number(&v["x"]);
                n.y = number(&v["y"]);
                n.rank = v["rank"].as_i64().unwrap() as i32;
                n
            })
            .collect();
        let mut edges: Vec<_> = a[1]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| {
                let mut e = l::ledge(
                    v["tail"].as_u64().unwrap() as usize,
                    v["head"].as_u64().unwrap() as usize,
                    v["minlen"].as_i64().unwrap() as i32,
                    number(&v["labelW"]),
                    number(&v["labelH"]),
                );
                e.tail_dx = number(&v["tailDx"]);
                e.head_dx = number(&v["headDx"]);
                e.tail_fixed = v["tailFixed"].as_bool().unwrap();
                e.head_fixed = v["headFixed"].as_bool().unwrap();
                e.reversed = v["reversed"].as_bool().unwrap();
                e
            })
            .collect();
        let r = l::layered(&mut nodes, &mut edges, &options(&a[2]));
        for (v, n) in after[0].as_array_mut().unwrap().iter_mut().zip(nodes) {
            for (k, n) in [
                ("w", n.w),
                ("h", n.h),
                ("x", n.x),
                ("y", n.y),
                ("hint", n.hint),
                ("key", n.key),
                ("span", n.span),
            ] {
                v[k] = encoded(n);
            }
            v["rank"] = json!(n.rank);
            v["pin"] = json!(n.pin);
        }
        for (v, e) in after[1].as_array_mut().unwrap().iter_mut().zip(edges) {
            for (k, n) in [
                ("tailDx", e.tail_dx),
                ("headDx", e.head_dx),
                ("labelX", e.label_x),
                ("labelY", e.label_y),
            ] {
                v[k] = encoded(n);
            }
            v["points"] = json!(e.points);
            v["reversed"] = json!(e.reversed);
        }
        let mut out = json!({"width":r.width,"height":r.height});
        if entry["result"].get("crossings").is_some() {
            out["crossings"] = json!(r.crossings);
            out["declared"] = json!(r.declared);
        }
        out
    } else {
        let mut nodes: Vec<_> = a[0]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| {
                let mut n = c::cnode(number(&v["w"]), number(&v["h"]));
                n.parent = v["parent"].as_i64().unwrap();
                n.is_group = v["isGroup"].as_bool().unwrap();
                n.dir = v["dir"].as_str().map(|d| c::direction(Some(d)));
                n.pad_x = number(&v["padX"]);
                n.pad_top = number(&v["padTop"]);
                n.pad_bottom = number(&v["padBottom"]);
                n.min_w = number(&v["minW"]);
                n.seq = number(&v["seq"]);
                n.span = number(&v["span"]);
                n
            })
            .collect();
        let mut edges: Vec<_> = a[1]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| {
                c::cedge(
                    v["src"].as_u64().unwrap() as usize,
                    v["dst"].as_u64().unwrap() as usize,
                    v["minlen"].as_i64().unwrap() as i32,
                    number(&v["labelW"]),
                    number(&v["labelH"]),
                )
            })
            .collect();
        let r = c::compound_layout(
            &mut nodes,
            &mut edges,
            c::direction(a[2].as_str()),
            &options(&a[3]),
        );
        for (v, n) in after[0].as_array_mut().unwrap().iter_mut().zip(nodes) {
            for (k, n) in [
                ("w", n.w),
                ("h", n.h),
                ("x", n.x),
                ("y", n.y),
                ("padX", n.pad_x),
                ("padTop", n.pad_top),
                ("padBottom", n.pad_bottom),
                ("minW", n.min_w),
                ("seq", n.seq),
                ("span", n.span),
            ] {
                v[k] = encoded(n);
            }
            v["parent"] = json!(n.parent);
        }
        for (v, e) in after[1].as_array_mut().unwrap().iter_mut().zip(edges) {
            v["route"] = json!(e.route);
            v["labelX"] = encoded(e.label_x);
            v["labelY"] = encoded(e.label_y);
        }
        json!({"width":r.width,"height":r.height})
    };
    json!({"result":result,"after":after})
}
fn run(entry: &Value) -> Value {
    let a = &entry["args"];
    let s = |i: usize| a[i].as_str().unwrap_or("");
    let n = |i: usize| number(&a[i]);
    let result = match entry["name"].as_str().unwrap() {
        "svg/ink.ts:plainColor" => json!(svg::ink::plain_color(s(0))),
        "svg/ink.ts:ink" => json!(svg::ink::ink(s(0), s(1))),
        "svg/ink.ts:paints" => json!(svg::ink::paints(s(0))),
        "svg/ink.ts:inkFill" => {
            let v = svg::ResolvedStyle {
                shape: a[0]["shape"].as_str().unwrap().into(),
                text: a[0]["text"].as_str().unwrap().into(),
                ..Default::default()
            };
            json!(svg::ink::ink_fill(&v, a[1].as_str()))
        }
        "svg/theme.ts:rounded" => json!(svg::rounded(s(0))),
        "svg/theme.ts:classNames" => json!(svg::class_names(s(0))),
        "svg/theme.ts:fontStyle" => {
            json!(svg::font_style(n(0), a[1].as_str(), a[2].as_str(), false))
        }
        "svg/theme.ts:seriesColor" => json!(svg::series_color(n(0) as usize)),
        "svg/theme.ts:resolveStyle" => {
            let declarations: Vec<_> = a[0]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap())
                .collect();
            let style = svg::resolve_style(&declarations);
            let mut result = json!({"shape":style.shape,"line":style.line,"text":style.text,"bold":style.bold,"italic":style.italic});
            if let Some(v) = style.stroke {
                result["stroke"] = json!(v);
            }
            if let Some(v) = style.font_size {
                result["fontSize"] = json!(v);
            }
            result
        }
        "svg/builder.ts:esc" => json!(svg::esc(s(0))),
        "svg/builder.ts:escText" => json!(svg::esc_text(s(0))),
        "svg/builder.ts:num" => json!(svg::num(n(0))),
        "svg/builder.ts:spanStyle" => json!(svg::span_style(style(n(0) as u64))),
        "svg/builder.ts:textStyle" => json!(svg::text_style(s(0), style(n(1) as u64))),
        "preprocess.ts:normalizeLineBreaks" => {
            json!(s(0).replace("\r\n", "\n").replace('\r', "\n"))
        }
        "preprocess.ts:encodeEntities" => json!(crate::text::entities::encode_entities(s(0))),
        "preprocess.ts:cleanupComments" => json!(pre::cleanup_comments(s(0))),
        "preprocess.ts:preprocess" => match pre::preprocess(s(0)) {
            Ok(p) => {
                let mut v = json!({"text":p.text,"withComments":p.with_comments,"lineOffset":p.line_offset,"config":value(&pre::Value::Map(p.config))});
                if let Some(t) = p.title {
                    v["title"] = json!(t);
                }
                v
            }
            Err(_) => return json!({"error":true}),
        },
        "util/yaml.ts:parseYaml" => match pre::parse_yaml(s(0)) {
            Ok(v) => value(&v),
            Err(_) => return json!({"error":true}),
        },
        "util/url.ts:sanitizeUrl" => json!(crate::url::sanitize_url(s(0))),
        "util/url.ts:safeUrl" => json!(crate::url::safe_url(s(0))),
        "util/url.ts:internalLink" => {
            let mut options = crate::RenderOptions::default();
            if let Some(a) = a[1]["linkSchemes"].as_array() {
                options.link_schemes = Some(a.iter().map(|v| v.as_str().unwrap().into()).collect());
            }
            json!(crate::url::internal_link(s(0), &options))
        }
        "svg/palette.ts:palette" => {
            let pre::Value::Map(conf) = config(&a[1]) else {
                panic!("config")
            };
            let _guard = crate::palette::enter(kind(s(0)), &conf);
            let inks: Vec<_> = (0..8)
                .map(|i| {
                    let fill = crate::palette::series_ink_fill(i);
                    if fill.is_empty() {
                        Value::Null
                    } else {
                        json!(fill.trim_start_matches(" fill=\"").trim_end_matches('"'))
                    }
                })
                .collect();
            let last = crate::palette::style()
                .split(";--pele-series-")
                .skip(1)
                .filter_map(|s| s.split(':').next()?.parse::<usize>().ok())
                .max()
                .unwrap_or(0);
            let mut inks = inks;
            inks.truncate(last);
            json!({"style":crate::palette::style(),"inks":inks})
        }
        "layout/layered.ts:layered" | "layout/compound.ts:compoundLayout" => return layout(entry),
        name => panic!("unhandled official unit: {name}"),
    };
    json!({"result":result})
}
fn equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(_), Value::Number(_)) => {
            let (a, b) = (number(a), number(b));
            (a - b).abs() <= 1e-7_f64.max(1e-14 * a.abs().max(b.abs()))
        }
        (Value::Array(a), Value::Array(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| equal(a, b))
        }
        (Value::Object(a), Value::Object(b)) => {
            a.len() == b.len() && a.iter().all(|(k, a)| b.get(k).is_some_and(|b| equal(a, b)))
        }
        _ => a == b,
    }
}
#[test]
fn official_utilities_and_layouts() {
    let bytes = include_bytes!("../tests/conformance/units.jsonl.gz");
    let reader = std::io::BufReader::new(flate2::read::GzDecoder::new(bytes.as_slice()));
    let mut failures = Vec::new();
    let mut count = 0;
    for line in reader.lines() {
        let e: Value = serde_json::from_str(&line.unwrap()).unwrap();
        let actual = run(&e);
        count += 1;
        let ok = if e.get("error").is_some() {
            actual.get("error").is_some()
        } else {
            equal(&e["result"], &actual["result"])
                && (!e["name"].as_str().unwrap().starts_with("layout/")
                    || equal(&e["after"], &actual["after"]))
        };
        if !ok {
            failures.push(json!({"id":e["id"],"name":e["name"],"origins":e["origins"],"expected":e["result"],"actual":actual}));
        }
    }
    let inventory: Value =
        serde_json::from_str(include_str!("../tests/conformance/manifest.json")).unwrap();
    assert_eq!(
        Some(count),
        inventory["units"].as_u64(),
        "official unit corpus count"
    );
    if !failures.is_empty() {
        std::fs::write(
            std::env::temp_dir().join("pele-rs-official-unit-failures.json"),
            serde_json::to_string_pretty(&failures).unwrap(),
        )
        .unwrap();
    }
    assert!(
        failures.is_empty(),
        "{} / {count} differences; see pele-rs-official-unit-failures.json in the system temporary directory",
        failures.len()
    );
}
