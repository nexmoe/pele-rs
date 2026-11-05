//! The model of a git graph: the commits, the branches, and the lanes they are drawn in.
//! Ported from pele's `diagrams/git/db.ts`.

use crate::diagrams::common::ordered::Ordered;
use crate::diagrams::git::parser::{CommitType, Direction, GitAst, Statement};
use crate::error::{Error, ErrorCode};
use crate::preprocess::{Config, Value};
use crate::util::is_space;

/// The kind of a commit, as Mermaid numbers them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CommitKind {
    Normal = 0,
    Reverse = 1,
    Highlight = 2,
    Merge = 3,
    CherryPick = 4,
}

impl CommitKind {
    /// The number Mermaid gives the kind.
    pub(crate) fn index(self) -> usize {
        self as usize
    }
}

/// A commit of the history.
#[derive(Debug, Clone)]
pub(crate) struct GitCommit {
    pub id: String,
    pub message: String,
    /// The place of the commit in the history.
    pub seq: usize,
    pub kind: CommitKind,
    pub tags: Vec<String>,
    /// The ids of the commits this one comes after.
    pub parents: Vec<String>,
    /// The branch the commit was made on.
    pub branch: String,
    /// The kind the commit was given, when it differs from the kind of a merge or a cherry-pick.
    pub custom_type: Option<CommitKind>,
    /// Whether the commit was given an id of its own.
    pub custom_id: bool,
}

/// The model of a git graph.
#[derive(Debug, Clone)]
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "read by the compat test, which is only built for the tests"
    )
)]
pub(crate) struct GitModel {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub direction: Direction,
    /// The commits, keyed by their id, in the order the ids first appeared.
    pub commits: Ordered<GitCommit>,
    /// The id of each branch's latest commit, or nothing while the branch has none.
    pub branches: Ordered<Option<String>>,
    /// The branch names in drawing order: by their order, then by creation.
    pub lanes: Vec<String>,
    pub current_branch: String,
    /// The id of the commit at the head of the current branch.
    pub head: Option<String>,
    pub warnings: Vec<String>,
}

/// A failure of the history, as Mermaid reports it.
fn fail(message: String) -> Error {
    Error::new(ErrorCode::Semantic, message)
}

/// The kind a statement gives a commit.
fn kind_of(kind: CommitType) -> CommitKind {
    match kind {
        CommitType::Reverse => CommitKind::Reverse,
        CommitType::Highlight => CommitKind::Highlight,
        CommitType::Normal => CommitKind::Normal,
    }
}

/// Builds the model of a git graph out of what the parser read.
pub(crate) fn build_git(
    ast: &GitAst,
    config: &Config,
    title: Option<&str>,
) -> Result<GitModel, Error> {
    let main_name = match setting(config, "mainBranchName") {
        Some(Value::String(name)) => name.clone(),
        _ => "main".to_string(),
    };
    let main_order = match setting(config, "mainBranchOrder") {
        Some(Value::Number(order)) => *order,
        _ => 0.0,
    };

    let mut builder = Builder {
        commits: Ordered::default(),
        branches: Ordered::default(),
        orders: Ordered::default(),
        warnings: Vec::new(),
        head: None,
        current: main_name.clone(),
        direction: Direction::LeftToRight,
        seq: 0,
    };
    builder.branches.set(&main_name, None);
    builder.orders.set(&main_name, main_order);

    if let Some(direction) = ast.direction {
        builder.direction = direction;
    }

    for statement in &ast.statements {
        builder.run(statement)?;
    }

    // A branch made without an order sorts by creation among those of order zero, as in Mermaid.
    let mut lanes: Vec<(String, f64)> = builder.orders.items.clone();
    lanes.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

    Ok(GitModel {
        title: match &ast.title {
            Some(title) if !title.is_empty() => Some(title.clone()),
            _ => title.map(str::to_string),
        },
        acc_title: ast
            .acc_title
            .as_deref()
            .filter(|text| !text.is_empty())
            .map(|text| crate::util::trim_start(text).to_string()),
        acc_descr: ast
            .acc_descr
            .as_deref()
            .filter(|text| !text.is_empty())
            .map(trim_lines),
        direction: builder.direction,
        commits: builder.commits,
        branches: builder.branches,
        lanes: lanes.into_iter().map(|(name, _)| name).collect(),
        current_branch: builder.current,
        head: builder.head,
        warnings: builder.warnings,
    })
}

