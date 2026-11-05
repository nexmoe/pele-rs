//! The tokens of Mermaid's C4 grammar, as its jison lexer returns them.

/// The tokens of `c4Diagram.jison`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum T {
    End,
    /// Not a jison token: the lexer would return the same empty token forever from here.
    Error,
    DirectionTb,
    DirectionBt,
    DirectionRl,
    DirectionLr,
    Title,
    AccDescription,
    AccTitle,
    AccTitleValue,
    AccDescr,
    AccDescrValue,
    AccDescrMultilineValue,
    Newline,
    C4Context,
    C4Container,
    C4Component,
    C4Dynamic,
    C4Deployment,
    Person,
    PersonExt,
    System,
    SystemDb,
    SystemQueue,
    SystemExt,
    SystemExtDb,
    SystemExtQueue,
    Container,
    ContainerDb,
    ContainerQueue,
    ContainerExt,
    ContainerExtDb,
    ContainerExtQueue,
    Component,
    ComponentDb,
    ComponentQueue,
    ComponentExt,
    ComponentExtDb,
    ComponentExtQueue,
    EnterpriseBoundary,
    SystemBoundary,
    Boundary,
    ContainerBoundary,
    Node,
    NodeL,
    NodeR,
    Rel,
    BiRel,
    RelU,
    RelD,
    RelL,
    RelR,
    RelB,
    RelIndex,
    UpdateElStyle,
    UpdateRelStyle,
    UpdateLayoutConfig,
    EofInStruct,
    AttributeEmpty,
    Str,
    StrKey,
    StrValue,
    LBrace,
    RBrace,
    Eof,
}

impl T {
    /// The token's name, as the error messages of the parser name it.
    pub(crate) fn name(self) -> &'static str {
        match self {
            T::End => "$end",
            T::Error => "INVALID",
            T::DirectionTb => "direction_tb",
            T::DirectionBt => "direction_bt",
            T::DirectionRl => "direction_rl",
            T::DirectionLr => "direction_lr",
            T::Title => "title",
            T::AccDescription => "accDescription",
            T::AccTitle => "acc_title",
            T::AccTitleValue => "acc_title_value",
            T::AccDescr => "acc_descr",
            T::AccDescrValue => "acc_descr_value",
            T::AccDescrMultilineValue => "acc_descr_multiline_value",
            T::Newline => "NEWLINE",
            T::C4Context => "C4_CONTEXT",
            T::C4Container => "C4_CONTAINER",
            T::C4Component => "C4_COMPONENT",
            T::C4Dynamic => "C4_DYNAMIC",
            T::C4Deployment => "C4_DEPLOYMENT",
            T::Person => "PERSON",
            T::PersonExt => "PERSON_EXT",
            T::System => "SYSTEM",
            T::SystemDb => "SYSTEM_DB",
            T::SystemQueue => "SYSTEM_QUEUE",
            T::SystemExt => "SYSTEM_EXT",
            T::SystemExtDb => "SYSTEM_EXT_DB",
            T::SystemExtQueue => "SYSTEM_EXT_QUEUE",
            T::Container => "CONTAINER",
            T::ContainerDb => "CONTAINER_DB",
            T::ContainerQueue => "CONTAINER_QUEUE",
            T::ContainerExt => "CONTAINER_EXT",
            T::ContainerExtDb => "CONTAINER_EXT_DB",
            T::ContainerExtQueue => "CONTAINER_EXT_QUEUE",
            T::Component => "COMPONENT",
            T::ComponentDb => "COMPONENT_DB",
            T::ComponentQueue => "COMPONENT_QUEUE",
            T::ComponentExt => "COMPONENT_EXT",
            T::ComponentExtDb => "COMPONENT_EXT_DB",
            T::ComponentExtQueue => "COMPONENT_EXT_QUEUE",
            T::EnterpriseBoundary => "ENTERPRISE_BOUNDARY",
            T::SystemBoundary => "SYSTEM_BOUNDARY",
            T::Boundary => "BOUNDARY",
            T::ContainerBoundary => "CONTAINER_BOUNDARY",
            T::Node => "NODE",
            T::NodeL => "NODE_L",
            T::NodeR => "NODE_R",
            T::Rel => "REL",
            T::BiRel => "BIREL",
            T::RelU => "REL_U",
            T::RelD => "REL_D",
            T::RelL => "REL_L",
            T::RelR => "REL_R",
            T::RelB => "REL_B",
            T::RelIndex => "REL_INDEX",
            T::UpdateElStyle => "UPDATE_EL_STYLE",
            T::UpdateRelStyle => "UPDATE_REL_STYLE",
            T::UpdateLayoutConfig => "UPDATE_LAYOUT_CONFIG",
            T::EofInStruct => "EOF_IN_STRUCT",
            T::AttributeEmpty => "ATTRIBUTE_EMPTY",
            T::Str => "STR",
            T::StrKey => "STR_KEY",
            T::StrValue => "STR_VALUE",
            T::LBrace => "LBRACE",
            T::RBrace => "RBRACE",
            T::Eof => "EOF",
        }
    }

    /// Whether the token is one of the statements that draw something.
    pub(crate) fn is_diagram(self) -> bool {
        matches!(
            self,
            T::Person
                | T::PersonExt
                | T::System
                | T::SystemDb
                | T::SystemQueue
                | T::SystemExt
                | T::SystemExtDb
                | T::SystemExtQueue
                | T::Container
                | T::ContainerDb
                | T::ContainerQueue
                | T::ContainerExt
                | T::ContainerExtDb
                | T::ContainerExtQueue
                | T::Component
                | T::ComponentDb
                | T::ComponentQueue
                | T::ComponentExt
                | T::ComponentExtDb
                | T::ComponentExtQueue
                | T::EnterpriseBoundary
                | T::SystemBoundary
                | T::Boundary
                | T::ContainerBoundary
                | T::Node
                | T::NodeL
                | T::NodeR
                | T::Rel
                | T::BiRel
                | T::RelU
                | T::RelD
                | T::RelL
                | T::RelR
                | T::RelB
                | T::RelIndex
                | T::UpdateElStyle
                | T::UpdateRelStyle
                | T::UpdateLayoutConfig
        )
    }

    /// Whether the token is one of the statements that do not draw anything.
    pub(crate) fn is_other(self) -> bool {
        matches!(
            self,
            T::Title | T::AccDescription | T::AccTitle | T::AccDescr | T::AccDescrMultilineValue
        )
    }

    /// Mermaid's `typeC4Shape` of a shape keyword, as the grammar's semantic actions use it.
    pub(crate) fn kind(self) -> Option<&'static str> {
        const KINDS: [&str; 20] = [
            "person",
            "external_person",
            "system",
            "system_db",
            "system_queue",
            "external_system",
            "external_system_db",
            "external_system_queue",
            "container",
            "container_db",
            "container_queue",
            "external_container",
            "external_container_db",
            "external_container_queue",
            "component",
            "component_db",
            "component_queue",
            "external_component",
            "external_component_db",
            "external_component_queue",
        ];
        let at = match self {
            T::Person => 0,
            T::PersonExt => 1,
            T::System => 2,
            T::SystemDb => 3,
            T::SystemQueue => 4,
            T::SystemExt => 5,
            T::SystemExtDb => 6,
            T::SystemExtQueue => 7,
            T::Container => 8,
            T::ContainerDb => 9,
            T::ContainerQueue => 10,
            T::ContainerExt => 11,
            T::ContainerExtDb => 12,
            T::ContainerExtQueue => 13,
            T::Component => 14,
            T::ComponentDb => 15,
            T::ComponentQueue => 16,
            T::ComponentExt => 17,
            T::ComponentExtDb => 18,
            T::ComponentExtQueue => 19,
            _ => return None,
        };
        Some(KINDS[at])
    }

    /// Mermaid's node type of a deployment node keyword.
    pub(crate) fn node_type(self) -> Option<&'static str> {
        match self {
            T::Node => Some("node"),
            T::NodeL => Some("nodeL"),
            T::NodeR => Some("nodeR"),
            _ => None,
        }
    }

    /// The kind of relation a relation keyword draws.
    pub(crate) fn rel_type(self) -> Option<&'static str> {
        match self {
            T::Rel => Some("rel"),
            T::BiRel => Some("birel"),
            T::RelU => Some("rel_u"),
            T::RelD => Some("rel_d"),
            T::RelL => Some("rel_l"),
            T::RelR => Some("rel_r"),
            T::RelB => Some("rel_b"),
            T::RelIndex => Some("rel"),
            _ => None,
        }
    }

    /// The keyword that opens the diagram, and the one that names the kind of diagram.
    pub(crate) fn is_header(self) -> bool {
        matches!(
            self,
            T::C4Context | T::C4Container | T::C4Component | T::C4Dynamic | T::C4Deployment
        )
    }
}

