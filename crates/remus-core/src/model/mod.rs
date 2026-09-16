//! The schema model: what `introspect.sql` returns, as Rust types.
//!
//! Field names and the single-letter codes mirror `pg_catalog` on purpose, so a
//! payload produced by running the SQL in psql deserialises with no translation
//! layer in between. Everything here is physical (an MPD in Merise terms); the
//! conceptual reading lives in [`conceptual`].

mod conceptual;

use serde::{Deserialize, Serialize};

use crate::Error;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Schema {
    pub remus_version: String,
    /// Dropped by [`Schema::canonical`]: two snapshots of an unchanged database
    /// must compare equal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generated_at: Option<String>,
    pub database: String,
    pub server_version: String,
    #[serde(default)]
    pub entities: Vec<Entity>,
    #[serde(default)]
    pub enums: Vec<EnumType>,
    #[serde(default)]
    pub domains: Vec<DomainType>,
    #[serde(default)]
    pub composites: Vec<CompositeType>,
    #[serde(default)]
    pub partitions: Vec<Partition>,
    #[serde(default)]
    pub extensions: Vec<Extension>,
}

/// `pg_class.relkind`, restricted to what a schema diagram shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntityKind {
    #[serde(rename = "r")]
    Table,
    #[serde(rename = "p")]
    PartitionedTable,
    #[serde(rename = "v")]
    View,
    #[serde(rename = "m")]
    MaterializedView,
    #[serde(rename = "f")]
    ForeignTable,
}

impl EntityKind {
    pub fn is_table(&self) -> bool {
        matches!(self, Self::Table | Self::PartitionedTable)
    }

    pub fn is_view(&self) -> bool {
        matches!(self, Self::View | Self::MaterializedView)
    }
}

/// `pg_constraint.contype`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConstraintKind {
    #[serde(rename = "p")]
    PrimaryKey,
    #[serde(rename = "u")]
    Unique,
    #[serde(rename = "f")]
    ForeignKey,
    #[serde(rename = "c")]
    Check,
    #[serde(rename = "x")]
    Exclusion,
}

/// `pg_type.typtype` of a column's type (of the element type for arrays).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TypeKind {
    #[serde(rename = "b")]
    Base,
    #[serde(rename = "e")]
    Enum,
    #[serde(rename = "d")]
    Domain,
    #[serde(rename = "c")]
    Composite,
    #[serde(rename = "r")]
    Range,
    #[serde(rename = "m")]
    Multirange,
    #[serde(rename = "p")]
    Pseudo,
}

/// `pg_attribute.attidentity`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Identity {
    #[serde(rename = "a")]
    Always,
    #[serde(rename = "d")]
    ByDefault,
}

/// `pg_attribute.attgenerated`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Generated {
    #[serde(rename = "s")]
    Stored,
    /// PostgreSQL 18+.
    #[serde(rename = "v")]
    Virtual,
}

/// `pg_constraint.confupdtype` / `confdeltype`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReferentialAction {
    #[serde(rename = "a")]
    NoAction,
    #[serde(rename = "r")]
    Restrict,
    #[serde(rename = "c")]
    Cascade,
    #[serde(rename = "n")]
    SetNull,
    #[serde(rename = "d")]
    SetDefault,
}

/// `pg_policy.polcmd`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PolicyCommand {
    #[serde(rename = "*")]
    All,
    #[serde(rename = "r")]
    Select,
    #[serde(rename = "a")]
    Insert,
    #[serde(rename = "w")]
    Update,
    #[serde(rename = "d")]
    Delete,
}

/// A schema-qualified relation name: the stable identity of an entity across
/// environments (OIDs are not, see CLAUDE.md).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct RelationRef {
    pub schema: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entity {
    pub schema: String,
    pub name: String,
    pub kind: EntityKind,
    pub comment: Option<String>,
    #[serde(default)]
    pub rls: bool,
    #[serde(default)]
    pub rls_forced: bool,
    #[serde(default)]
    pub unlogged: bool,
    pub partition_key: Option<String>,
    pub view_definition: Option<String>,
    /// Relations a view reads from. Empty for tables.
    #[serde(default)]
    pub depends_on: Vec<RelationRef>,
    #[serde(default)]
    pub columns: Vec<Column>,
    #[serde(default)]
    pub constraints: Vec<Constraint>,
    #[serde(default)]
    pub indexes: Vec<Index>,
    #[serde(default)]
    pub policies: Vec<Policy>,
}

