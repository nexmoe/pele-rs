//! The model of a use case diagram: the actors, the use cases, the boundaries, the notes, the
//! JSON tables and the relationships between them. Ports Mermaid's `usecaseModelBuilder.ts`, with
//! its messages, as pele's `db.ts` does.

use std::collections::HashMap;

use crate::error::{Error, ErrorCode};

use super::json::JsonValue;
use super::lexer::{Source, location_text};

/// A stretch of the source, in UTF-16 code units, the end exclusive.
pub(crate) type Span = (usize, usize);

/// Whether a label is plain text or markdown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LabelType {
    Text,
    Markdown,
}

/// The figure an actor is drawn as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ActorType {
    Normal,
    Hollow,
    Awesome,
    Icon,
}

impl ActorType {
    fn as_str(self) -> &'static str {
        match self {
            ActorType::Normal => "normal",
            ActorType::Hollow => "hollow",
            ActorType::Awesome => "awesome",
            ActorType::Icon => "icon",
        }
    }
}

/// The shape a use case is drawn as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UseCaseShape {
    Ellipse,
    Rect,
}

/// The shape a system boundary is drawn as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BoundaryType {
    Rect,
    Package,
}

/// What a relationship means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RelationshipType {
    Association,
    Include,
    Extend,
    Generalization,
}

impl RelationshipType {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            RelationshipType::Association => "association",
            RelationshipType::Include => "include",
            RelationshipType::Extend => "extend",
            RelationshipType::Generalization => "generalization",
        }
    }
}

/// How fast an animated relationship runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Animation {
    Fast,
    Slow,
}

impl Animation {
    #[allow(dead_code, reason = "the golden tests read the name")]
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Animation::Fast => "fast",
            Animation::Slow => "slow",
        }
    }
}

/// The way the diagram is laid out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Direction {
    Tb,
    Bt,
    Lr,
    Rl,
}

impl Direction {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Direction::Tb => "TB",
            Direction::Bt => "BT",
            Direction::Lr => "LR",
            Direction::Rl => "RL",
        }
    }
}

/// The kind of thing an id names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SymbolKind {
    Actor,
    Usecase,
    Boundary,
    Json,
    Edge,
}

impl SymbolKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            SymbolKind::Actor => "actor",
            SymbolKind::Usecase => "usecase",
            SymbolKind::Boundary => "boundary",
            SymbolKind::Json => "json",
            SymbolKind::Edge => "edge",
        }
    }
}

/// A map that keeps the order its keys were first written in, as a JavaScript `Map` does. The
/// shared map of `diagrams/common/ordered.rs` has no way to take a key away, which this model
/// needs when an id changes from an actor to a use case.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Ordered<V> {
    entries: Vec<(String, V)>,
}

impl<V> Ordered<V> {
    pub(crate) fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub(crate) fn get(&self, key: &str) -> Option<&V> {
        self.entries
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value)
    }

    pub(crate) fn get_mut(&mut self, key: &str) -> Option<&mut V> {
        self.entries
            .iter_mut()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value)
    }

    /// Writes a value, keeping the place the key took first.
    pub(crate) fn set(&mut self, key: &str, value: V) {
        match self.entries.iter_mut().find(|(name, _)| name == key) {
            Some(entry) => entry.1 = value,
            None => self.entries.push((key.to_string(), value)),
        }
    }

    pub(crate) fn remove(&mut self, key: &str) {
        self.entries.retain(|(name, _)| name != key);
    }

    pub(crate) fn values(&self) -> impl Iterator<Item = &V> {
        self.entries.iter().map(|(_, value)| value)
    }
}

/// An actor of the diagram.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Actor {
    pub id: String,
    pub label: String,
    pub label_type: LabelType,
    pub actor_type: ActorType,
    pub icon: Option<String>,
    pub business: bool,
    pub stereotype: Option<String>,
    pub parent_id: Option<String>,
    pub classes: Vec<String>,
    pub styles: Vec<String>,
}

/// A use case of the diagram.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct UseCase {
    pub id: String,
    pub label: String,
    pub label_type: LabelType,
    pub shape: UseCaseShape,
    pub business: bool,
    pub stereotype: Option<String>,
    pub parent_id: Option<String>,
    pub classes: Vec<String>,
    pub styles: Vec<String>,
}

/// A system boundary of the diagram.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SystemBoundary {
    pub id: String,
    pub label: String,
    pub label_type: LabelType,
    pub boundary_type: BoundaryType,
    pub members: Vec<String>,
    pub classes: Vec<String>,
    pub styles: Vec<String>,
}

/// A relationship between two elements.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Relationship {
    pub id: String,
    pub explicit_id: bool,
    pub source: String,
    pub target: String,
    pub relationship_type: RelationshipType,
    /// One of Mermaid's arrow kinds, as the order of its `Arrow` enum.
    pub arrow_type: u8,
    pub label: Option<String>,
    pub label_type: Option<LabelType>,
    pub minlen: i32,
    pub classes: Vec<String>,
    pub styles: Vec<String>,
    pub animate: bool,
    pub animation: Option<Animation>,
}

/// A note about an element.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct UsecaseNote {
    pub id: String,
    pub target: String,
    pub label: String,
    pub label_type: LabelType,
}

/// A JSON table of the diagram.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct UsecaseJsonNode {
    pub id: String,
    pub value: JsonValue,
    pub classes: Vec<String>,
    pub styles: Vec<String>,
}

/// The styles of a class.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ClassDef {
    pub id: String,
    pub styles: Vec<String>,
}

/// One statement of the source, with the positions of its parts.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct GraphStatement {
    pub kind: StatementKind,
    pub span: Span,
    pub nodes: Vec<NodeOccurrence>,
    pub edges: Vec<EdgeOccurrence>,
    pub group: Option<String>,
    pub id_span: Option<Span>,
    pub title_span: Option<Span>,
    pub end_span: Option<Span>,
    pub reference: Option<String>,
    pub reference_span: Option<Span>,
    pub children: Vec<GraphStatement>,
    pub metadata: Vec<MetadataOccurrence>,
    pub class_spans: Vec<Span>,
}