/// The keyword of every shape of the grammar, and the token it returns.
pub(crate) const KEYWORDS: &[(&str, T)] = &[
    ("C4Context", T::C4Context),
    ("C4Container", T::C4Container),
    ("C4Component", T::C4Component),
    ("C4Dynamic", T::C4Dynamic),
    ("C4Deployment", T::C4Deployment),
    ("Person", T::Person),
    ("Person_Ext", T::PersonExt),
    ("System", T::System),
    ("SystemDb", T::SystemDb),
    ("SystemQueue", T::SystemQueue),
    ("System_Ext", T::SystemExt),
    ("SystemDb_Ext", T::SystemExtDb),
    ("SystemQueue_Ext", T::SystemExtQueue),
    ("Container", T::Container),
    ("ContainerDb", T::ContainerDb),
    ("ContainerQueue", T::ContainerQueue),
    ("Container_Ext", T::ContainerExt),
    ("ContainerDb_Ext", T::ContainerExtDb),
    ("ContainerQueue_Ext", T::ContainerExtQueue),
    ("Component", T::Component),
    ("ComponentDb", T::ComponentDb),
    ("ComponentQueue", T::ComponentQueue),
    ("Component_Ext", T::ComponentExt),
    ("ComponentDb_Ext", T::ComponentExtDb),
    ("ComponentQueue_Ext", T::ComponentExtQueue),
    ("Enterprise_Boundary", T::EnterpriseBoundary),
    ("System_Boundary", T::SystemBoundary),
    ("Boundary", T::Boundary),
    ("Container_Boundary", T::ContainerBoundary),
    ("Deployment_Node", T::Node),
    ("Node", T::Node),
    ("Node_L", T::NodeL),
    ("Node_R", T::NodeR),
    ("Rel", T::Rel),
    ("BiRel", T::BiRel),
    ("Rel_Up", T::RelU),
    ("Rel_U", T::RelU),
    ("Rel_Down", T::RelD),
    ("Rel_D", T::RelD),
    ("Rel_Left", T::RelL),
    ("Rel_L", T::RelL),
    ("Rel_Right", T::RelR),
    ("Rel_R", T::RelR),
    ("Rel_Back", T::RelB),
    ("RelIndex", T::RelIndex),
    ("UpdateElementStyle", T::UpdateElStyle),
    ("UpdateRelStyle", T::UpdateRelStyle),
    ("UpdateLayoutConfig", T::UpdateLayoutConfig),
];

/// The longest keyword. A longer word is never one.
pub(crate) const KEYWORD_MAX: usize = 19;
