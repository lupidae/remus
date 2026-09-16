//! Mermaid `erDiagram` output.
//!
//! Renders natively on GitHub, GitLab and in Miro (as editable shapes), which
//! makes it the cheapest way to get a picture into a PR or a wiki. It is also
//! the lossiest format: no types beyond a token, no defaults, no indexes. The
//! attribute comment column is used to keep the Postgres facts a generic ERD
//! would drop (enum, domain, identity, generated, nullable).

use std::{collections::HashSet, fmt::Write};

use super::DiagramOptions;
use crate::{
    Schema,
    model::{Column, Constraint, Entity, RelationRef, TypeKind},
};

pub fn render(schema: &Schema, options: &DiagramOptions) -> String {
    let qualify = schema.is_multi_schema();
    let collapsed: HashSet<RelationRef> = if options.conceptual {
        schema
            .entities
            .iter()
            .filter(|e| e.is_junction())
            .map(Entity::reference)
            .collect()
    } else {
        HashSet::new()
    };
    let is_shown =
        |e: &Entity| !collapsed.contains(&e.reference()) && (options.views || !e.kind.is_view());
    let shown = |target: &RelationRef| schema.entity(target).filter(|e| is_shown(e));

    let mut out = String::from("erDiagram\n");

    for entity in schema.entities.iter().filter(|e| is_shown(e)) {
        let name = node_name(entity, qualify);
        if !options.attributes || entity.columns.is_empty() {
            let _ = writeln!(out, "    {name}");
            continue;
        }
        let _ = writeln!(out, "    {name} {{");
        for column in &entity.columns {
            let _ = write!(
                out,
                "        {} {}{}",
                type_token(column),
                token(&column.name),
                key_markers(entity, column)
            );
            match attribute_note(schema, column) {
                Some(note) => {
                    let _ = writeln!(out, " \"{note}\"");
                }
                None => out.push('\n'),
            }
        }
        let _ = writeln!(out, "    }}");
    }

    for entity in &schema.entities {
        if collapsed.contains(&entity.reference()) {
            write_collapsed_junction(&mut out, entity, &shown, qualify);
            continue;
        }
        if !is_shown(entity) {
            continue;
        }
        for fk in entity.foreign_keys() {
            let Some(parent) = fk.target().and_then(|t| shown(&t)) else {
                continue; // outside the selected schemas, or a collapsed junction
            };
            let (parent_side, child_side) = cardinality(entity, fk);
            let label = fk.comment.clone().unwrap_or_else(|| fk.columns.join(", "));
            let _ = writeln!(
                out,
                "    {} {parent_side}--{child_side} {} : \"{}\"",
                node_name(parent, qualify),
                node_name(entity, qualify),
                escape(&label)
            );
        }
        if entity.kind.is_view() {
            for source in entity.depends_on.iter().filter_map(&shown) {
                let _ = writeln!(
                    out,
                    "    {} ||..|| {} : \"reads\"",
                    node_name(source, qualify),
                    node_name(entity, qualify)
                );
            }
        }
    }

    out
}

/// Crow's foot ends for `parent <-> child`, from what the catalog can prove.
///
/// Parent end: a foreign key targets a unique key, so a child row has at most
/// one parent; it has at least one only when every FK column is NOT NULL.
/// Child end: a parent has at most one child only when the FK columns are
/// themselves unique. Nothing in the catalog says a parent must have a child,
/// so the child end is always "zero or".
fn cardinality(child: &Entity, fk: &Constraint) -> (&'static str, &'static str) {
    let mandatory = fk
        .columns
        .iter()
        .all(|c| child.column(c).is_some_and(|col| col.not_null));
    let parent_end = if mandatory { "||" } else { "|o" };
    let child_end = if child.is_unique_set(&fk.columns) {
        "o|"
    } else {
        "o{"
    };
    (parent_end, child_end)
}

/// One N:N edge in place of the hidden junction table, labelled with its name so
/// the physical table stays discoverable.
fn write_collapsed_junction<'a>(
    out: &mut String,
    junction: &Entity,
    shown: &impl Fn(&RelationRef) -> Option<&'a Entity>,
    qualify: bool,
) {
    let targets: Vec<_> = junction
        .foreign_keys()
        .filter_map(|fk| fk.target())
        .collect();
    let [left, right] = targets.as_slice() else {
        return;
    };
    let (Some(left), Some(right)) = (shown(left), shown(right)) else {
        return;
    };
    let _ = writeln!(
        out,
        "    {} }}o--o{{ {} : \"{}\"",
        node_name(left, qualify),
        node_name(right, qualify),
        escape(&junction.name)
    );
}

/// Mermaid node names allow word characters only, so `shop.orders` becomes
/// `shop_orders`; single-schema diagrams keep the short name.
fn node_name(entity: &Entity, qualify: bool) -> String {
    if qualify {
        token(&format!("{}_{}", entity.schema, entity.name))
    } else {
        token(&entity.name)
    }
}

