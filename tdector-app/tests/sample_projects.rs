use tdector_app::Session;

#[test]
fn bundled_projects_load_and_roundtrip_through_the_headless_api() {
    for name in ["ginger", "epigraph"] {
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../sample/{name}.json"));
        let json = std::fs::read_to_string(path).expect("bundled sample");
        let mut session = Session::default();
        session
            .load_json(&json)
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        assert!(!session.project().segments.is_empty(), "{name}");
        assert!(!session.is_dirty(), "{name}");
        let snapshot = session.save_snapshot().expect("save bundled sample");
        let mut reloaded = Session::default();
        reloaded
            .load_json(std::str::from_utf8(&snapshot.bytes).expect("JSON"))
            .expect("reload bundled sample");
        assert_eq!(
            snapshot.bytes,
            reloaded.save_snapshot().expect("resave").bytes,
            "{name}"
        );
    }
}
