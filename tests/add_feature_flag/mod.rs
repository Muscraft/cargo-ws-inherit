use cargo_test_support::cargo_test;
use cargo_test_support::compare::assert;
use cargo_test_support::Project;

use cargo_test_support::curr_dir;

#[cargo_test]
fn add_feature_flag() {
    let project = Project::from_template(curr_dir!().join("in"));
    let project_root = project.root();
    let cwd = &project_root;
    snapbox::cmd::Command::new(snapbox::cmd::cargo_bin("cargo-ws-inherit"))
        .arg("run")
        .current_dir(cwd)
        .assert()
        .success()
        .stdout_matches_path(curr_dir!().join("stdout.log"))
        .stderr_matches_path(curr_dir!().join("stderr.log"));

    assert().subset_matches(curr_dir!().join("out"), &project_root);
}