/// Columns are listed in table order; `attnum` itself is not carried because it
/// keeps gaps for dropped columns and would differ after a dump/restore.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Column {
    pub name: String,
    /// As `format_type` prints it: `character varying(255)`, `order_status`, `text[]`.
    pub r#type: String,
    pub type_schema: String,
    pub type_name: String,
    pub type_kind: TypeKind,
    #[serde(default)]
    pub is_array: bool,
    #[serde(default)]
    pub not_null: bool,
    /// The default expression, or the generation expression for generated columns
    /// (PostgreSQL stores both in `pg_attrdef`).
    pub default: Option<String>,
    pub identity: Option<Identity>,
    pub generated: Option<Generated>,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Constraint {
    pub name: String,
    pub kind: ConstraintKind,
    /// Constrained columns in constraint order. Empty for CHECK constraints.
    #[serde(default)]
    pub columns: Vec<String>,
    pub ref_schema: Option<String>,
    pub ref_table: Option<String>,
    /// Referenced columns, positionally paired with `columns`.
    #[serde(default)]
    pub ref_columns: Vec<String>,
    pub on_update: Option<ReferentialAction>,
    pub on_delete: Option<ReferentialAction>,
    #[serde(default)]
    pub deferrable: bool,
    /// `pg_get_constraintdef`: authoritative, used verbatim when emitting DDL.
    pub definition: String,
    pub comment: Option<String>,
}

impl Constraint {
    /// The referenced relation of a foreign key.
    pub fn target(&self) -> Option<RelationRef> {
        match (&self.ref_schema, &self.ref_table) {
            (Some(schema), Some(name)) => Some(RelationRef {
                schema: schema.clone(),
                name: name.clone(),
            }),
            (None, _) | (_, None) => None,
        }
    }
}