/// The kind of a statement of the source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StatementKind {
    Node,
    Edge,
    Group,
    Note,
    Json,
    Metadata,
    EdgeMetadata,
    ClassDef,
    ClassAssign,
    Style,
    Direction,
    AccTitle,
    AccDescr,
    Comment,
    Blank,
}

impl StatementKind {
    #[allow(dead_code, reason = "the golden tests read the name")]
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            StatementKind::Node => "node",
            StatementKind::Edge => "edge",
            StatementKind::Group => "group",
            StatementKind::Note => "note",
            StatementKind::Json => "json",
            StatementKind::Metadata => "metadata",
            StatementKind::EdgeMetadata => "edgeMetadata",
            StatementKind::ClassDef => "classDef",
            StatementKind::ClassAssign => "classAssign",
            StatementKind::Style => "style",
            StatementKind::Direction => "direction",
            StatementKind::AccTitle => "accTitle",
            StatementKind::AccDescr => "accDescr",
            StatementKind::Comment => "comment",
            StatementKind::Blank => "blank",
        }
    }
}

/// A mention of an element.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct NodeOccurrence {
    pub id: String,
    pub span: Span,
    pub id_span: Span,
    pub label_span: Option<Span>,
    pub defines: bool,
    pub stereotype_span: Option<Span>,
    pub metadata: Vec<MetadataOccurrence>,
    pub class_spans: Vec<Span>,
}

/// A mention of a relationship.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct EdgeOccurrence {
    pub id: String,
    pub span: Span,
    pub label_span: Option<Span>,
    pub id_span: Option<Span>,
    pub metadata: Vec<MetadataOccurrence>,
}

/// One property of a metadata block.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct MetadataOccurrence {
    pub key: String,
    pub span: Span,
    pub key_span: Span,
    pub value_span: Span,
}

/// The model of a use case diagram.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct UsecaseModel {
    pub actors: Ordered<Actor>,
    pub use_cases: Ordered<UseCase>,
    pub system_boundaries: Ordered<SystemBoundary>,
    pub relationships: Vec<Relationship>,
    pub notes: Ordered<UsecaseNote>,
    pub json_nodes: Ordered<UsecaseJsonNode>,
    pub class_defs: Ordered<ClassDef>,
    pub direction: Direction,
    pub title: Option<String>,
    pub acc_title: String,
    pub acc_descr: String,
    /// The accessible description as written, before the indentation of its lines is dropped.
    pub acc_description: String,
    pub source: String,
    pub header_span: Span,
    pub statements: Vec<GraphStatement>,
}

/// A label as the parser read it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DraftLabel {
    pub text: String,
    pub kind: LabelType,
    pub span: Span,
}

/// The value of a metadata property.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum PropertyValue {
    Text(String),
    Bool(bool),
}

impl PropertyValue {
    fn as_text(&self) -> Option<&str> {
        match self {
            PropertyValue::Text(text) => Some(text),
            PropertyValue::Bool(_) => None,
        }
    }

    fn as_bool(&self) -> Option<bool> {
        match self {
            PropertyValue::Bool(b) => Some(*b),
            PropertyValue::Text(_) => None,
        }
    }
}

/// One property of a metadata block, as the parser read it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DraftProperty {
    pub key: String,
    pub value: PropertyValue,
    pub span: Span,
    /// Where an error about the property points.
    pub key_span: Span,
    pub value_span: Span,
}

/// An element as the parser read it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DraftElement {
    pub id: String,
    pub kind: ElementKind,
    pub label: DraftLabel,
    pub span: Span,
    pub generated: bool,
    /// The id of the boundary the element is in.
    pub parent: Option<String>,
    pub shape: Option<UseCaseShape>,
    pub metadata: Option<Vec<DraftProperty>>,
    pub stereotype: Option<String>,
    pub classes: Vec<String>,
}

/// The kind of an element.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ElementKind {
    Actor,
    Usecase,
}

/// A system boundary as the parser read it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DraftBoundary {
    pub id: String,
    pub label: DraftLabel,
    pub span: Span,
    pub generated: bool,
    pub classes: Vec<String>,
}

/// A JSON node as the parser read it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DraftJson {
    pub id: String,
    pub value: JsonValue,
    pub span: Span,
    pub classes: Vec<String>,
}

/// One end of a relationship as the parser read it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DraftEndpoint {
    pub id: String,
    pub label: DraftLabel,
    pub span: Span,
    pub generated: bool,
    /// Whether the mention declares the element.
    pub declares: bool,
    /// Whether the mention carries a `:::` suffix.
    pub classed: bool,
}

/// A relationship as the parser read it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DraftRelationship {
    pub source: DraftEndpoint,
    pub target: DraftEndpoint,
    pub span: Span,
    pub id: Option<String>,
    pub id_span: Option<Span>,
    pub relationship_type: RelationshipType,
    pub arrow_type: u8,
    pub label: Option<DraftLabel>,
    pub minlen: i32,
}

/// Everything the parser read, in source order, before any of it is resolved.
#[derive(Debug, Clone, Default)]
pub(crate) struct Drafts {
    pub elements: Vec<DraftElement>,
    pub boundaries: Vec<DraftBoundary>,
    pub jsons: Vec<DraftJson>,
    pub relationships: Vec<DraftRelationship>,
    pub notes: Vec<DraftNote>,
    pub metadata: Vec<DraftMetadata>,
    pub classes: Vec<DraftClassAssign>,
    pub styles: Vec<DraftStyle>,
    pub class_defs: Vec<DraftClassDef>,
    pub direction: Option<Direction>,
    pub acc_title: String,
    pub acc_description: String,
}

