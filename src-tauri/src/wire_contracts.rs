//! The Serde DTOs are the wire type source. This test runs in the existing Rust
//! quality gate; regeneration is explicit and never changes a normal test run.
use std::{
    any::TypeId,
    collections::{BTreeMap, HashSet},
    path::PathBuf,
};
use ts_rs::{Config, TypeVisitor, TS};

struct Declarations {
    config: Config,
    seen: HashSet<TypeId>,
    types: BTreeMap<String, String>,
}

impl TypeVisitor for Declarations {
    fn visit<T: TS + 'static + ?Sized>(&mut self) {
        if !self.seen.insert(TypeId::of::<T>()) {
            return;
        }
        if T::output_path().is_some() {
            self.types
                .insert(T::ident(&self.config), T::decl(&self.config));
        }
        T::visit_dependencies(self);
    }
}

#[test]
fn generated_wire_contracts_are_current() {
    let mut declarations = Declarations {
        // JSON numbers match IPC. Runtime schemas reject integers outside JS's
        // safe range instead of silently accepting a lossy u64/u128 value.
        config: Config::new().with_large_int("number"),
        seen: HashSet::new(),
        types: BTreeMap::new(),
    };
    declarations.visit::<crate::AppBuildInfo>();
    declarations.visit::<crate::settings::AppSettings>();
    declarations.visit::<crate::settings::SettingsRecoveryNotice>();
    declarations.visit::<crate::launcher::LauncherAddResult>();
    declarations.visit::<crate::launcher::LauncherLaunchResult>();
    declarations.visit::<crate::twitch::TwitchDeviceAuthStart>();
    declarations.visit::<crate::twitch::TwitchAuthPollResult>();
    declarations.visit::<crate::twitch::TwitchAuthValidationResult>();
    declarations.visit::<crate::twitch::ChatMessage>();
    declarations.visit::<crate::app_events::AppEventsSnapshot>();
    declarations.visit::<crate::app_events::SpeechStateSnapshot>();
    declarations.visit::<crate::speech::bouyomi::BouyomiConnectionDiagnostics>();
    let mut generated = String::from("// Generated from Rust Serde DTOs by ts-rs. Do not edit.\n// Regenerate: RICE_UPDATE_WIRE_TYPES=1 cargo test --manifest-path src-tauri/Cargo.toml --no-default-features generated_wire_contracts_are_current\n\n");
    for declaration in declarations.types.values() {
        generated.push_str("export ");
        generated.push_str(declaration);
        generated.push('\n');
    }
    generated.push_str("\nexport type WireContracts = {\n");
    for name in declarations.types.keys() {
        generated.push_str(&format!("  {name}: {name};\n"));
    }
    generated.push_str("};\n");
    let generated = generated
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../bindings/wire.ts");
    if std::env::var_os("RICE_UPDATE_WIRE_TYPES").as_deref() == Some(std::ffi::OsStr::new("1")) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &generated).unwrap();
    }
    assert_eq!(
        std::fs::read_to_string(path).expect("generate bindings/wire.ts"),
        generated,
        "Rust wire DTOs changed; regenerate bindings/wire.ts and review schema parity"
    );
}
