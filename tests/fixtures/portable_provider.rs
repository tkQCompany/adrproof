use std::io::{Read, Write};
use std::time::Duration;

fn main() {
    let mode = std::env::args().nth(1).unwrap_or_else(|| "valid".into());
    let mut request = String::new();
    std::io::stdin().read_to_string(&mut request).unwrap();
    assert!(request.contains("adrproof-external-provider-request-v1"));
    assert!(request.contains("\"provider_id\":\"portable-fixture\""));
    assert!(request.contains("\"provider_version\":\"1.0.0\""));

    match mode.as_str() {
        "sleep" => std::thread::sleep(Duration::from_secs(30)),
        "malformed" => print!("{{"),
        "oversized" => {
            std::io::stdout().write_all(&vec![b'x'; 8 * 1024 * 1024 + 1]).unwrap();
        }
        "valid" => print!(
            r#"{{
  "schema_version":"adrproof-external-provider-response-v1",
  "provider":{{"id":"portable-fixture","version":"1.0.0"}},
  "inputs":["project:input.txt"],
  "artifacts":[{{
    "id":"project:input.txt",
    "kind":"portable_fixture",
    "provenance":{{"kind":"deterministically_extracted","source":"project:input.txt","span":null,"extractor":"portable-rust-fixture"}}
  }}],
  "facts":[{{
    "id":"portable-fixture:component:api",
    "relation":"component",
    "arguments":["api"],
    "value":true,
    "attributes":{{}},
    "provenance":{{"kind":"deterministically_extracted","source":"project:input.txt","span":null,"extractor":"portable-rust-fixture"}}
  }}],
  "coverage":[{{
    "relation":"component",
    "provider":"portable-fixture",
    "world":"partial",
    "scope":{{"kind":"global"}},
    "qualifiers":{{"fixture":"portable"}},
    "statement":"the fixture reports one recognized component without a completeness claim",
    "diagnostics":[]
  }}],
  "diagnostics":[]
}}"#
        ),
        other => panic!("unknown fixture mode: {other}"),
    }
}
