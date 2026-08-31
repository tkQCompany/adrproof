# Modeling and language strategy

ADRProof uses one Project Intent Model, but deliberately does not offer one
universal specification language. The shared model records stable identities,
project-level facts and constraints, dependencies, provenance, proof obligations,
verifier runs and evidence. It does not absorb every source language's semantics.

ADRLogic remains a small relational frontend for project-level constraints. Rust
function contracts belong in Anodized, Verus, or Creusot; concurrency and temporal
behavior in TLA+ or Quint; APIs in OpenAPI/JSON Schema; databases in SQL DDL;
embedded timing and allocation analyses may use AADL; hardware and properties may
use VHDL/SystemVerilog and SVA/PSL. Verifier-native artifacts remain inspectable
and editable by experts. LLM-generated formalizations remain untrusted until a
deterministic tool checks them.

SysML v2 is an important precedent because its semantic model can support textual,
graphical, and API views. Future C4, Mermaid, PlantUML, or SysML projections should
likewise be views of ADRProof's model, not independent sources of truth. UML and
Executable UML also show the cost of stretching a universal model toward all
implementation semantics. AADL and hardware/property languages demonstrate the
value of precise, domain-specific formalisms.

The closer analogy is SymbiYosys: ADRProof orchestrates heterogeneous engines,
tracks what they checked and on which inputs, and connects evidence to intent. It
does not compete with those engines or translate them lossily into ADRLogic.

The PostgreSQL migration provider follows the same rule. SQL DDL remains the
native, expert-inspectable schema language. ADRProof parses a deliberately bounded
set of structural facts from it and publishes explicit coverage; it does not
translate SQL into a new database DSL or claim to implement PostgreSQL semantics.
