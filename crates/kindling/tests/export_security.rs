//! Regression tests for GH-167: `export` and `browse` must refuse an output
//! path that resolves to the source database (direct, symlink, or hard link).

mod support;

use support::{assert_success, stderr, CliEnv};

#[test]
fn export_refuses_the_source_database_directly() {
    let env = CliEnv::new();
    assert_success(&env.run_db(&["demo", "--reset"]));

    let out = env.run_db(&["export", &env.db()]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("source database"), "{}", stderr(&out));
}

#[cfg(unix)]
#[test]
fn export_refuses_a_symlink_to_the_source_database() {
    let env = CliEnv::new();
    assert_success(&env.run_db(&["demo", "--reset"]));

    let link = env.path("export-link.json");
    std::os::unix::fs::symlink(&env.db_path, &link).unwrap();

    let out = env.run_db(&["export", &link.to_string_lossy()]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("source database"), "{}", stderr(&out));
}

#[cfg(unix)]
#[test]
fn export_refuses_a_hard_link_to_the_source_database() {
    let env = CliEnv::new();
    assert_success(&env.run_db(&["demo", "--reset"]));

    let hard = env.path("export-hardlink.json");
    if std::fs::hard_link(&env.db_path, &hard).is_err() {
        // The filesystem does not support hard links; nothing to test.
        return;
    }

    let out = env.run_db(&["export", &hard.to_string_lossy()]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("source database"), "{}", stderr(&out));
}

#[test]
fn export_allows_a_normal_output_path() {
    let env = CliEnv::new();
    assert_success(&env.run_db(&["demo", "--reset"]));

    let out_path = env.path("bundle.json");
    let out = env.run_db(&["export", &out_path.to_string_lossy()]);
    assert_success(&out);
    assert!(out_path.exists());
}

#[test]
fn browse_refuses_the_source_database_via_output() {
    let env = CliEnv::new();
    assert_success(&env.run_db(&["demo", "--reset"]));

    let out = env.run_db(&["browse", "--no-open", "--output", &env.db()]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("source database"), "{}", stderr(&out));
}

#[cfg(unix)]
#[test]
fn browse_refuses_a_symlink_to_the_source_database() {
    let env = CliEnv::new();
    assert_success(&env.run_db(&["demo", "--reset"]));

    let link = env.path("browse-link.html");
    std::os::unix::fs::symlink(&env.db_path, &link).unwrap();

    let out = env.run_db(&["browse", "--no-open", "--output", &link.to_string_lossy()]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("source database"), "{}", stderr(&out));
}

#[cfg(unix)]
#[test]
fn browse_refuses_a_hard_link_to_the_source_database() {
    let env = CliEnv::new();
    assert_success(&env.run_db(&["demo", "--reset"]));

    let hard = env.path("browse-hardlink.html");
    if std::fs::hard_link(&env.db_path, &hard).is_err() {
        // The filesystem does not support hard links; nothing to test.
        return;
    }

    let out = env.run_db(&["browse", "--no-open", "--output", &hard.to_string_lossy()]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("source database"), "{}", stderr(&out));
}

#[test]
fn browse_allows_a_normal_output_path() {
    let env = CliEnv::new();
    assert_success(&env.run_db(&["demo", "--reset"]));

    let out_path = env.path("browse.html");
    let out = env.run_db(&[
        "browse",
        "--no-open",
        "--output",
        &out_path.to_string_lossy(),
    ]);
    assert_success(&out);
    assert!(out_path.exists());
}