/// A note as the parser read it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DraftNote {
    pub target: String,
    pub span: Span,
    pub label: DraftLabel,
}

/// A metadata statement as the parser read it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DraftMetadata {
    pub target: String,
    pub span: Span,
    pub properties: Vec<DraftProperty>,
    /// The statement of the source the metadata belongs to.
    pub statement: usize,
}

/// A `class` statement as the parser read it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DraftClassAssign {
    pub targets: Vec<(String, Span)>,
    pub classes: Vec<String>,
}

/// A `style` statement as the parser read it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DraftStyle {
    pub target: String,
    pub span: Span,
    pub styles: Vec<String>,
}

/// A `classDef` statement as the parser read it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DraftClassDef {
    pub ids: Vec<String>,
    pub styles: Vec<String>,
}

/// Where an id was first mentioned, which decides the order of the model.
#[derive(Debug, Clone)]
struct Origin {
    kind: SymbolKind,
    span: Span,
    generated: bool,
    /// The label a generated id was derived from.
    label: Option<String>,
}

/// An element of the diagram, as the model holds it while it is resolved.
#[derive(Debug, Clone)]
struct ElementState {
    kind: ElementKind,
    id: String,
    label: DraftLabel,
    span: Span,
    parent: Option<String>,
    shape: Option<UseCaseShape>,
    stereotype: Option<String>,
    stereotype_span: Option<Span>,
    classes: Vec<String>,
    actor_type: Option<ActorType>,
    icon: Option<String>,
    business: Option<bool>,
}

/// A system boundary of the diagram, as the model holds it while it is resolved.
#[derive(Debug, Clone)]
struct BoundaryState {
    draft: DraftBoundary,
    boundary_type: Option<BoundaryType>,
    members: Vec<String>,
}

fn push_unique(target: &mut Vec<String>, values: &[String]) {
    for value in values {
        if !target.iter().any(|name| name == value) {
            target.push(value.clone());
        }
    }
}

/// Which kind of element the keys of a metadata block could only belong to, for the error
/// message.
fn infer_kind(properties: &[DraftProperty]) -> Option<&'static str> {
    let mut possible: Vec<&'static str> = vec!["actor", "usecase", "boundary", "edge"];
    for property in properties {
        match property.key.as_str() {
            "icon" => {
                possible.clear();
                possible.push("actor");
            }
            "animate" | "animation" => {
                possible.clear();
                possible.push("edge");
            }
            "type" => {
                possible.clear();
                if property.value.as_text() == Some("rect")
                    || property.value.as_text() == Some("package")
                {
                    possible.push("boundary");
                } else if property.value.as_text() == Some("normal")
                    || property.value.as_text() == Some("hollow")
                    || property.value.as_text() == Some("awesome")
                {
                    possible.push("actor");
                }
            }
            "business" => {
                possible.retain(|kind| *kind != "boundary" && *kind != "edge");
            }
            _ => return None,
        }
    }
    if possible.len() == 1 {
        Some(possible[0])
    } else {
        None
    }
}

/// What a declaration of the source makes.
enum Declaration {
    Element(usize),
    Boundary(usize),
    Json(usize),
    Edge(String, Span),
}

/// Something with an id.
trait HasId {
    fn id(&self) -> &str;
}

impl HasId for ElementState {
    fn id(&self) -> &str {
        &self.id
    }
}

impl HasId for BoundaryState {
    fn id(&self) -> &str {
        &self.draft.id
    }
}

/// The elements of the model, in the order their ids were first mentioned.
fn in_order<'a, V: HasId>(values: &'a Ordered<V>, first: &HashMap<String, usize>) -> Vec<&'a V> {
    let mut out: Vec<&V> = values.values().collect();
    out.sort_by_key(|value| first.get(value.id()).copied().unwrap_or(0));
    out
}

