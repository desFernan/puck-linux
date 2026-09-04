//! Loads a real avatar folder off disk, rather than one written into a
//! temp directory by the unit tests. The fixture is shaped the way
//! puck-mac's packages are — locomotion clips under `clips`, agent
//! reactions under `emotions`, one PNG per entry beside the manifest — so
//! this is the check that a package authored on macOS drops in unchanged.

use puck_linux::avatar;

fn fixture(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/avatars")
        .join(name)
}

#[test]
fn loads_a_package_that_keeps_emotions_in_their_own_map() {
    let dir = fixture("valid");
    let loaded = avatar::load(&dir).expect("the fixture package should load");

    assert_eq!(loaded.hitbox.width, 32.0);
    assert_eq!(loaded.hitbox.height, 32.0);

    assert_eq!(loaded.clips.get("idle"), Some(&dir.join("idle.png")));
    assert_eq!(loaded.clips.get("walk"), Some(&dir.join("walk.png")));
    // Declared under `emotions`, reachable by the same name as any clip —
    // this is what the bridge swaps in while the agent is working.
    assert_eq!(
        loaded.clips.get("thinking"),
        Some(&dir.join("thinking.png"))
    );
}

#[test]
fn a_folder_with_no_manifest_is_refused_rather_than_ignored() {
    let missing = fixture("no-such-package");
    assert!(matches!(
        avatar::load(&missing),
        Err(avatar::LoadError::Io(_))
    ));
}