/// A secondary index. Indexes backing PK / UNIQUE / EXCLUDE constraints are not
/// listed; the constraint describes them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Index {
    pub name: String,
    #[serde(default)]
    pub unique: bool,
    #[serde(default)]
    pub partial: bool,
    /// `pg_get_indexdef`: a complete `CREATE INDEX` statement.
    pub definition: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Policy {
    pub name: String,
    pub command: PolicyCommand,
    #[serde(default)]
    pub permissive: bool,
    /// Empty means PUBLIC.
    #[serde(default)]
    pub roles: Vec<String>,
    pub using: Option<String>,
    pub check: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnumType {
    pub schema: String,
    pub name: String,
    #[serde(default)]
    pub labels: Vec<String>,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DomainType {
    pub schema: String,
    pub name: String,
    pub base_type: String,
    #[serde(default)]
    pub not_null: bool,
    pub default: Option<String>,
    /// `pg_get_constraintdef` of each CHECK, e.g. `CHECK ((VALUE > 0))`.
    #[serde(default)]
    pub constraints: Vec<String>,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompositeType {
    pub schema: String,
    pub name: String,
    #[serde(default)]
    pub attributes: Vec<CompositeAttribute>,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompositeAttribute {
    pub name: String,
    pub r#type: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Partition {
    pub parent_schema: String,
    pub parent: String,
    pub child_schema: String,
    pub child: String,
    /// `FOR VALUES ...` or `DEFAULT`.
    pub bound: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Extension {
    pub name: String,
    pub schema: String,
    pub version: String,
}

impl Schema {
    pub fn from_json(json: &str) -> Result<Self, Error> {
        Ok(serde_json::from_str(json)?)
    }

    pub fn entity(&self, target: &RelationRef) -> Option<&Entity> {
        self.entities
            .iter()
            .find(|e| e.schema == target.schema && e.name == target.name)
    }

    /// The domain a column's type refers to, when it is one.
    pub fn domain_of(&self, column: &Column) -> Option<&DomainType> {
        (column.type_kind == TypeKind::Domain).then(|| {
            self.domains
                .iter()
                .find(|d| d.schema == column.type_schema && d.name == column.type_name)
        })?
    }

    /// Keep only the given schemas. An empty list keeps everything.
    pub fn retain_schemas(&mut self, keep: &[String]) {
        if keep.is_empty() {
            return;
        }
        self.entities.retain(|e| keep.contains(&e.schema));
        self.enums.retain(|e| keep.contains(&e.schema));
        self.domains.retain(|d| keep.contains(&d.schema));
        self.composites.retain(|c| keep.contains(&c.schema));
        self.partitions.retain(|p| keep.contains(&p.parent_schema));
    }

    /// Diagram labels need qualifying only when names could collide.
    pub fn is_multi_schema(&self) -> bool {
        let first = self.entities.first().map(|e| e.schema.as_str());
        self.entities
            .iter()
            .any(|e| Some(e.schema.as_str()) != first)
    }

    /// Strips everything that changes between two runs against an identical
    /// database, so snapshots can be compared or content-addressed.
    pub fn canonical(mut self) -> Self {
        self.generated_at = None;
        self
    }
}

impl Entity {
    pub fn reference(&self) -> RelationRef {
        RelationRef {
            schema: self.schema.clone(),
            name: self.name.clone(),
        }
    }

    pub fn qualified_name(&self) -> String {
        format!("{}.{}", self.schema, self.name)
    }

    pub fn primary_key(&self) -> Option<&Constraint> {
        self.constraints
            .iter()
            .find(|c| c.kind == ConstraintKind::PrimaryKey)
    }

    pub fn foreign_keys(&self) -> impl Iterator<Item = &Constraint> {
        self.constraints
            .iter()
            .filter(|c| c.kind == ConstraintKind::ForeignKey)
    }

    pub fn column(&self, name: &str) -> Option<&Column> {
        self.columns.iter().find(|c| c.name == name)
    }

    pub fn is_primary_key_column(&self, name: &str) -> bool {
        self.primary_key()
            .is_some_and(|pk| pk.columns.iter().any(|c| c == name))
    }

    pub fn is_foreign_key_column(&self, name: &str) -> bool {
        self.foreign_keys()
            .any(|fk| fk.columns.iter().any(|c| c == name))
    }

    /// A single-column UNIQUE constraint on `name`. Multi-column uniqueness is
    /// a property of the set, not of one column, and is reported separately.
    pub fn has_single_unique(&self, name: &str) -> bool {
        self.constraints
            .iter()
            .any(|c| c.kind == ConstraintKind::Unique && c.columns == [name])
    }

    /// True when `columns` are exactly covered by a PK or UNIQUE constraint,
    /// i.e. at most one row can hold a given combination. This is what turns an
    /// FK into a 1:1 rather than a 1:N on the diagram.
    pub fn is_unique_set(&self, columns: &[String]) -> bool {
        self.constraints
            .iter()
            .filter(|c| matches!(c.kind, ConstraintKind::PrimaryKey | ConstraintKind::Unique))
            .any(|c| {
                c.columns.len() == columns.len() && columns.iter().all(|x| c.columns.contains(x))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::{ConstraintKind, Schema};

    const MINIMAL: &str = r#"{
        "remus_version": "0.1",
        "generated_at": "2026-01-01T00:00:00+00:00",
        "database": "db",
        "server_version": "16.0",
        "entities": [{
            "schema": "public", "name": "t", "kind": "r", "comment": null,
            "rls": false, "rls_forced": false, "unlogged": false,
            "partition_key": null, "view_definition": null,
            "columns": [{
                "name": "id", "type": "bigint",
                "type_schema": "pg_catalog", "type_name": "int8", "type_kind": "b",
                "is_array": false, "not_null": true, "default": null,
                "identity": "a", "generated": null, "comment": null
            }],
            "constraints": [{
                "name": "t_pkey", "kind": "p", "columns": ["id"],
                "ref_schema": null, "ref_table": null, "ref_columns": [],
                "on_update": null, "on_delete": null, "deferrable": false,
                "definition": "PRIMARY KEY (id)", "comment": null
            }]
        }]
    }"#;

    #[test]
    fn parses_catalog_codes_into_enums() {
        let schema = Schema::from_json(MINIMAL).unwrap();
        let entity = &schema.entities[0];
        assert!(entity.kind.is_table());
        assert_eq!(entity.constraints[0].kind, ConstraintKind::PrimaryKey);
        assert!(entity.is_primary_key_column("id"));
        assert!(entity.is_unique_set(&["id".to_owned()]));
        assert!(!entity.is_unique_set(&[]));
    }

    #[test]
    fn missing_collections_default_to_empty() {
        let schema = Schema::from_json(MINIMAL).unwrap();
        assert!(schema.enums.is_empty());
        assert!(schema.entities[0].indexes.is_empty());
        assert!(schema.entities[0].depends_on.is_empty());
    }

    #[test]
    fn canonical_drops_the_timestamp_only() {
        let schema = Schema::from_json(MINIMAL).unwrap();
        let canonical = schema.clone().canonical();
        assert!(canonical.generated_at.is_none());
        assert_eq!(canonical.entities, schema.entities);
    }

    #[test]
    fn rejects_unknown_codes() {
        let broken = MINIMAL.replace(r#""kind": "r""#, r#""kind": "z""#);
        assert!(Schema::from_json(&broken).is_err());
    }
}
