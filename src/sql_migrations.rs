use crate::project::{
    Artifact, ArtifactId, CoverageDiagnostic, CoverageScope, FactCoverage, FactId, ProjectFact,
    Provenance, ProvenanceKind, WorldAssumption,
};
use crate::{Error, SourceSpan};
use pg_query::NodeEnum;
use pg_query::protobuf::{
    AlterTableCmd, AlterTableStmt, AlterTableType, ColumnDef, ConstrType, Constraint, CreateStmt,
    CreateTableAsStmt, DoStmt, DropStmt, ObjectType, RangeVar,
};
use serde::Serialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

const RELATIONS: &[&str] = &[
    "schema",
    "table",
    "column",
    "column_type",
    "column_not_null",
    "primary_key",
    "unique_constraint",
    "foreign_key",
    "check_constraint",
    "partition_of",
    "inherits",
    "materialized_view",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SchemaEffect {
    KnownIrrelevantToSchema,
    KnownSchemaEffect {
        scope: CoverageScope,
        relations: Vec<String>,
    },
    UnknownSchemaEffect,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UnsupportedSql {
    pub source: PathBuf,
    pub line: usize,
    pub statement: String,
    pub reason: String,
    pub effect: SchemaEffect,
}

#[derive(Debug, Clone)]
pub struct SqlMigrationFacts {
    pub artifacts: Vec<Artifact>,
    pub facts: Vec<ProjectFact>,
    pub input_files: Vec<PathBuf>,
    pub coverage: Vec<FactCoverage>,
    pub unsupported: Vec<UnsupportedSql>,
    pub migration_count: usize,
}

#[derive(Debug, Clone)]
pub struct PostgresMigrationFactProvider {
    pub project_root: PathBuf,
    pub migration_root: PathBuf,
    pub default_schema: String,
}

#[derive(Debug, Clone, Default)]
struct EffectiveDatabaseSchema {
    schemas: BTreeMap<String, Provenance>,
    tables: BTreeMap<String, TableState>,
    materialized_views: BTreeMap<String, Provenance>,
    partitions: BTreeMap<String, (String, Provenance, String)>,
    inheritance: BTreeMap<String, Vec<(String, Provenance)>>,
}

#[derive(Debug, Clone)]
struct TableState {
    provenance: Provenance,
    columns: BTreeMap<String, ColumnState>,
    constraints: BTreeMap<String, ConstraintState>,
}

#[derive(Debug, Clone)]
struct CoverageGap {
    relations: Vec<String>,
    scope: CoverageScope,
    diagnostic: CoverageDiagnostic,
}

#[derive(Debug, Clone)]
struct ColumnState {
    type_name: String,
    provenance: Provenance,
    explicit_not_null: Option<Provenance>,
}

struct AlterContext<'a> {
    table_name: &'a str,
    sql: &'a str,
    source: &'a Path,
    line: usize,
}

#[derive(Debug, Clone)]
enum ConstraintState {
    Primary {
        columns: Vec<String>,
        provenance: Provenance,
    },
    Unique {
        columns: Vec<String>,
        provenance: Provenance,
    },
    Foreign {
        columns: Vec<String>,
        target_table: String,
        target_columns: Vec<String>,
        provenance: Provenance,
    },
    Check {
        expression: String,
        provenance: Provenance,
    },
}

impl ConstraintState {
    fn provenance(&self) -> &Provenance {
        match self {
            Self::Primary { provenance, .. }
            | Self::Unique { provenance, .. }
            | Self::Foreign { provenance, .. }
            | Self::Check { provenance, .. } => provenance,
        }
    }
    fn uses_column(&self, column: &str) -> bool {
        match self {
            Self::Primary { columns, .. }
            | Self::Unique { columns, .. }
            | Self::Foreign { columns, .. } => columns.iter().any(|item| item == column),
            Self::Check { .. } => false,
        }
    }
}

impl PostgresMigrationFactProvider {
    pub fn discover(project_root: &Path) -> Option<Self> {
        let migration_root = project_root.join("migrations");
        migration_root.is_dir().then(|| Self {
            project_root: project_root.to_path_buf(),
            migration_root,
            default_schema: "public".into(),
        })
    }

    pub fn extract(&self) -> Result<SqlMigrationFacts, Error> {
        let migrations = self.ordered_migrations()?;
        let mut schema = EffectiveDatabaseSchema::default();
        let mut unsupported = Vec::new();
        let mut gaps = Vec::new();
        let mut artifacts = Vec::new();
        let mut inputs = Vec::new();
        for (_, path) in &migrations {
            let relative = path
                .strip_prefix(&self.project_root)
                .unwrap_or(path)
                .to_path_buf();
            let base = Provenance {
                kind: ProvenanceKind::DeterministicallyExtracted,
                source: relative.clone(),
                span: Some(SourceSpan {
                    filename: relative.clone(),
                    line: 1,
                    column: 1,
                }),
                extractor: Some("pg_query 6.2.0 / PostgreSQL parser".into()),
            };
            artifacts.push(Artifact {
                id: ArtifactId(relative.display().to_string()),
                kind: "postgres_migration".into(),
                provenance: base,
            });
            inputs.push(path.clone());
            self.apply_file(path, &mut schema, &mut unsupported, &mut gaps)?;
        }
        for item in &unsupported {
            let diagnostic = CoverageDiagnostic {
                reason: item.reason.clone(),
                provenance: provenance(&item.source, item.line),
            };
            match &item.effect {
                SchemaEffect::KnownIrrelevantToSchema => {}
                SchemaEffect::KnownSchemaEffect { scope, relations } => gaps.push(CoverageGap {
                    relations: relations.clone(),
                    scope: scope.clone(),
                    diagnostic,
                }),
                SchemaEffect::UnknownSchemaEffect => gaps.push(CoverageGap {
                    relations: RELATIONS.iter().map(|value| (*value).into()).collect(),
                    scope: CoverageScope::Global,
                    diagnostic,
                }),
            }
        }
        let qualifiers = BTreeMap::from([
            ("dialect".into(), "postgresql".into()),
            ("default_schema".into(), self.default_schema.clone()),
            ("stream".into(), "forward_sqlx_migrations".into()),
        ]);
        let mut coverage = RELATIONS
            .iter()
            .map(|relation| FactCoverage {
                relation: (*relation).into(),
                provider: "postgres_migrations".into(),
                world: WorldAssumption::Closed,
                scope: CoverageScope::Global,
                qualifiers: qualifiers.clone(),
                statement: "the relation is complete except in explicit uncertainty scopes".into(),
                diagnostics: Vec::new(),
            })
            .collect::<Vec<_>>();
        for gap in gaps {
            for relation in gap.relations {
                coverage.push(FactCoverage {
                    relation,
                    provider: "postgres_migrations".into(),
                    world: WorldAssumption::Partial,
                    scope: gap.scope.clone(),
                    qualifiers: qualifiers.clone(),
                    statement: "the relation is incomplete in this scoped uncertainty region"
                        .into(),
                    diagnostics: vec![gap.diagnostic.clone()],
                });
            }
        }
        coverage.sort_by_key(|item| serde_json::to_string(item).unwrap());
        let mut facts = self.emit_facts(&schema, &migrations);
        for (index, item) in unsupported.iter().enumerate() {
            facts.push(fact(
                format!(
                    "sql:unsupported:{index}:{}:{}",
                    item.source.display(),
                    item.line
                ),
                "unsupported_sql",
                vec![item.statement.clone()],
                provenance(&item.source, item.line),
                BTreeMap::from([
                    ("reason".into(), item.reason.clone()),
                    ("schema_effect".into(), effect_name(&item.effect).into()),
                ]),
            ));
        }
        facts.sort_by(|a, b| a.id.cmp(&b.id));
        artifacts.sort_by(|a, b| a.id.cmp(&b.id));
        inputs.sort();
        unsupported.sort_by(|a, b| {
            (&a.source, a.line, &a.statement).cmp(&(&b.source, b.line, &b.statement))
        });
        Ok(SqlMigrationFacts {
            artifacts,
            facts,
            input_files: inputs,
            coverage,
            unsupported,
            migration_count: migrations.len(),
        })
    }

    fn ordered_migrations(&self) -> Result<Vec<(u64, PathBuf)>, Error> {
        let mut migrations = Vec::new();
        for entry in fs::read_dir(&self.migration_root).map_err(|source| Error::Io {
            path: self.migration_root.clone(),
            source,
        })? {
            let path = entry
                .map_err(|source| Error::Io {
                    path: self.migration_root.clone(),
                    source,
                })?
                .path();
            let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
                continue;
            };
            if !name.ends_with(".sql") || name.ends_with(".down.sql") {
                continue;
            }
            let prefix = name
                .split_once('_')
                .map(|(value, _)| value)
                .ok_or_else(|| {
                    Error::ProviderFailure(format!(
                        "migration `{}` has no numeric version prefix",
                        path.display()
                    ))
                })?;
            let version = prefix.parse::<u64>().map_err(|_| {
                Error::ProviderFailure(format!(
                    "migration `{}` has an invalid numeric version prefix",
                    path.display()
                ))
            })?;
            migrations.push((version, path));
        }
        migrations.sort_by(|a, b| (a.0, &a.1).cmp(&(b.0, &b.1)));
        for pair in migrations.windows(2) {
            if pair[0].0 == pair[1].0 {
                return Err(Error::ProviderFailure(format!(
                    "duplicate migration version {}: {} and {}",
                    pair[0].0,
                    pair[0].1.display(),
                    pair[1].1.display()
                )));
            }
        }
        Ok(migrations)
    }

    fn apply_file(
        &self,
        path: &Path,
        schema: &mut EffectiveDatabaseSchema,
        unsupported: &mut Vec<UnsupportedSql>,
        gaps: &mut Vec<CoverageGap>,
    ) -> Result<(), Error> {
        let sql = fs::read_to_string(path).map_err(|source| Error::Io {
            path: path.to_path_buf(),
            source,
        })?;
        let parsed = pg_query::parse(&sql).map_err(|error| {
            Error::ProviderFailure(format!(
                "cannot parse {} as PostgreSQL: {error}",
                path.display()
            ))
        })?;
        let relative = path
            .strip_prefix(&self.project_root)
            .unwrap_or(path)
            .to_path_buf();
        for raw in parsed.protobuf.stmts {
            let Some(node) = raw.stmt.and_then(|node| node.node) else {
                continue;
            };
            let offset = usize::try_from(raw.stmt_location.max(0)).unwrap_or(0);
            let provenance = provenance(&relative, line_at(&sql, offset));
            match node {
                NodeEnum::CreateSchemaStmt(statement) => {
                    schema
                        .schemas
                        .entry(statement.schemaname)
                        .or_insert(provenance);
                }
                NodeEnum::CreateStmt(statement) => {
                    self.apply_create(statement, schema, &sql, &relative, unsupported, gaps)?;
                }
                NodeEnum::AlterTableStmt(statement) => {
                    self.apply_alter(statement, schema, &sql, &relative, unsupported)?;
                }
                NodeEnum::DropStmt(statement) => {
                    self.apply_drop(statement, schema, provenance, unsupported);
                }
                NodeEnum::CreateTableAsStmt(statement) => {
                    self.apply_create_table_as(*statement, schema, &sql, &relative, gaps)?;
                }
                NodeEnum::DoStmt(statement) => {
                    unsupported.push(classify_do(
                        statement,
                        &relative,
                        statement_keyword_line(&sql, offset, "DO"),
                    ));
                }
                NodeEnum::RenameStmt(_) => {
                    unsupported.push(UnsupportedSql {
                        source: relative.clone(),
                        line: line_at(&sql, offset),
                        statement: node_kind(&node).into(),
                        reason: "statement may mutate selected schema facts but is not interpreted"
                            .into(),
                        effect: SchemaEffect::UnknownSchemaEffect,
                    });
                }
                _ => {
                    // DML, indexes, types, functions, triggers, comments, grants and
                    // other statements do not directly establish the selected fact relations.
                }
            }
        }
        Ok(())
    }

    fn apply_create(
        &self,
        statement: CreateStmt,
        schema: &mut EffectiveDatabaseSchema,
        sql: &str,
        source: &Path,
        unsupported: &mut Vec<UnsupportedSql>,
        gaps: &mut Vec<CoverageGap>,
    ) -> Result<(), Error> {
        let relation = statement
            .relation
            .ok_or_else(|| Error::ProviderFailure("CREATE TABLE has no relation".into()))?;
        let table_name = qualified_table_name(&relation, &self.default_schema);
        let table_provenance = provenance(source, line_at(sql, location(relation.location)));
        if statement.if_not_exists && schema.tables.contains_key(&table_name) {
            return Ok(());
        }
        if statement.of_typename.is_some() {
            unsupported.push(UnsupportedSql {
                source: source.into(),
                line: table_provenance.span.as_ref().map_or(1, |span| span.line),
                statement: "CREATE TABLE OF".into(),
                reason: "typed-table columns and constraints are not expanded".into(),
                effect: SchemaEffect::KnownSchemaEffect {
                    scope: CoverageScope::Table(table_name.clone()),
                    relations: vec![
                        "column".into(),
                        "column_type".into(),
                        "column_not_null".into(),
                    ],
                },
            });
        }
        let schema_name = table_name.split_once('.').unwrap().0.to_string();
        schema
            .schemas
            .entry(schema_name)
            .or_insert_with(|| table_provenance.clone());
        let mut table = TableState {
            provenance: table_provenance.clone(),
            columns: BTreeMap::new(),
            constraints: BTreeMap::new(),
        };
        let parents = statement
            .inh_relations
            .iter()
            .filter_map(|node| match node.node.as_ref() {
                Some(NodeEnum::RangeVar(parent)) => {
                    Some(qualified_table_name(parent, &self.default_schema))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        let is_partition = statement.partbound.is_some();
        for parent_name in &parents {
            if let Some(parent) = schema.tables.get(parent_name) {
                inherit_safe_properties(&mut table, parent, &table_provenance);
                if is_partition {
                    schema.partitions.insert(
                        table_name.clone(),
                        (
                            parent_name.clone(),
                            table_provenance.clone(),
                            statement
                                .partbound
                                .as_ref()
                                .and_then(|bound| serde_json::to_string(bound).ok())
                                .unwrap_or_default(),
                        ),
                    );
                    for relation in ["primary_key", "unique_constraint", "foreign_key"] {
                        gaps.push(CoverageGap {
                            relations: vec![relation.into()],
                            scope: CoverageScope::Table(table_name.clone()),
                            diagnostic: CoverageDiagnostic {
                                reason: "partition-local key/index semantics are not modeled"
                                    .into(),
                                provenance: table_provenance.clone(),
                            },
                        });
                    }
                } else {
                    schema
                        .inheritance
                        .entry(table_name.clone())
                        .or_default()
                        .push((parent_name.clone(), table_provenance.clone()));
                }
            } else {
                gaps.push(CoverageGap {
                    relations: vec![
                        "column".into(),
                        "column_type".into(),
                        "column_not_null".into(),
                        "check_constraint".into(),
                    ],
                    scope: CoverageScope::Table(table_name.clone()),
                    diagnostic: CoverageDiagnostic {
                        reason: format!(
                            "parent `{parent_name}` is outside modeled effective state"
                        ),
                        provenance: table_provenance.clone(),
                    },
                });
            }
        }
        for element in statement.table_elts {
            match element.node {
                Some(NodeEnum::ColumnDef(column)) => {
                    self.add_column(&table_name, &mut table, *column, sql, source)?;
                }
                Some(NodeEnum::Constraint(constraint)) => {
                    self.add_constraint(&table_name, &mut table, *constraint, None, sql, source)?;
                }
                Some(NodeEnum::TableLikeClause(_)) => unsupported.push(UnsupportedSql {
                    source: source.into(),
                    line: table.provenance.span.as_ref().map_or(1, |span| span.line),
                    statement: "CREATE TABLE LIKE".into(),
                    reason: "LIKE-derived columns and constraints are not expanded".into(),
                    effect: SchemaEffect::KnownSchemaEffect {
                        scope: CoverageScope::Table(table_name.clone()),
                        relations: vec![
                            "column".into(),
                            "column_type".into(),
                            "column_not_null".into(),
                            "check_constraint".into(),
                            "primary_key".into(),
                            "unique_constraint".into(),
                        ],
                    },
                }),
                _ => {}
            }
        }
        for constraint in statement.constraints {
            if let Some(NodeEnum::Constraint(constraint)) = constraint.node {
                self.add_constraint(&table_name, &mut table, *constraint, None, sql, source)?;
            }
        }
        schema.tables.insert(table_name, table);
        Ok(())
    }

    fn apply_create_table_as(
        &self,
        statement: CreateTableAsStmt,
        schema: &mut EffectiveDatabaseSchema,
        sql: &str,
        source: &Path,
        gaps: &mut Vec<CoverageGap>,
    ) -> Result<(), Error> {
        let into = statement
            .into
            .ok_or_else(|| Error::ProviderFailure("CREATE TABLE AS has no target".into()))?;
        let relation = into
            .rel
            .ok_or_else(|| Error::ProviderFailure("CREATE TABLE AS has no relation".into()))?;
        let name = qualified_table_name(&relation, &self.default_schema);
        let current = provenance(source, line_at(sql, location(relation.location)));
        schema
            .schemas
            .entry(name.split_once('.').unwrap().0.into())
            .or_insert_with(|| current.clone());
        let object_type = ObjectType::try_from(statement.objtype).unwrap_or(ObjectType::Undefined);
        let scope = if object_type == ObjectType::ObjectMatview {
            schema
                .materialized_views
                .insert(name.clone(), current.clone());
            CoverageScope::MaterializedView(name)
        } else {
            schema.tables.insert(
                name.clone(),
                TableState {
                    provenance: current.clone(),
                    columns: BTreeMap::new(),
                    constraints: BTreeMap::new(),
                },
            );
            CoverageScope::Table(name)
        };
        gaps.push(CoverageGap {
            relations: vec!["column".into(), "column_type".into()],
            scope,
            diagnostic: CoverageDiagnostic {
                reason: "result columns and types require SELECT result inference".into(),
                provenance: current,
            },
        });
        Ok(())
    }

    fn add_column(
        &self,
        table_name: &str,
        table: &mut TableState,
        column: ColumnDef,
        sql: &str,
        source: &Path,
    ) -> Result<(), Error> {
        let column_provenance = provenance(source, line_at(sql, location(column.location)));
        let type_name = column
            .type_name
            .as_ref()
            .map(normalized_type)
            .unwrap_or_else(|| "unknown".into());
        let name = column.colname.clone();
        let mut explicit_not_null = column.is_not_null.then(|| column_provenance.clone());
        table.columns.insert(
            name.clone(),
            ColumnState {
                type_name,
                provenance: column_provenance,
                explicit_not_null: explicit_not_null.clone(),
            },
        );
        for constraint in column.constraints {
            if let Some(NodeEnum::Constraint(constraint)) = constraint.node {
                if ConstrType::try_from(constraint.contype) == Ok(ConstrType::ConstrNotnull) {
                    explicit_not_null = Some(provenance(
                        source,
                        line_at(sql, location(constraint.location)),
                    ));
                    if let Some(state) = table.columns.get_mut(&name) {
                        state.explicit_not_null = explicit_not_null.clone();
                    }
                }
                self.add_constraint(
                    table_name,
                    table,
                    *constraint,
                    Some(name.clone()),
                    sql,
                    source,
                )?;
            }
        }
        Ok(())
    }

    fn add_constraint(
        &self,
        table_name: &str,
        table: &mut TableState,
        constraint: Constraint,
        inline_column: Option<String>,
        sql: &str,
        source: &Path,
    ) -> Result<(), Error> {
        let kind = ConstrType::try_from(constraint.contype).unwrap_or(ConstrType::Undefined);
        if matches!(
            kind,
            ConstrType::ConstrNotnull | ConstrType::ConstrDefault | ConstrType::ConstrNull
        ) {
            return Ok(());
        }
        let mut columns = strings(&constraint.keys);
        if columns.is_empty()
            && let Some(column) = inline_column.clone()
        {
            columns.push(column);
        }
        let constraint_provenance = provenance(source, line_at(sql, location(constraint.location)));
        let generated = generated_constraint_name(table_name, kind, &columns);
        let name = if constraint.conname.is_empty() {
            generated
        } else {
            constraint.conname.clone()
        };
        let state = match kind {
            ConstrType::ConstrPrimary => Some(ConstraintState::Primary {
                columns,
                provenance: constraint_provenance,
            }),
            ConstrType::ConstrUnique => Some(ConstraintState::Unique {
                columns,
                provenance: constraint_provenance,
            }),
            ConstrType::ConstrForeign => {
                let mut foreign_columns = strings(&constraint.fk_attrs);
                if foreign_columns.is_empty()
                    && let Some(column) = inline_column
                {
                    foreign_columns.push(column);
                }
                Some(ConstraintState::Foreign {
                    columns: foreign_columns,
                    target_table: constraint
                        .pktable
                        .as_ref()
                        .map(|table| qualified_table_name(table, &self.default_schema))
                        .unwrap_or_default(),
                    target_columns: strings(&constraint.pk_attrs),
                    provenance: constraint_provenance,
                })
            }
            ConstrType::ConstrCheck => Some(ConstraintState::Check {
                expression: constraint
                    .raw_expr
                    .as_ref()
                    .and_then(|node| serde_json::to_string(node).ok())
                    .unwrap_or_else(|| constraint.cooked_expr.clone()),
                provenance: constraint_provenance,
            }),
            _ => None,
        };
        if let Some(state) = state {
            table.constraints.insert(name, state);
        }
        Ok(())
    }

    fn apply_alter(
        &self,
        statement: AlterTableStmt,
        schema: &mut EffectiveDatabaseSchema,
        sql: &str,
        source: &Path,
        unsupported: &mut Vec<UnsupportedSql>,
    ) -> Result<(), Error> {
        let relation = statement
            .relation
            .ok_or_else(|| Error::ProviderFailure("ALTER TABLE has no relation".into()))?;
        let name = qualified_table_name(&relation, &self.default_schema);
        let alter_line = line_at(sql, location(relation.location));
        let Some(table) = schema.tables.get_mut(&name) else {
            unsupported.push(UnsupportedSql {
                source: source.into(),
                line: line_at(sql, location(relation.location)),
                statement: format!("ALTER TABLE {name}"),
                reason: "target table is outside the modeled migration state".into(),
                effect: SchemaEffect::KnownSchemaEffect {
                    scope: CoverageScope::Table(name),
                    relations: RELATIONS.iter().map(|value| (*value).into()).collect(),
                },
            });
            return Ok(());
        };
        for command in statement.cmds {
            let Some(NodeEnum::AlterTableCmd(command)) = command.node else {
                continue;
            };
            self.apply_alter_command(
                table,
                *command,
                AlterContext {
                    table_name: &name,
                    sql,
                    source,
                    line: alter_line,
                },
                unsupported,
            )?;
        }
        Ok(())
    }

    fn apply_alter_command(
        &self,
        table: &mut TableState,
        command: AlterTableCmd,
        context: AlterContext<'_>,
        unsupported: &mut Vec<UnsupportedSql>,
    ) -> Result<(), Error> {
        let kind = AlterTableType::try_from(command.subtype).unwrap_or(AlterTableType::Undefined);
        match kind {
            AlterTableType::AtAddColumn => {
                if let Some(NodeEnum::ColumnDef(column)) = command.def.and_then(|node| node.node) {
                    self.add_column(
                        context.table_name,
                        table,
                        *column,
                        context.sql,
                        context.source,
                    )?;
                }
            }
            AlterTableType::AtDropColumn => {
                table.columns.remove(&command.name);
                table
                    .constraints
                    .retain(|_, constraint| !constraint.uses_column(&command.name));
            }
            AlterTableType::AtSetNotNull => {
                if let Some(column) = table.columns.get_mut(&command.name) {
                    column.explicit_not_null = Some(provenance(context.source, context.line));
                }
            }
            AlterTableType::AtDropNotNull => {
                if let Some(column) = table.columns.get_mut(&command.name) {
                    column.explicit_not_null = None;
                }
            }
            AlterTableType::AtAddConstraint => {
                if let Some(NodeEnum::Constraint(constraint)) =
                    command.def.and_then(|node| node.node)
                {
                    self.add_constraint(
                        context.table_name,
                        table,
                        *constraint,
                        None,
                        context.sql,
                        context.source,
                    )?;
                }
            }
            AlterTableType::AtDropConstraint => {
                table.constraints.remove(&command.name);
            }
            AlterTableType::AtColumnDefault
            | AlterTableType::AtAlterColumnType
            | AlterTableType::AtValidateConstraint
            | AlterTableType::AtAlterConstraint
            | AlterTableType::AtSetStatistics
            | AlterTableType::AtSetOptions
            | AlterTableType::AtResetOptions
            | AlterTableType::AtSetStorage
            | AlterTableType::AtSetCompression => {}
            _ => unsupported.push(UnsupportedSql {
                source: context.source.into(),
                line: context.line,
                statement: format!("ALTER TABLE command {}", kind.as_str_name()),
                reason: "ALTER variant is not classified for selected schema facts".into(),
                effect: SchemaEffect::KnownSchemaEffect {
                    scope: CoverageScope::Table(context.table_name.into()),
                    relations: RELATIONS.iter().map(|value| (*value).into()).collect(),
                },
            }),
        }
        Ok(())
    }

    fn apply_drop(
        &self,
        statement: DropStmt,
        schema: &mut EffectiveDatabaseSchema,
        provenance: Provenance,
        unsupported: &mut Vec<UnsupportedSql>,
    ) {
        let object_type = ObjectType::try_from(statement.remove_type);
        if matches!(
            object_type,
            Ok(ObjectType::ObjectTable | ObjectType::ObjectMatview)
        ) {
            for object in statement.objects {
                if let Some(NodeEnum::List(list)) = object.node {
                    let parts = strings(&list.items);
                    let name = if parts.len() == 1 {
                        format!("{}.{}", self.default_schema, parts[0])
                    } else {
                        parts.join(".")
                    };
                    if object_type == Ok(ObjectType::ObjectMatview) {
                        schema.materialized_views.remove(&name);
                    } else {
                        schema.tables.remove(&name);
                        schema.partitions.remove(&name);
                        schema.inheritance.remove(&name);
                    }
                }
            }
        } else if matches!(
            ObjectType::try_from(statement.remove_type),
            Ok(ObjectType::ObjectSchema
                | ObjectType::ObjectColumn
                | ObjectType::ObjectTabconstraint)
        ) {
            unsupported.push(UnsupportedSql {
                source: provenance.source,
                line: provenance.span.map_or(1, |span| span.line),
                statement: "DROP schema/column/constraint".into(),
                reason: "standalone DROP object form is not interpreted".into(),
                effect: SchemaEffect::UnknownSchemaEffect,
            });
        }
    }

    fn emit_facts(
        &self,
        schema: &EffectiveDatabaseSchema,
        migrations: &[(u64, PathBuf)],
    ) -> Vec<ProjectFact> {
        let mut facts = Vec::new();
        for (_, path) in migrations {
            let relative = path.strip_prefix(&self.project_root).unwrap_or(path);
            facts.push(fact(
                format!("sql:migration:{}", relative.display()),
                "migration",
                vec![relative.display().to_string()],
                provenance(relative, 1),
                BTreeMap::new(),
            ));
        }
        for (name, source) in &schema.schemas {
            facts.push(fact(
                format!("sql:schema:{name}"),
                "schema",
                vec![name.clone()],
                source.clone(),
                BTreeMap::new(),
            ));
        }
        for (table_name, table) in &schema.tables {
            facts.push(fact(
                format!("sql:table:{table_name}"),
                "table",
                vec![table_name.clone()],
                table.provenance.clone(),
                BTreeMap::new(),
            ));
            let primary_columns =
                table
                    .constraints
                    .values()
                    .find_map(|constraint| match constraint {
                        ConstraintState::Primary {
                            columns,
                            provenance,
                        } => Some((columns, provenance)),
                        _ => None,
                    });
            for (column_name, column) in &table.columns {
                facts.push(fact(
                    format!("sql:column:{table_name}:{column_name}"),
                    "column",
                    vec![table_name.clone(), column_name.clone()],
                    column.provenance.clone(),
                    BTreeMap::new(),
                ));
                facts.push(fact(
                    format!("sql:column-type:{table_name}:{column_name}"),
                    "column_type",
                    vec![
                        table_name.clone(),
                        column_name.clone(),
                        column.type_name.clone(),
                    ],
                    column.provenance.clone(),
                    BTreeMap::new(),
                ));
                let derived = primary_columns
                    .filter(|(columns, _)| columns.contains(column_name))
                    .map(|(_, provenance)| (*provenance).clone());
                if let Some(source) = column.explicit_not_null.clone().or(derived) {
                    facts.push(fact(
                        format!("sql:not-null:{table_name}:{column_name}"),
                        "column_not_null",
                        vec![table_name.clone(), column_name.clone()],
                        source,
                        BTreeMap::from([(
                            "basis".into(),
                            if column.explicit_not_null.is_some() {
                                "explicit"
                            } else {
                                "primary_key_semantics"
                            }
                            .into(),
                        )]),
                    ));
                }
            }
            for (constraint_name, constraint) in &table.constraints {
                let (relation, arguments, attributes) = match constraint {
                    ConstraintState::Primary { columns, .. } => (
                        "primary_key",
                        vec![table_name.clone(), columns.join(",")],
                        BTreeMap::from([("constraint_name".into(), constraint_name.clone())]),
                    ),
                    ConstraintState::Unique { columns, .. } => (
                        "unique_constraint",
                        vec![table_name.clone(), columns.join(",")],
                        BTreeMap::from([("constraint_name".into(), constraint_name.clone())]),
                    ),
                    ConstraintState::Foreign {
                        columns,
                        target_table,
                        target_columns,
                        ..
                    } => (
                        "foreign_key",
                        vec![
                            table_name.clone(),
                            columns.join(","),
                            target_table.clone(),
                            target_columns.join(","),
                        ],
                        BTreeMap::from([("constraint_name".into(), constraint_name.clone())]),
                    ),
                    ConstraintState::Check { expression, .. } => (
                        "check_constraint",
                        vec![table_name.clone(), constraint_name.clone()],
                        BTreeMap::from([("expression_ast_json".into(), expression.clone())]),
                    ),
                };
                facts.push(fact(
                    format!("sql:{relation}:{table_name}:{constraint_name}"),
                    relation,
                    arguments,
                    constraint.provenance().clone(),
                    attributes,
                ));
            }
        }
        for (name, source) in &schema.materialized_views {
            facts.push(fact(
                format!("sql:materialized-view:{name}"),
                "materialized_view",
                vec![name.clone()],
                source.clone(),
                BTreeMap::new(),
            ));
        }
        for (child, (parent, source, bound)) in &schema.partitions {
            facts.push(fact(
                format!("sql:partition-of:{child}:{parent}"),
                "partition_of",
                vec![child.clone(), parent.clone()],
                source.clone(),
                BTreeMap::from([("partition_bound_ast_json".into(), bound.clone())]),
            ));
        }
        for (child, parents) in &schema.inheritance {
            for (parent, source) in parents {
                facts.push(fact(
                    format!("sql:inherits:{child}:{parent}"),
                    "inherits",
                    vec![child.clone(), parent.clone()],
                    source.clone(),
                    BTreeMap::new(),
                ));
            }
        }
        facts.sort_by(|a, b| a.id.cmp(&b.id));
        facts
    }
}

fn inherit_safe_properties(
    child: &mut TableState,
    parent: &TableState,
    child_provenance: &Provenance,
) {
    let primary_columns = parent
        .constraints
        .values()
        .find_map(|constraint| match constraint {
            ConstraintState::Primary { columns, .. } => Some(columns),
            _ => None,
        });
    for (name, column) in &parent.columns {
        child.columns.insert(
            name.clone(),
            ColumnState {
                type_name: column.type_name.clone(),
                provenance: child_provenance.clone(),
                explicit_not_null: (column.explicit_not_null.is_some()
                    || primary_columns.is_some_and(|columns| columns.contains(name)))
                .then(|| child_provenance.clone()),
            },
        );
    }
    for (name, constraint) in &parent.constraints {
        if let ConstraintState::Check { expression, .. } = constraint {
            child.constraints.insert(
                name.clone(),
                ConstraintState::Check {
                    expression: expression.clone(),
                    provenance: child_provenance.clone(),
                },
            );
        }
    }
}

fn classify_do(statement: DoStmt, source: &Path, line: usize) -> UnsupportedSql {
    let body = statement.args.iter().find_map(|argument| {
        let Some(NodeEnum::DefElem(element)) = argument.node.as_ref() else {
            return None;
        };
        if element.defname != "as" {
            return None;
        }
        match element
            .arg
            .as_ref()
            .and_then(|argument| argument.node.as_ref())
        {
            Some(NodeEnum::String(value)) => Some(value.sval.as_str()),
            _ => None,
        }
    });
    let tokens = body
        .unwrap_or_default()
        .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>();
    let unsafe_token = tokens.iter().find(|token| {
        matches!(
            token.as_str(),
            "execute"
                | "perform"
                | "call"
                | "create"
                | "alter"
                | "drop"
                | "truncate"
                | "grant"
                | "revoke"
        )
    });
    let unknown_call = unknown_plpgsql_call(body.unwrap_or_default());
    let (reason, effect) = if let Some(token) = unsafe_token {
        (
            format!("DO body contains potentially dynamic/schema-mutating token `{token}`"),
            SchemaEffect::UnknownSchemaEffect,
        )
    } else if let Some(call) = unknown_call {
        (
            format!("DO body calls non-allowlisted function/procedure `{call}`"),
            SchemaEffect::UnknownSchemaEffect,
        )
    } else {
        (
            "DO body contains no EXECUTE or schema-mutating DDL token".into(),
            SchemaEffect::KnownIrrelevantToSchema,
        )
    };
    UnsupportedSql {
        source: source.into(),
        line,
        statement: "DO".into(),
        reason,
        effect,
    }
}

fn unknown_plpgsql_call(body: &str) -> Option<String> {
    let mut outside_strings = String::with_capacity(body.len());
    let mut quoted = false;
    let mut chars = body.chars().peekable();
    while let Some(character) = chars.next() {
        if character == '\'' {
            if quoted && chars.peek() == Some(&'\'') {
                chars.next();
                outside_strings.push(' ');
                outside_strings.push(' ');
                continue;
            }
            quoted = !quoted;
            outside_strings.push(' ');
        } else {
            outside_strings.push(if quoted { ' ' } else { character });
        }
    }
    for (position, _) in outside_strings.match_indices('(') {
        let prefix = outside_strings[..position].trim_end();
        let name = prefix
            .chars()
            .rev()
            .take_while(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '_' | '.')
            })
            .collect::<String>()
            .chars()
            .rev()
            .collect::<String>()
            .to_ascii_lowercase();
        if !name.is_empty() && !matches!(name.as_str(), "exists" | "count") {
            return Some(name);
        }
    }
    None
}

fn effect_name(effect: &SchemaEffect) -> &'static str {
    match effect {
        SchemaEffect::KnownIrrelevantToSchema => "known_irrelevant_to_schema",
        SchemaEffect::KnownSchemaEffect { .. } => "known_schema_effect",
        SchemaEffect::UnknownSchemaEffect => "unknown_schema_effect",
    }
}

