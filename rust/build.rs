extern crate configure_me_codegen;

fn main() -> Result<(), String> {
    println!("cargo:rerun-if-changed=executor_config_spec.toml");
    println!("cargo:rerun-if-changed=standalone_config_spec.toml");
    configure_me_codegen::build_script_auto()
        .map_err(|e| format!("configure_me code generation failed: {}", e))
}