/// Resolves what the parser read into a model, once the whole diagram is read: which ids are
/// actors, use cases, boundaries, JSON nodes or edges, in what order they appear, and whether the
/// statements about them agree.
pub(crate) fn resolve_usecase(
    src: &Source,
    drafts: &mut Drafts,
    header_span: Span,
    statements: Vec<GraphStatement>,
    title: Option<&str>,
) -> Result<UsecaseModel, Error> {
    let mut model = UsecaseModel {
        actors: Ordered::new(),
        use_cases: Ordered::new(),
        system_boundaries: Ordered::new(),
        relationships: Vec::new(),
        notes: Ordered::new(),
        json_nodes: Ordered::new(),
        class_defs: Ordered::new(),
        direction: Direction::Lr,
        title: None,
        acc_title: String::new(),
        acc_descr: String::new(),
        acc_description: String::new(),
        source: src.text.clone(),
        header_span,
        statements,
    };
    let mut symbols: Ordered<Origin> = Ordered::new();
    let mut elements: Ordered<ElementState> = Ordered::new();
    let mut boundaries: Ordered<BoundaryState> = Ordered::new();
    // The place of each relationship in the model, so that a class or a style reaches it.
    let mut edges: Ordered<usize> = Ordered::new();
    // The offset at which each id is first mentioned, which decides the order of the model.
    let mut first: HashMap<String, usize> = HashMap::new();

    let mut mention = |id: &str, offset: usize| match first.get(id) {
        Some(&previous) if previous <= offset => {}
        _ => {
            first.insert(id.to_string(), offset);
        }
    };

    for relationship in &drafts.relationships {
        mention(&relationship.source.id, relationship.source.span.0);
        mention(&relationship.target.id, relationship.target.span.0);
    }
    for element in &drafts.elements {
        mention(&element.id, element.span.0);
    }
    for boundary in &drafts.boundaries {
        mention(&boundary.id, boundary.span.0);
    }
    for json in &drafts.jsons {
        mention(&json.id, json.span.0);
    }

    // The declarations are made in the order they were written, so that the first one of an id
    // is the one that counts.
    let mut declarations: Vec<(usize, Declaration)> = Vec::new();
    for (index, element) in drafts.elements.iter().enumerate() {
        declarations.push((element.span.0, Declaration::Element(index)));
    }
    for (index, boundary) in drafts.boundaries.iter().enumerate() {
        declarations.push((boundary.span.0, Declaration::Boundary(index)));
    }
    for (index, json) in drafts.jsons.iter().enumerate() {
        declarations.push((json.span.0, Declaration::Json(index)));
    }
    for relationship in &drafts.relationships {
        if let (Some(id), Some(span)) = (&relationship.id, relationship.id_span) {
            declarations.push((span.0, Declaration::Edge(id.clone(), span)));
        }
    }
    declarations.sort_by_key(|(offset, _)| *offset);

    for (_, declaration) in declarations {
        match declaration {
            Declaration::Element(index) => {
                let draft = drafts.elements[index].clone();
                declare_element(src, &mut symbols, &mut elements, &draft)?;
            }
            Declaration::Boundary(index) => {
                let draft = drafts.boundaries[index].clone();
                declare_boundary(src, &mut symbols, &mut boundaries, &draft)?;
            }
            Declaration::Json(index) => {
                let draft = &drafts.jsons[index];
                unique(src, &mut symbols, &draft.id, SymbolKind::Json, draft.span)?;
            }
            Declaration::Edge(id, span) => {
                unique(src, &mut symbols, &id, SymbolKind::Edge, span)?;
            }
        }
    }

    for state in in_order(&elements, &first) {
        publish(&mut model, state);
    }
    for state in in_order(&boundaries, &first) {
        model.system_boundaries.set(
            &state.draft.id,
            SystemBoundary {
                id: state.draft.id.clone(),
                label: state.draft.label.text.clone(),
                label_type: state.draft.label.kind,
                boundary_type: BoundaryType::Rect,
                members: Vec::new(),
                classes: Vec::new(),
                styles: Vec::new(),
            },
        );
    }
    let mut json_order: Vec<&DraftJson> = drafts.jsons.iter().collect();
    json_order.sort_by_key(|value| first.get(&value.id).copied().unwrap_or(0));
    for json in json_order {
        model.json_nodes.set(
            &json.id,
            UsecaseJsonNode {
                id: json.id.clone(),
                value: json.value.clone(),
                classes: json.classes.clone(),
                styles: Vec::new(),
            },
        );
    }

    let mut anonymous = 0usize;
    let relationships = drafts.relationships.clone();
    for draft in &relationships {
        let source_kind = endpoint(src, &mut symbols, &mut elements, &draft.source)?;
        let target_kind = endpoint(src, &mut symbols, &mut elements, &draft.target)?;
        let related = |kind: SymbolKind| {
            matches!(
                kind,
                SymbolKind::Actor | SymbolKind::Usecase | SymbolKind::Json
            )
        };
        if !related(source_kind) {
            let span = symbols
                .get(&draft.source.id)
                .map(|origin| origin.span)
                .unwrap_or(draft.source.span);
            return Err(conflict(
                src,
                format!(
                    "Relationship source '{}' cannot be {}",
                    draft.source.id,
                    source_kind.as_str()
                ),
                draft.source.span,
                span,
                None,
                None,
            ));
        }
        if !related(target_kind) {
            let span = symbols
                .get(&draft.target.id)
                .map(|origin| origin.span)
                .unwrap_or(draft.target.span);
            return Err(conflict(
                src,
                format!(
                    "Relationship target '{}' cannot be {}",
                    draft.target.id,
                    target_kind.as_str()
                ),
                draft.target.span,
                span,
                None,
                None,
            ));
        }
        if matches!(
            draft.relationship_type,
            RelationshipType::Include | RelationshipType::Extend
        ) && (source_kind != SymbolKind::Usecase || target_kind != SymbolKind::Usecase)
        {
            return Err(fail(
                src,
                format!(
                    "{} relationship requires use-case endpoints",
                    draft.relationship_type.as_str()
                ),
                draft.span,
            ));
        }
        if draft.relationship_type == RelationshipType::Generalization
            && (source_kind == SymbolKind::Json || source_kind != target_kind)
        {
            return Err(fail(
                src,
                "Generalization requires actor-to-actor or use-case-to-use-case endpoints"
                    .to_string(),
                draft.span,
            ));
        }
        if draft.relationship_type == RelationshipType::Association
            && (source_kind == SymbolKind::Json || target_kind == SymbolKind::Json)
            && draft.arrow_type > 2
        {
            return Err(fail(
                src,
                format!(
                    "JSON relationship '{}' to '{}' permits only point, reversed-point, or markerless solid association",
                    draft.source.id, draft.target.id
                ),
                draft.span,
            ));
        }
        let id = match &draft.id {
            Some(id) => id.clone(),
            None => {
                let id = format!("edge-{anonymous}");
                anonymous += 1;
                id
            }
        };
        let relationship = Relationship {
            id: id.clone(),
            explicit_id: draft.id.is_some(),
            source: draft.source.id.clone(),
            target: draft.target.id.clone(),
            relationship_type: draft.relationship_type,
            arrow_type: draft.arrow_type,
            label: draft.label.as_ref().map(|label| label.text.clone()),
            label_type: draft.label.as_ref().map(|label| label.kind),
            minlen: draft.minlen,
            classes: Vec::new(),
            styles: Vec::new(),
            animate: false,
            animation: None,
        };
        model.relationships.push(relationship);
        edges.set(&id, model.relationships.len() - 1);
    }

    // The relationships may have added use cases; put all elements back in the order of first
    // mention.
    let declared_actors = model.actors.clone();
    let declared_use_cases = model.use_cases.clone();
    model.actors = Ordered::new();
    model.use_cases = Ordered::new();
    for state in in_order(&elements, &first) {
        if let Some(actor) = declared_actors.get(&state.id) {
            let actor = actor.clone();
            model.actors.set(&state.id, actor);
        } else if let Some(use_case) = declared_use_cases.get(&state.id) {
            let use_case = use_case.clone();
            model.use_cases.set(&state.id, use_case);
        } else {
            publish(&mut model, state);
        }
    }

    let metadata = drafts.metadata.clone();
    for draft in &metadata {
        let Some(origin) = symbols.get(&draft.target).cloned() else {
            let inferred = infer_kind(&draft.properties);
            return Err(fail(
                src,
                format!(
                    "Metadata target '{}' is unresolved{}",
                    draft.target,
                    match inferred {
                        Some(kind) => format!(" (metadata implies {kind})"),
                        None => String::new(),
                    }
                ),
                draft.span,
            ));
        };
        let kind = origin.kind;
        if matches!(kind, SymbolKind::Actor | SymbolKind::Usecase) {
            let state = elements
                .get_mut(&draft.target)
                .expect("the element is known");
            element_metadata(src, state, &draft.properties, true)?;
        } else if kind == SymbolKind::Edge {
            let statement = &mut model.statements[draft.statement];
            statement.kind = StatementKind::EdgeMetadata;
            statement.edges = vec![EdgeOccurrence {
                id: draft.target.clone(),
                span: statement.span,
                label_span: None,
                id_span: Some(draft.span),
                metadata: statement.metadata.clone(),
            }];
            statement.nodes.clear();
        }
        if matches!(kind, SymbolKind::Actor | SymbolKind::Usecase) {
            continue;
        }
        for property in &draft.properties {
            if kind == SymbolKind::Boundary && property.key == "type" {
                let boundary_type = match property.value.as_text() {
                    Some("rect") => Some(BoundaryType::Rect),
                    Some("package") => Some(BoundaryType::Package),
                    _ => None,
                };
                if let Some(boundary_type) = boundary_type {
                    if let Some(state) = boundaries.get_mut(&draft.target) {
                        state.boundary_type = Some(boundary_type);
                    }
                    continue;
                }
            }
            if kind == SymbolKind::Edge && property.key == "animate" {
                if let Some(animate) = property.value.as_bool() {
                    if let Some(at) = edges.get(&draft.target).copied() {
                        model.relationships[at].animate = animate;
                    }
                    continue;
                }
            }
            if kind == SymbolKind::Edge && property.key == "animation" {
                let animation = match property.value.as_text() {
                    Some("fast") => Some(Animation::Fast),
                    Some("slow") => Some(Animation::Slow),
                    _ => None,
                };
                if let Some(animation) = animation {
                    if let Some(at) = edges.get(&draft.target).copied() {
                        model.relationships[at].animation = Some(animation);
                        model.relationships[at].animate = true;
                    }
                    continue;
                }
            }
            return Err(invalid(src, &draft.target, kind, property));
        }
    }
    for relationship in &mut model.relationships {
        if relationship.animation.is_some() {
            relationship.animate = true;
        }
    }

    // The members of a boundary are the elements that were written in it.
    let mut members: Vec<(usize, String, String)> = Vec::new();
    for element in &drafts.elements {
        if let Some(parent) = &element.parent {
            members.push((element.span.0, parent.clone(), element.id.clone()));
        }
    }
    members.sort_by_key(|(offset, _, _)| *offset);
    for (_, boundary_id, member) in members {
        if let Some(state) = boundaries.get_mut(&boundary_id) {
            if !state.members.contains(&member) {
                state.members.push(member);
            }
        }
    }
    for state in boundaries.values() {
        if let Some(boundary) = model.system_boundaries.get_mut(&state.draft.id) {
            boundary.boundary_type = state.boundary_type.unwrap_or(BoundaryType::Rect);
            boundary.members = state.members.clone();
            boundary.classes = state.draft.classes.clone();
        }
    }

    // Every element is written into the model again, with the metadata it was given, and the
    // geometry it was asked for is checked.
    let states: Vec<ElementState> = elements.values().cloned().collect();
    for state in &states {
        if state.kind == ElementKind::Actor {
            if state.icon.is_some()
                && state
                    .actor_type
                    .is_some_and(|actor_type| actor_type != ActorType::Normal)
            {
                return Err(fail(
                    src,
                    format!(
                        "Actor '{}' cannot combine icon with type '{}'",
                        state.id,
                        state.actor_type.map(ActorType::as_str).unwrap_or("")
                    ),
                    state.span,
                ));
            }
            if state.business.unwrap_or(false)
                && (state.icon.is_some() || state.actor_type == Some(ActorType::Awesome))
            {
                return Err(fail(
                    src,
                    format!(
                        "Business actor '{}' must use normal or hollow geometry",
                        state.id
                    ),
                    state.span,
                ));
            }
        } else if state.shape == Some(UseCaseShape::Rect) && state.business.unwrap_or(false) {
            return Err(fail(
                src,
                format!(
                    "Rectangular use case '{}' cannot be a business use case",
                    state.id
                ),
                state.span,
            ));
        }
        publish(&mut model, state);
    }

    for (notes, draft) in drafts.notes.iter().enumerate() {
        let Some(origin) = symbols.get(&draft.target).cloned() else {
            return Err(fail(
                src,
                format!("Note target '{}' is unresolved", draft.target),
                draft.span,
            ));
        };
        if !matches!(origin.kind, SymbolKind::Actor | SymbolKind::Usecase) {
            return Err(conflict(
                src,
                format!(
                    "Note target '{}' must be an actor or use case, not {}",
                    draft.target,
                    origin.kind.as_str()
                ),
                draft.span,
                origin.span,
                None,
                None,
            ));
        }
        let id = format!("note-{notes}");
        model.notes.set(
            &id,
            UsecaseNote {
                id: id.clone(),
                target: draft.target.clone(),
                label: draft.label.text.clone(),
                label_type: draft.label.kind,
            },
        );
    }

    for definition in &drafts.class_defs {
        for id in &definition.ids {
            model.class_defs.set(
                id,
                ClassDef {
                    id: id.clone(),
                    styles: definition.styles.clone(),
                },
            );
        }
    }
    for assignment in &drafts.classes {
        for (target, span) in &assignment.targets {
            add_classes(
                src,
                &mut model,
                &symbols,
                &edges,
                target,
                *span,
                &assignment.classes,
            )?;
        }
    }
    for assignment in &drafts.styles {
        add_styles(
            src,
            &mut model,
            &symbols,
            &edges,
            &assignment.target,
            assignment.span,
            &assignment.styles,
        )?;
    }

    model.direction = drafts.direction.unwrap_or(model.direction);
    model.title = title.map(str::to_string);
    model.acc_title = drafts.acc_title.clone();
    model.acc_description = drafts.acc_description.clone();
    model.acc_descr = drop_indentation(&drafts.acc_description);
    Ok(model)
}