fn statement_keyword_line(sql: &str, offset: usize, keyword: &str) -> usize {
    let tail = &sql[offset.min(sql.len())..];
    let position = tail
        .find(&format!("\n{keyword} "))
        .map(|value| offset + value + 1)
        .or_else(|| tail.strip_prefix(keyword).map(|_| offset))
        .unwrap_or(offset);
    line_at(sql, position)
}

fn qualified_table_name(relation: &RangeVar, default_schema: &str) -> String {
    format!(
        "{}.{}",
        if relation.schemaname.is_empty() {
            default_schema
        } else {
            &relation.schemaname
        },
        relation.relname
    )
}

fn normalized_type(name: &pg_query::protobuf::TypeName) -> String {
    let mut parts = strings(&name.names);
    if parts.first().is_some_and(|part| part == "pg_catalog") {
        parts.remove(0);
    }
    let mut value = parts.join(".");
    for _ in &name.array_bounds {
        value.push_str("[]");
    }
    value
}

fn strings(nodes: &[pg_query::Node]) -> Vec<String> {
    nodes
        .iter()
        .filter_map(|node| match node.node.as_ref()? {
            NodeEnum::String(value) => Some(value.sval.clone()),
            _ => None,
        })
        .collect()
}

fn generated_constraint_name(table: &str, kind: ConstrType, columns: &[String]) -> String {
    let local_table = table.rsplit('.').next().unwrap_or(table);
    match kind {
        ConstrType::ConstrPrimary => format!("{local_table}_pkey"),
        ConstrType::ConstrUnique => format!("{local_table}_{}_key", columns.join("_")),
        ConstrType::ConstrForeign => format!("{local_table}_{}_fkey", columns.join("_")),
        ConstrType::ConstrCheck => format!("{local_table}_{}_check", columns.join("_")),
        _ => format!("__{}:{}", kind.as_str_name(), columns.join(",")),
    }
}