/// A value of the settings of the diagram.
fn setting<'a>(config: &'a Config, key: &str) -> Option<&'a Value> {
    match config.get("gitGraph") {
        Some(Value::Map(map)) => map.get(key),
        _ => None,
    }
}

/// Replaces every line break and the whitespace after it with a single line break.
fn trim_lines(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find('\n') {
        out.push_str(&rest[..i]);
        out.push('\n');
        rest = rest[i + 1..].trim_start_matches(is_space);
    }
    out.push_str(rest);
    out
}

/// The history as it is built, statement by statement.
struct Builder {
    commits: Ordered<GitCommit>,
    branches: Ordered<Option<String>>,
    orders: Ordered<f64>,
    warnings: Vec<String>,
    head: Option<String>,
    current: String,
    direction: Direction,
    seq: usize,
}

impl Builder {
    /// The id of the next commit. Mermaid appends seven random hex digits to the number of the
    /// commit; these digits are derived from the number instead, so the same text always gives
    /// the same ids.
    fn next_id(&self) -> String {
        let mut salt = (self.seq as u32).wrapping_add(1);
        loop {
            let id = format!("{}-{}", self.seq, hex7(salt.wrapping_mul(0x9e37_79b1)));
            if !self.commits.contains(&id) {
                return id;
            }
            salt = salt.wrapping_add(0x9e37_79b1);
        }
    }

    /// Adds a commit, which becomes the head of the current branch.
    fn add(&mut self, mut commit: GitCommit) {
        commit.seq = self.seq;
        self.seq += 1;
        self.head = Some(commit.id.clone());
        let id = commit.id.clone();
        self.commits.set(&id, commit);
        let current = self.current.clone();
        self.branches.set(&current, self.head.clone());
    }

    /// Runs a statement of the graph.
    fn run(&mut self, statement: &Statement) -> Result<(), Error> {
        match statement {
            Statement::Commit {
                id,
                message,
                tags,
                kind,
            } => {
                let commit_id = match id {
                    Some(id) => id.clone(),
                    None => self.next_id(),
                };
                if self.commits.contains(&commit_id) {
                    self.warnings
                        .push(format!("Commit ID {commit_id} already exists"));
                }
                let parents = self.head.clone().into_iter().collect();
                self.add(GitCommit {
                    id: commit_id,
                    message: message.clone().unwrap_or_default(),
                    seq: 0,
                    kind: kind.map_or(CommitKind::Normal, kind_of),
                    tags: tags.clone(),
                    parents,
                    branch: self.current.clone(),
                    custom_type: None,
                    custom_id: false,
                });
            }
            Statement::Branch { name, order } => {
                if self.branches.contains(name) {
                    return Err(fail(format!(
                        "Trying to create an existing branch. (Help: Either use a new name if you \
                         want create a new branch or try using \"checkout {name}\")"
                    )));
                }
                self.branches.set(name, self.head.clone());
                self.orders.set(name, order.unwrap_or(0.0));
                self.checkout(name)?;
            }
            Statement::Merge {
                branch,
                id,
                tags,
                kind,
            } => self.merge(branch, id.as_deref(), tags, *kind)?,
            Statement::Checkout { branch } => self.checkout(branch)?,
            Statement::CherryPicking { id, tags, parent } => {
                self.cherry_pick(id.as_deref(), tags, parent.as_deref())?
            }
        }
        Ok(())
    }

    /// Moves to a branch, which has to exist.
    fn checkout(&mut self, branch: &str) -> Result<(), Error> {
        if !self.branches.contains(branch) {
            return Err(fail(format!(
                "Trying to checkout branch which is not yet created. (Help try using \
                 \"branch {branch}\")"
            )));
        }
        self.current = branch.to_string();
        self.head = self.branches.get(branch).cloned().flatten();
        Ok(())
    }