/// The indentation of the lines of an accessible description is dropped, as Mermaid does.
fn drop_indentation(text: &str) -> String {
    let mut out = String::new();
    let mut at = 0usize;
    let bytes = text.as_bytes();
    while at < bytes.len() {
        if bytes[at] == b'\n' {
            let mut end = at + 1;
            while end < bytes.len() && (bytes[end] as char).is_whitespace() {
                end += 1;
            }
            if end > at + 1 {
                out.push('\n');
                at = end;
                continue;
            }
        }
        let ch = text[at..].chars().next().unwrap_or('\u{fffd}');
        out.push(ch);
        at += ch.len_utf8();
    }
    out
}

fn fail(src: &Source, message: String, span: Span) -> Error {
    Error::new(
        ErrorCode::Semantic,
        format!("{message} at {}", location_text(src, span.0, span.1)),
    )
    .at(&src.text, src.byte(span.0))
}

fn conflict(
    src: &Source,
    message: String,
    current: Span,
    previous: Span,
    current_label: Option<&str>,
    previous_label: Option<&str>,
) -> Error {
    let suffix = |label: Option<&str>| match label {
        Some(label) => format!(" (label \"{label}\")"),
        None => String::new(),
    };
    Error::new(
        ErrorCode::Semantic,
        format!(
            "{message} at {}{}; previous declaration at {}{}",
            location_text(src, current.0, current.1),
            suffix(current_label),
            location_text(src, previous.0, previous.1),
            suffix(previous_label)
        ),
    )
    .at(&src.text, src.byte(current.0))
}

