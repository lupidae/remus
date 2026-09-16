//! SQL identifier and literal quoting, matching `quote_ident` / `quote_literal`.
//!
//! Quoting only when needed keeps the DDL readable, which matters because the
//! SQL output is meant to be eyeballed in a diff as much as executed.

use std::borrow::Cow;

/// Words PostgreSQL reserves outright (the "reserved" column of the keyword
/// appendix). Non-reserved keywords such as `name` or `type` are legal bare
/// identifiers and do not need quoting.
const RESERVED: &[&str] = &[
    "all",
    "analyse",
    "analyze",
    "and",
    "any",
    "array",
    "as",
    "asc",
    "asymmetric",
    "authorization",
    "binary",
    "both",
    "case",
    "cast",
    "check",
    "collate",
    "collation",
    "column",
    "concurrently",
    "constraint",
    "create",
    "cross",
    "current_catalog",
    "current_date",
    "current_role",
    "current_schema",
    "current_time",
    "current_timestamp",
    "current_user",
    "default",
    "deferrable",
    "desc",
    "distinct",
    "do",
    "else",
    "end",
    "except",
    "false",
    "fetch",
    "for",
    "foreign",
    "freeze",
    "from",
    "full",
    "grant",
    "group",
    "having",
    "ilike",
    "in",
    "initially",
    "inner",
    "intersect",
    "into",
    "is",
    "isnull",
    "join",
    "lateral",
    "leading",
    "left",
    "like",
    "limit",
    "localtime",
    "localtimestamp",
    "natural",
    "not",
    "notnull",
    "null",
    "offset",
    "on",
    "only",
    "or",
    "order",
    "outer",
    "overlaps",
    "placing",
    "primary",
    "references",
    "returning",
    "right",
    "select",
    "session_user",
    "similar",
    "some",
    "symmetric",
    "system_user",
    "table",
    "tablesample",
    "then",
    "to",
    "trailing",
    "true",
    "union",
    "unique",
    "user",
    "using",
    "variadic",
    "verbose",
    "when",
    "where",
    "window",
    "with",
];

fn is_safe_bare(name: &str) -> bool {
    let mut chars = name.chars();
    let starts_well = chars
        .next()
        .is_some_and(|c| c.is_ascii_lowercase() || c == '_');
    starts_well
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        && !RESERVED.contains(&name)
}

pub fn quote_ident(name: &str) -> Cow<'_, str> {
    if is_safe_bare(name) {
        Cow::Borrowed(name)
    } else {
        Cow::Owned(format!("\"{}\"", name.replace('"', "\"\"")))
    }
}

pub fn qualified(schema: &str, name: &str) -> String {
    format!("{}.{}", quote_ident(schema), quote_ident(name))
}

pub fn quote_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

#[cfg(test)]
mod tests {
    use super::{qualified, quote_ident, quote_literal};

    #[test]
    fn plain_lowercase_names_stay_bare() {
        assert_eq!(quote_ident("orders"), "orders");
        assert_eq!(quote_ident("order_items_2"), "order_items_2");
        assert_eq!(qualified("shop", "orders"), "shop.orders");
    }

    #[test]
    fn reserved_words_mixed_case_and_symbols_are_quoted() {
        assert_eq!(quote_ident("select"), "\"select\"");
        assert_eq!(quote_ident("Orders"), "\"Orders\"");
        assert_eq!(quote_ident("zip code"), "\"zip code\"");
        assert_eq!(quote_ident("1st"), "\"1st\"");
        assert_eq!(quote_ident("say \"hi\""), "\"say \"\"hi\"\"\"");
    }

    #[test]
    fn non_reserved_keywords_stay_bare() {
        assert_eq!(quote_ident("name"), "name");
        assert_eq!(quote_ident("type"), "type");
    }

    #[test]
    fn literals_double_single_quotes() {
        assert_eq!(quote_literal("it's"), "'it''s'");
    }
}