    /// Merges a branch into the current one.
    fn merge(
        &mut self,
        other: &str,
        custom_id: Option<&str>,
        tags: &[String],
        kind: Option<CommitType>,
    ) -> Result<(), Error> {
        let current_head = self.branches.get(&self.current).cloned().flatten();
        let other_head = self.branches.get(other).cloned().flatten();
        let current_commit = current_head
            .as_deref()
            .and_then(|id| self.commits.get(id))
            .cloned();
        let other_commit = other_head
            .as_deref()
            .and_then(|id| self.commits.get(id))
            .cloned();
        if let (Some(current), Some(_)) = (&current_commit, &other_commit) {
            if current.branch == other {
                return Err(fail(format!("Cannot merge branch '{other}' into itself.")));
            }
        }
        if self.current == other {
            return Err(fail(
                "Incorrect usage of \"merge\". Cannot merge a branch to itself".to_string(),
            ));
        }
        if current_commit.is_none() {
            return Err(fail(format!(
                "Incorrect usage of \"merge\". Current branch ({})has no commits",
                self.current
            )));
        }
        if !self.branches.contains(other) {
            return Err(fail(format!(
                "Incorrect usage of \"merge\". Branch to be merged ({other}) does not exist"
            )));
        }
        if other_commit.is_none() {
            return Err(fail(format!(
                "Incorrect usage of \"merge\". Branch to be merged ({other}) has no commits"
            )));
        }
        if current_head == other_head {
            return Err(fail(
                "Incorrect usage of \"merge\". Both branches have same head".to_string(),
            ));
        }
        if let Some(custom_id) = custom_id {
            if self.commits.contains(custom_id) {
                return Err(fail(format!(
                    "Incorrect usage of \"merge\". Commit with id:{custom_id} already exists, use \
                     different custom id"
                )));
            }
        }
        let parents = match &self.head {
            Some(head) => vec![head.clone(), other_head.unwrap_or_default()],
            None => Vec::new(),
        };
        let id = match custom_id {
            Some(id) => id.to_string(),
            None => self.next_id(),
        };
        self.add(GitCommit {
            id,
            message: format!("merged branch {other} into {}", self.current),
            seq: 0,
            kind: CommitKind::Merge,
            tags: tags.to_vec(),
            parents,
            branch: self.current.clone(),
            custom_type: kind.map(kind_of),
            custom_id: custom_id.is_some(),
        });
        Ok(())
    }

    /// Picks a commit of another branch onto the current one.
    fn cherry_pick(
        &mut self,
        source_id: Option<&str>,
        tags: &[String],
        parent: Option<&str>,
    ) -> Result<(), Error> {
        let source = source_id
            .and_then(|id| self.commits.get(id))
            .cloned()
            .ok_or_else(|| {
                fail(
                    "Incorrect usage of \"cherryPick\". Source commit id should exist and provided"
                        .to_string(),
                )
            })?;
        if let Some(parent) = parent {
            if !source.parents.iter().any(|id| id == parent) {
                return Err(fail(
                    "Invalid operation: The specified parent commit is not an immediate parent of \
                     the cherry-picked commit."
                        .to_string(),
                ));
            }
        }
        if source.kind == CommitKind::Merge && parent.is_none() {
            return Err(fail(
                "Incorrect usage of cherry-pick: If the source commit is a merge commit, an \
                 immediate parent commit must be specified."
                    .to_string(),
            ));
        }
        if source.branch == self.current {
            return Err(fail(
                "Incorrect usage of \"cherryPick\". Source commit is already on current branch"
                    .to_string(),
            ));
        }
        let current_head = self.branches.get(&self.current).cloned().flatten();
        if current_head
            .as_deref()
            .is_none_or(|id| !self.commits.contains(id))
        {
            return Err(fail(format!(
                "Incorrect usage of \"cherry-pick\". Current branch ({})has no commits",
                self.current
            )));
        }
        let parents = match &self.head {
            Some(head) => vec![head.clone(), source.id.clone()],
            None => Vec::new(),
        };
        let tags = if tags.is_empty() {
            let mut tag = format!("cherry-pick:{}", source.id);
            if source.kind == CommitKind::Merge {
                tag.push_str(&format!("|parent:{}", parent.unwrap_or_default()));
            }
            vec![tag]
        } else {
            tags.iter().filter(|tag| !tag.is_empty()).cloned().collect()
        };
        self.add(GitCommit {
            id: self.next_id(),
            message: format!("cherry-picked {} into {}", source.message, self.current),
            seq: 0,
            kind: CommitKind::CherryPick,
            tags,
            parents,
            branch: self.current.clone(),
            custom_type: None,
            custom_id: false,
        });
        Ok(())
    }
}

/// The first seven digits of a number in hexadecimal.
fn hex7(value: u32) -> String {
    format!("{value:08x}")[..7].to_string()
}