fn invalid(src: &Source, id: &str, kind: SymbolKind, property: &DraftProperty) -> Error {
    fail(
        src,
        format!(
            "Metadata property '{}' is invalid for {} '{}'",
            property.key,
            kind.as_str(),
            id
        ),
        property.key_span,
    )
}

/// An id means one thing in the whole diagram, and an id made from a label may be declared once.
fn claim(
    src: &Source,
    symbols: &mut Ordered<Origin>,
    id: &str,
    kind: SymbolKind,
    span: Span,
    generated: bool,
    label: &str,
) -> Result<(), Error> {
    let Some(origin) = symbols.get(id) else {
        return Ok(());
    };
    let origin = origin.clone();
    let label = if generated { Some(label) } else { None };
    let origin_label = if origin.generated {
        origin.label.as_deref()
    } else {
        None
    };
    if origin.kind != kind {
        return Err(conflict(
            src,
            format!(
                "ID '{id}' is declared as both {} and {}",
                origin.kind.as_str(),
                kind.as_str()
            ),
            span,
            origin.span,
            label,
            origin_label,
        ));
    }
    if origin.generated || generated {
        return Err(conflict(
            src,
            format!("Generated ID '{id}' collides with another declaration"),
            span,
            origin.span,
            label,
            origin_label,
        ));
    }
    Ok(())
}

/// An id that may be declared only once.
fn unique(
    src: &Source,
    symbols: &mut Ordered<Origin>,
    id: &str,
    kind: SymbolKind,
    span: Span,
) -> Result<(), Error> {
    if let Some(previous) = symbols.get(id) {
        let previous = previous.clone();
        return Err(conflict(
            src,
            format!(
                "ID '{id}' is declared more than once ({} and {})",
                previous.kind.as_str(),
                kind.as_str()
            ),
            span,
            previous.span,
            None,
            if previous.generated {
                previous.label.as_deref()
            } else {
                None
            },
        ));
    }
    symbols.set(
        id,
        Origin {
            kind,
            span,
            generated: false,
            label: None,
        },
    );
    Ok(())
}

/// A metadata statement replaces what a declaration said; two declarations have to agree.
fn actor_property(
    src: &Source,
    state: &mut ElementState,
    property: &DraftProperty,
    replace: bool,
) -> Result<(), Error> {
    let key = property.key.as_str();
    let conflict_error = || {
        conflict(
            src,
            format!("Actor '{}' has conflicting {key} metadata", state.id),
            property.key_span,
            state.span,
            None,
            None,
        )
    };
    match key {
        "type" => {
            let actor_type = match property.value.as_text() {
                Some("normal") => Some(ActorType::Normal),
                Some("hollow") => Some(ActorType::Hollow),
                Some("awesome") => Some(ActorType::Awesome),
                _ => None,
            };
            if let Some(actor_type) = actor_type {
                if !replace
                    && state
                        .actor_type
                        .is_some_and(|was| was.as_str() != actor_type.as_str())
                {
                    return Err(conflict_error());
                }
                state.actor_type = Some(actor_type);
                return Ok(());
            }
        }
        "icon" => {
            if let Some(icon) = property.value.as_text() {
                if !replace && state.icon.as_deref().is_some_and(|was| was != icon) {
                    return Err(conflict_error());
                }
                state.icon = Some(icon.to_string());
                return Ok(());
            }
        }
        "business" => {
            if let Some(business) = property.value.as_bool() {
                if !replace && state.business.is_some_and(|was| was != business) {
                    return Err(conflict_error());
                }
                state.business = Some(business);
                return Ok(());
            }
        }
        _ => {}
    }
    Err(invalid(src, &state.id, SymbolKind::Actor, property))
}

