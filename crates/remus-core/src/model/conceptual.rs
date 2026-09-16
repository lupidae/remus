//! The one heuristic in the codebase: recognising pure association tables.
//!
//! The catalog knows tables and foreign keys; it does not know that `order_items`
//! is *the* N:N between orders and products. Collapsing such tables is what turns
//! a physical diagram into something closer to a conceptual model, and it is the
//! only inference remus makes. It stays deliberately conservative because a
//! confidently wrong conceptual diagram is worse than none.

use super::Entity;

/// Columns that carry no domain meaning, so their presence does not stop a table
/// from being a pure association.
const HOUSEKEEPING: &[&str] = &[
    "id",
    "created_at",
    "updated_at",
    "inserted_at",
    "deleted_at",
    "created_by",
    "updated_by",
];

impl Entity {
    /// A join table: exactly two foreign keys, and every column is either part
    /// of one of them, generated, or housekeeping. Polymorphic associations
    /// (`owner_type` / `owner_id`) are never inferred; they need an annotation.
    pub fn is_junction(&self) -> bool {
        if !self.kind.is_table() {
            return false;
        }
        let foreign_keys: Vec<_> = self.foreign_keys().collect();
        let [first, second] = foreign_keys.as_slice() else {
            return false;
        };
        let is_fk_column = |name: &str| {
            first.columns.iter().any(|c| c == name) || second.columns.iter().any(|c| c == name)
        };
        self.columns.iter().all(|col| {
            is_fk_column(&col.name)
                || col.generated.is_some()
                || HOUSEKEEPING.contains(&col.name.as_str())
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::model::{Column, Constraint, ConstraintKind, Entity, EntityKind, TypeKind};

    fn column(name: &str) -> Column {
        Column {
            name: name.to_owned(),
            r#type: "bigint".to_owned(),
            type_schema: "pg_catalog".to_owned(),
            type_name: "int8".to_owned(),
            type_kind: TypeKind::Base,
            is_array: false,
            not_null: true,
            default: None,
            identity: None,
            generated: None,
            comment: None,
        }
    }

    fn foreign_key(name: &str, column: &str, table: &str) -> Constraint {
        Constraint {
            name: name.to_owned(),
            kind: ConstraintKind::ForeignKey,
            columns: vec![column.to_owned()],
            ref_schema: Some("public".to_owned()),
            ref_table: Some(table.to_owned()),
            ref_columns: vec!["id".to_owned()],
            on_update: None,
            on_delete: None,
            deferrable: false,
            definition: String::new(),
            comment: None,
        }
    }

    fn table(columns: &[&str], constraints: Vec<Constraint>) -> Entity {
        Entity {
            schema: "public".to_owned(),
            name: "order_items".to_owned(),
            kind: EntityKind::Table,
            comment: None,
            rls: false,
            rls_forced: false,
            unlogged: false,
            partition_key: None,
            view_definition: None,
            depends_on: vec![],
            columns: columns.iter().map(|c| column(c)).collect(),
            constraints,
            indexes: vec![],
            policies: vec![],
        }
    }

    fn two_foreign_keys() -> Vec<Constraint> {
        vec![
            foreign_key("fk_order", "order_id", "orders"),
            foreign_key("fk_product", "product_id", "products"),
        ]
    }

    #[test]
    fn two_fks_and_housekeeping_is_a_junction() {
        let entity = table(
            &["order_id", "product_id", "created_at"],
            two_foreign_keys(),
        );
        assert!(entity.is_junction());
    }

    #[test]
    fn an_extra_attribute_makes_it_an_entity() {
        let entity = table(&["order_id", "product_id", "quantity"], two_foreign_keys());
        assert!(!entity.is_junction());
    }

    #[test]
    fn one_or_three_fks_is_not_a_junction() {
        let one = table(
            &["order_id"],
            vec![foreign_key("fk_order", "order_id", "orders")],
        );
        assert!(!one.is_junction());

        let mut three = two_foreign_keys();
        three.push(foreign_key("fk_user", "user_id", "users"));
        let entity = table(&["order_id", "product_id", "user_id"], three);
        assert!(!entity.is_junction());
    }

    #[test]
    fn views_are_never_junctions() {
        let mut entity = table(&["order_id", "product_id"], two_foreign_keys());
        entity.kind = EntityKind::View;
        assert!(!entity.is_junction());
    }
}