/// Attribute types and names: `[A-Za-z_][A-Za-z0-9_()\[\]]*` in Mermaid's lexer.
fn token(raw: &str) -> String {
    let mut out: String = raw
        .chars()
        .map(|c| match c {
            c if c.is_ascii_alphanumeric() || matches!(c, '_' | '(' | ')' | '[' | ']') => c,
            _ => '_',
        })
        .collect();
    if out
        .chars()
        .next()
        .is_none_or(|c| !(c.is_ascii_alphabetic() || c == '_'))
    {
        out.insert(0, '_');
    }
    out
}

/// User-defined types read better by their bare name than by
/// `character varying(255)`-style expansions, and a diagram is not the place
/// for schema qualification of types.
fn type_token(column: &Column) -> String {
    let base = match column.type_kind {
        TypeKind::Enum | TypeKind::Domain | TypeKind::Composite => column.type_name.clone(),
        TypeKind::Base | TypeKind::Range | TypeKind::Multirange | TypeKind::Pseudo => {
            column.r#type.trim_end_matches("[]").to_owned()
        }
    };
    let suffix = if column.is_array { "[]" } else { "" };
    token(&format!("{base}{suffix}"))
}

fn key_markers(entity: &Entity, column: &Column) -> String {
    let mut markers = Vec::new();
    if entity.is_primary_key_column(&column.name) {
        markers.push("PK");
    }
    if entity.is_foreign_key_column(&column.name) {
        markers.push("FK");
    }
    if entity.has_single_unique(&column.name) {
        markers.push("UK");
    }
    if markers.is_empty() {
        String::new()
    } else {
        format!(" {}", markers.join(","))
    }
}

/// The column comment when there is one, otherwise the Postgres facts that
/// have nowhere else to go in Mermaid.
fn attribute_note(schema: &Schema, column: &Column) -> Option<String> {
    if let Some(comment) = &column.comment {
        return Some(escape(comment));
    }
    let mut facts = Vec::new();
    match column.type_kind {
        TypeKind::Enum => facts.push("enum".to_owned()),
        TypeKind::Domain => facts.push(match schema.domain_of(column) {
            Some(domain) => format!("domain over {}", domain.base_type),
            None => "domain".to_owned(),
        }),
        TypeKind::Composite => facts.push("composite".to_owned()),
        TypeKind::Base | TypeKind::Range | TypeKind::Multirange | TypeKind::Pseudo => {}
    }
    if column.identity.is_some() {
        facts.push("identity".to_owned());
    }
    if column.generated.is_some() {
        facts.push("generated".to_owned());
    }
    if !column.not_null {
        facts.push("nullable".to_owned());
    }
    if facts.is_empty() {
        None
    } else {
        Some(escape(&facts.join(", ")))
    }
}

/// Quoted Mermaid strings cannot contain double quotes or newlines.
fn escape(text: &str) -> String {
    text.replace('"', "'").replace(['\n', '\r'], " ")
}

#[cfg(test)]
mod tests {
    use super::{render, token};
    use crate::{Schema, emit::DiagramOptions};

    const SHOWCASE: &str = include_str!("../../tests/fixtures/showcase.json");

    #[test]
    fn tokens_are_mermaid_safe() {
        assert_eq!(token("zip code"), "zip_code");
        assert_eq!(token("character varying(255)"), "character_varying(255)");
        assert_eq!(token("numeric(10,2)"), "numeric(10_2)");
        assert_eq!(token("1st"), "_1st");
    }

    #[test]
    fn conceptual_mode_replaces_the_junction_with_one_edge() {
        let schema = Schema::from_json(SHOWCASE).unwrap();
        let physical = render(&schema, &DiagramOptions::default());
        let conceptual = render(
            &schema,
            &DiagramOptions {
                conceptual: true,
                ..DiagramOptions::default()
            },
        );
        assert!(physical.contains("shop_order_products {"));
        assert!(!conceptual.contains("shop_order_products {"));
        assert!(conceptual.contains("shop_orders }o--o{ shop_products : \"order_products\""));
    }

    #[test]
    fn cardinality_follows_uniqueness_and_nullability() {
        let schema = Schema::from_json(SHOWCASE).unwrap();
        let out = render(&schema, &DiagramOptions::default());
        // customer_profiles.customer_id is the PK: a customer has zero or one profile.
        assert!(out.contains("shop_customers ||--o| shop_customer_profiles"));
        // orders.customer_id is NOT NULL: every order has exactly one customer.
        assert!(out.contains("shop_customers ||--o{ shop_orders"));
        // customers.referrer_id is nullable: a customer has zero or one referrer.
        assert!(out.contains("shop_customers |o--o{ shop_customers : \"referrer_id\""));
    }

    #[test]
    fn views_are_opt_in() {
        let schema = Schema::from_json(SHOWCASE).unwrap();
        let without = render(&schema, &DiagramOptions::default());
        let with = render(
            &schema,
            &DiagramOptions {
                views: true,
                ..DiagramOptions::default()
            },
        );
        assert!(!without.contains("open_orders"));
        assert!(with.contains("shop_open_orders ||..|| shop_open_orders_by_customer : \"reads\""));
    }
}