fn provenance(source: &Path, line: usize) -> Provenance {
    Provenance {
        kind: ProvenanceKind::DeterministicallyExtracted,
        source: source.to_path_buf(),
        span: Some(SourceSpan {
            filename: source.to_path_buf(),
            line,
            column: 1,
        }),
        extractor: Some("pg_query 6.2.0 / PostgreSQL parser".into()),
    }
}

fn line_at(sql: &str, offset: usize) -> usize {
    sql.as_bytes()[..offset.min(sql.len())]
        .iter()
        .filter(|byte| **byte == b'\n')
        .count()
        + 1
}

fn location(value: i32) -> usize {
    usize::try_from(value.max(0)).unwrap_or(0)
}

fn node_kind(node: &NodeEnum) -> &'static str {
    match node {
        NodeEnum::CreateTableAsStmt(_) => "CREATE TABLE AS",
        NodeEnum::RenameStmt(_) => "RENAME",
        NodeEnum::DoStmt(_) => "DO",
        _ => "unsupported statement",
    }
}

fn fact(
    id: String,
    relation: &str,
    arguments: Vec<String>,
    provenance: Provenance,
    attributes: BTreeMap<String, String>,
) -> ProjectFact {
    ProjectFact {
        id: FactId(id),
        relation: relation.into(),
        arguments,
        value: true,
        attributes,
        provenance,
    }
}