fn element_metadata(
    src: &Source,
    state: &mut ElementState,
    properties: &[DraftProperty],
    replace: bool,
) -> Result<(), Error> {
    for property in properties {
        if state.kind == ElementKind::Actor {
            actor_property(src, state, property, replace)?;
        } else if property.key == "business" && property.value.as_bool().is_some() {
            let business = property.value.as_bool().unwrap_or(false);
            if !replace && state.business.is_some_and(|was| was != business) {
                return Err(conflict(
                    src,
                    format!("Use case '{}' has conflicting business metadata", state.id),
                    property.key_span,
                    state.span,
                    None,
                    None,
                ));
            }
            state.business = Some(business);
        } else {
            return Err(invalid(src, &state.id, SymbolKind::Usecase, property));
        }
    }
    Ok(())
}

/// Writes an element into the model.
fn publish(model: &mut UsecaseModel, state: &ElementState) {
    match state.kind {
        ElementKind::Actor => {
            model.use_cases.remove(&state.id);
            let actor = Actor {
                id: state.id.clone(),
                label: state.label.text.clone(),
                label_type: state.label.kind,
                actor_type: if state.icon.is_some() {
                    ActorType::Icon
                } else {
                    state.actor_type.unwrap_or(ActorType::Normal)
                },
                icon: state.icon.clone(),
                business: state.business.unwrap_or(false),
                stereotype: state.stereotype.clone(),
                parent_id: state.parent.clone(),
                classes: state.classes.clone(),
                styles: model
                    .actors
                    .get(&state.id)
                    .map(|actor| actor.styles.clone())
                    .unwrap_or_default(),
            };
            model.actors.set(&state.id, actor);
        }
        ElementKind::Usecase => {
            model.actors.remove(&state.id);
            let use_case = UseCase {
                id: state.id.clone(),
                label: state.label.text.clone(),
                label_type: state.label.kind,
                shape: state.shape.unwrap_or(UseCaseShape::Ellipse),
                business: state.business.unwrap_or(false),
                stereotype: state.stereotype.clone(),
                parent_id: state.parent.clone(),
                classes: state.classes.clone(),
                styles: model
                    .use_cases
                    .get(&state.id)
                    .map(|use_case| use_case.styles.clone())
                    .unwrap_or_default(),
            };
            model.use_cases.set(&state.id, use_case);
        }
    }
}

/// An element of the diagram, as the parser read it, is declared.
fn declare_element(
    src: &Source,
    symbols: &mut Ordered<Origin>,
    elements: &mut Ordered<ElementState>,
    draft: &DraftElement,
) -> Result<(), Error> {
    let kind = match draft.kind {
        ElementKind::Actor => SymbolKind::Actor,
        ElementKind::Usecase => SymbolKind::Usecase,
    };
    claim(
        src,
        symbols,
        &draft.id,
        kind,
        draft.span,
        draft.generated,
        &draft.label.text,
    )?;
    let existing = elements.get(&draft.id).cloned();
    match existing {
        None => {
            let mut state = ElementState {
                kind: draft.kind,
                id: draft.id.clone(),
                label: draft.label.clone(),
                span: draft.span,
                parent: draft.parent.clone(),
                shape: draft.shape,
                stereotype: draft.stereotype.clone(),
                stereotype_span: draft.stereotype.as_ref().map(|_| draft.span),
                classes: draft.classes.clone(),
                actor_type: None,
                icon: None,
                business: None,
            };
            if let Some(properties) = &draft.metadata {
                element_metadata(src, &mut state, properties, false)?;
            }
            elements.set(&draft.id, state);
            symbols.set(
                &draft.id,
                Origin {
                    kind,
                    span: draft.span,
                    generated: draft.generated,
                    label: Some(draft.label.text.clone()),
                },
            );
            Ok(())
        }
        Some(mut state) => {
            if state.label.text != draft.label.text || state.label.kind != draft.label.kind {
                return Err(conflict(
                    src,
                    format!("ID '{}' has conflicting labels", draft.id),
                    draft.span,
                    state.span,
                    None,
                    None,
                ));
            }
            if let (Some(was), Some(is)) = (state.shape, draft.shape) {
                if was != is {
                    return Err(conflict(
                        src,
                        format!("Use case '{}' has conflicting shapes", draft.id),
                        draft.span,
                        state.span,
                        None,
                        None,
                    ));
                }
            }
            // A boundary whose title makes an empty id is no parent, but it still takes the
            // place of one.
            if let (Some(parent), Some(was)) = (&draft.parent, &state.parent) {
                if parent != was {
                    return Err(conflict(
                        src,
                        format!(
                            "Element '{}' belongs to more than one system boundary",
                            draft.id
                        ),
                        draft.span,
                        state.span,
                        None,
                        None,
                    ));
                }
            }
            if let (Some(was), Some(is)) = (&state.stereotype, &draft.stereotype) {
                if was != is {
                    return Err(conflict(
                        src,
                        format!("Element '{}' has conflicting stereotypes", draft.id),
                        draft.span,
                        state.stereotype_span.unwrap_or(state.span),
                        None,
                        None,
                    ));
                }
            }
            if state.shape.is_none() {
                state.shape = draft.shape;
            }
            if state.parent.is_none() {
                state.parent = draft.parent.clone();
            }
            if state.stereotype.is_none() {
                state.stereotype = draft.stereotype.clone();
            }
            if state.stereotype_span.is_none() {
                state.stereotype_span = draft.stereotype.as_ref().map(|_| draft.span);
            }
            push_unique(&mut state.classes, &draft.classes);
            if let Some(properties) = &draft.metadata {
                element_metadata(src, &mut state, properties, false)?;
            }
            elements.set(&draft.id, state);
            Ok(())
        }
    }
}

