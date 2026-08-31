# PostgreSQL migration fact provider

`PostgresMigrationFactProvider` statically analyzes an ordered PostgreSQL
migration stream. It does not run PostgreSQL, migrations, or procedures, and it
is not a substitute for the catalog of a live database. SQL remains the native
schema language; the provider projects only explicitly supported facts into the
Project Intent Model.

## Parser and scope

The provider uses pinned
[`pg_query 6.2.0`](https://docs.rs/pg_query/6.2.0/pg_query/), a typed AST generated
by the PostgreSQL parser from
[`libpg_query`](https://github.com/pganalyze/libpg_query). This is deliberately a
stronger trust boundary than a custom or multi-dialect parser, although a small,
auditable ADRProof adapter still implements the schema-state transition
semantics.

`pg_query 6.2.0` embeds the PostgreSQL 17.7 parser. The adapter's semantic
baseline is the PostgreSQL 17/18 documentation for the supported DDL subset. The
analyzed integration currently builds PostgreSQL 18 images, and the supported
constructs have no known version difference material to the facts below. The
provider is not, however, a general PostgreSQL version-compatibility engine.

PostgreSQL is the only supported dialect. Autodetection covers
`project_root/migrations`. The provider selects `*.sql` files, skips
`*.down.sql`, requires a numeric prefix before the first `_`, sorts by its value,
and rejects duplicate versions. It does not recurse into subdirectories or merge
multiple independent database streams.

## EffectiveDatabaseSchema

Migrations are applied in order to an internal model keyed by normalized schema,
table, column, and constraint names. The model retains current columns and PK,
UNIQUE, FK, and CHECK constraints. `DROP`, `DROP COLUMN`, `DROP CONSTRAINT`, and
`DROP NOT NULL` remove the corresponding current state; historical facts are not
emitted as facts about the current schema.

Supported constructs that change selected relations are:

- `CREATE SCHEMA` and ordinary `CREATE TABLE`;
- inline and table-level `PRIMARY KEY`, `UNIQUE`, `REFERENCES`/`FOREIGN KEY`,
  and `CHECK`;
- `ALTER TABLE ADD/DROP COLUMN`;
- `ALTER COLUMN SET/DROP NOT NULL`;
- `ADD/DROP CONSTRAINT`;
- `DROP TABLE` and `DROP MATERIALIZED VIEW`;
- `CREATE TABLE ... PARTITION OF` and `CREATE TABLE ... INHERITS` within the
  subset described below;
- `CREATE TABLE AS` and `CREATE MATERIALIZED VIEW AS` at object-identity level.

A PK preserves the column order of a composite key. PK columns also receive
`column_not_null` with `basis=primary_key_semantics`; this is a semantic
consequence of PostgreSQL and is distinguished from an explicit `NOT NULL`.
CHECK preserves its name and a deterministic JSON representation of its AST, but
ADRProof does not interpret the expression's meaning.

According to the official
[inheritance documentation](https://www.postgresql.org/docs/18/ddl-inherit.html),
`INHERITS` copies columns, types, CHECK constraints, and NOT NULL constraints,
but not PK, UNIQUE, or FK constraints. The provider emits those safely inherited
facts and a separate `inherits` relation.
[Partitions](https://www.postgresql.org/docs/18/sql-createtable.html) require
compatible columns and inherit CHECK and NOT NULL constraints; the provider emits
those facts and `partition_of`. Partition-local PK/UNIQUE/FK facts remain scoped
as `Partial` because the adapter does not model partitioned-index relationships.

[CTAS](https://www.postgresql.org/docs/18/sql-createtableas.html) creates an
ordinary table, so `table(name)` is Closed. Without analysis of the SELECT
result, columns and their types are Partial only for that table. Similarly,
[CREATE MATERIALIZED VIEW](https://www.postgresql.org/docs/18/sql-creatematerializedview.html)
emits `materialized_view`, never `table`; resulting columns and types remain
Partial for the materialized view.

Renames, typed tables, `LIKE`, and unknown ALTER variants are not fully
interpreted. DML, indexes, types, functions, triggers, and comments do not
establish the selected relations. A `UNIQUE INDEX` is not a
`unique_constraint` fact.

## Facts and identifiers

The emitted relations are:

- `migration(path)`;
- `schema(schema)`;
- `table(schema.table)`;
- `column(table, column)`;
- `column_type(table, column, normalized_type_name)`;
- `column_not_null(table, column)`;
- `primary_key(table, comma_separated_columns)`;
- `unique_constraint(table, comma_separated_columns)`;
- `foreign_key(table, columns, target_table, target_columns)`;
- `check_constraint(table, constraint_name)`;
- `partition_of(child, parent)`;
- `inherits(child, parent)`;
- `materialized_view(schema.object)`;
- diagnostic `unsupported_sql(statement)`.

A type is a structural name derived from the AST, not the complete PostgreSQL
type system. Unqualified names receive a configurable default schema (`public`).
The PostgreSQL parser folds unquoted identifiers while preserving the distinction
between `foo` and `"Foo"`. Runtime `search_path` resolution is not performed.

Every current fact identifies the migration and statement line that established
its current state. Entry identifiers and fingerprints use logical
`project:migrations/...` paths, so relocating a checkout does not invalidate
evidence. A fingerprint includes the complete content of every relevant
migration; even a comment-only change may conservatively produce STALE evidence.

## Scoped coverage and error policy

`FactCoverage` is defined by `(relation, CoverageScope)`. Typed scopes currently
include `Global`, `Schema`, `Table`, `MaterializedView`, and `Package`. The
provider publishes a global completeness claim for the supported stream and
explicit regions of uncertainty per relation and scope.

Resolution selects all claims whose scope contains the queried object. `Global`
contains everything, `Schema` contains tables and materialized views in that
schema, and an object scope contains only that object. If any matching entry is
`Partial`, the result is Partial; only a non-empty set containing exclusively
Closed entries produces Closed. There is no "most specific wins" rule: a global
Unknown always overrides a local Closed claim.

Unsupported SQL receives a deterministic classification:

- `KnownIrrelevantToSchema` does not degrade selected relations;
- `KnownSchemaEffect { scope, relations }` degrades only the specified region;
- `UnknownSchemaEffect` degrades all selected relations globally.

A `DO` block is classified syntactically. The presence of `EXECUTE`, `PERFORM`,
`CALL`, or a DDL token (`CREATE`, `ALTER`, `DROP`, `TRUNCATE`, `GRANT`, or
`REVOKE`) produces global Unknown. Calling a function or procedure other than the
explicitly safe `count` aggregate also produces Unknown because the called
routine could execute DDL. Only the absence of these constructs establishes no
direct effect on modeled schema DDL. The provider does not interpret PL/pgSQL; a
more complex block remains Unknown.

A positive fact that was actually emitted may still be used within a Partial
region: incompleteness does not invalidate an observation. An absent atom—whether
needed as a negation or as the basis of FAIL for a positive requirement—is
resolved authoritatively only when its relation and scope are Closed. Otherwise,
the result is `UNVERIFIED`, never PASS or FAIL inferred from missing observation.

Every Partial entry retains the migration, line, and reason as provenance. A
parser error, invalid migration name, or duplicate version terminates the
provider with an error.

Migration artifacts have `Produces` edges to facts. A constraint that uses SQL
relations currently depends conservatively on every migration in the stream.
This prevents false-CURRENT results at the cost of potentially excessive STALE
results.
