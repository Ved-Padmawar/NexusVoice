use super::*;

/// Regenerates `src/shared/bindings.ts` on every test run.
#[test]
fn export_bindings_writes_the_typescript_file() {
    export_bindings(&command_bindings());

    let bindings = std::fs::read_to_string("../src/shared/bindings.ts").expect("bindings written");
    assert!(bindings.contains("export const commands"));
}