/// A system boundary of the diagram, as the parser read it, is declared.
fn declare_boundary(
    src: &Source,
    symbols: &mut Ordered<Origin>,
    boundaries: &mut Ordered<BoundaryState>,
    draft: &DraftBoundary,
) -> Result<(), Error> {
    claim(
        src,
        symbols,
        &draft.id,
        SymbolKind::Boundary,
        draft.span,
        draft.generated,
        &draft.label.text,
    )?;
    if let Some(existing) = boundaries.get(&draft.id).cloned() {
        if existing.draft.label.text != draft.label.text
            || existing.draft.label.kind != draft.label.kind
        {
            return Err(conflict(
                src,
                format!("Boundary '{}' has conflicting titles", draft.id),
                draft.span,
                existing.draft.span,
                None,
                None,
            ));
        }
        let mut existing = existing;
        push_unique(&mut existing.draft.classes, &draft.classes);
        boundaries.set(&draft.id, existing);
        return Ok(());
    }
    boundaries.set(
        &draft.id,
        BoundaryState {
            draft: draft.clone(),
            boundary_type: None,
            members: Vec::new(),
        },
    );
    symbols.set(
        &draft.id,
        Origin {
            kind: SymbolKind::Boundary,
            span: draft.span,
            generated: draft.generated,
            label: Some(draft.label.text.clone()),
        },
    );
    Ok(())
}

/// An end of a relationship that nothing declares becomes a use case.
fn endpoint(
    src: &Source,
    symbols: &mut Ordered<Origin>,
    elements: &mut Ordered<ElementState>,
    end: &DraftEndpoint,
) -> Result<SymbolKind, Error> {
    if end.classed && !end.declares {
        return Err(fail(
            src,
            format!(
                "Relationship endpoint '{}' uses ::: without declaring the node",
                end.id
            ),
            end.span,
        ));
    }
    if let Some(origin) = symbols.get(&end.id) {
        return Ok(origin.kind);
    }
    elements.set(
        &end.id,
        ElementState {
            kind: ElementKind::Usecase,
            id: end.id.clone(),
            label: end.label.clone(),
            span: end.span,
            parent: None,
            shape: Some(UseCaseShape::Ellipse),
            stereotype: None,
            stereotype_span: None,
            classes: Vec::new(),
            actor_type: None,
            icon: None,
            business: None,
        },
    );
    symbols.set(
        &end.id,
        Origin {
            kind: SymbolKind::Usecase,
            span: end.span,
            generated: end.generated,
            label: Some(end.label.text.clone()),
        },
    );
    Ok(SymbolKind::Usecase)
}

/// The classes of the thing a `class` statement names.
fn add_classes(
    src: &Source,
    model: &mut UsecaseModel,
    symbols: &Ordered<Origin>,
    edges: &Ordered<usize>,
    id: &str,
    span: Span,
    classes: &[String],
) -> Result<(), Error> {
    let kind = symbols.get(id).map(|origin| origin.kind);
    match kind {
        Some(SymbolKind::Actor) => {
            let actor = model
                .actors
                .get_mut(id)
                .ok_or_else(|| unresolved(src, id, span))?;
            push_unique(&mut actor.classes, classes);
        }
        Some(SymbolKind::Usecase) => {
            let use_case = model
                .use_cases
                .get_mut(id)
                .ok_or_else(|| unresolved(src, id, span))?;
            push_unique(&mut use_case.classes, classes);
        }
        Some(SymbolKind::Boundary) => {
            let boundary = model
                .system_boundaries
                .get_mut(id)
                .ok_or_else(|| unresolved(src, id, span))?;
            push_unique(&mut boundary.classes, classes);
        }
        Some(SymbolKind::Json) => {
            let json = model
                .json_nodes
                .get_mut(id)
                .ok_or_else(|| unresolved(src, id, span))?;
            push_unique(&mut json.classes, classes);
        }
        Some(SymbolKind::Edge) => {
            let at = edges
                .get(id)
                .copied()
                .ok_or_else(|| unresolved(src, id, span))?;
            push_unique(&mut model.relationships[at].classes, classes);
        }
        _ => return Err(unresolved(src, id, span)),
    }
    Ok(())
}

/// The styles of the thing a `style` statement names.
fn add_styles(
    src: &Source,
    model: &mut UsecaseModel,
    symbols: &Ordered<Origin>,
    edges: &Ordered<usize>,
    id: &str,
    span: Span,
    styles: &[String],
) -> Result<(), Error> {
    let kind = symbols.get(id).map(|origin| origin.kind);
    match kind {
        Some(SymbolKind::Actor) => {
            let actor = model
                .actors
                .get_mut(id)
                .ok_or_else(|| unresolved(src, id, span))?;
            actor.styles.extend_from_slice(styles);
        }
        Some(SymbolKind::Usecase) => {
            let use_case = model
                .use_cases
                .get_mut(id)
                .ok_or_else(|| unresolved(src, id, span))?;
            use_case.styles.extend_from_slice(styles);
        }
        Some(SymbolKind::Boundary) => {
            let boundary = model
                .system_boundaries
                .get_mut(id)
                .ok_or_else(|| unresolved(src, id, span))?;
            boundary.styles.extend_from_slice(styles);
        }
        Some(SymbolKind::Json) => {
            let json = model
                .json_nodes
                .get_mut(id)
                .ok_or_else(|| unresolved(src, id, span))?;
            json.styles.extend_from_slice(styles);
        }
        Some(SymbolKind::Edge) => {
            let at = edges
                .get(id)
                .copied()
                .ok_or_else(|| unresolved(src, id, span))?;
            model.relationships[at].styles.extend_from_slice(styles);
        }
        _ => return Err(unresolved(src, id, span)),
    }
    Ok(())
}

fn unresolved(src: &Source, id: &str, span: Span) -> Error {
    fail(
        src,
        format!("Class/style target '{id}' is unresolved or anonymous"),
        span,
    )
}
