use super::*;

#[test]
fn format_json_and_json_alias_share_the_report_renderer() {
    let fixture = MixedMapFixtureRepository::new();
    let format_output = fixture.run(&["--no-cache", "--format", "json"]);
    let alias_output = fixture.run(&["--no-cache", "--json"]);

    assert!(format_output.status.success());
    assert!(alias_output.status.success());
    assert!(format_output.stderr.is_empty());
    assert!(alias_output.stderr.is_empty());

    let format_json: Value = serde_json::from_str(&stdout(&format_output)).expect("format JSON");
    let alias_json: Value = serde_json::from_str(&stdout(&alias_output)).expect("alias JSON");
    assert_eq!(format_json, alias_json);
}

#[test]
fn format_html_and_html_alias_render_the_same_standalone_report() {
    let fixture = MixedMapFixtureRepository::new();
    let format_output = fixture.run(&["--no-cache", "--format", "html"]);
    let alias_output = fixture.run(&["--no-cache", "--html"]);

    assert!(format_output.status.success());
    assert!(alias_output.status.success());
    assert!(format_output.stderr.is_empty());
    assert!(alias_output.stderr.is_empty());
    assert_eq!(stdout(&format_output), stdout(&alias_output));

    let html = stdout(&format_output);
    assert!(html.starts_with("<!doctype html>"));
    assert!(html.contains("Suggested reading order"));
    assert!(html.contains("Complete report data"));
    assert!(html.contains("IBM+Plex+Sans"));
    assert!(html.contains("Google+Sans"));
    assert!(html.contains("Google+Sans+Code"));
    assert!(!html.contains("linear-gradient"));
    assert_plain_report(&html);
}

#[cfg(unix)]
#[test]
fn open_writes_a_private_html_report_and_invokes_the_platform_opener() {
    use std::os::unix::fs::PermissionsExt;

    let fixture = MixedMapFixtureRepository::new();
    let opener_directory = fixture.temporary_root.join("opener");
    let marker = fixture.temporary_root.join("opened-path");
    fs::create_dir_all(&opener_directory).expect("create fake opener directory");
    let opener_name = if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
    let opener = opener_directory.join(opener_name);
    write_file(&opener, b"#!/bin/sh\nprintf '%s' \"$1\" > \"$DALIL_OPEN_MARKER\"\n");
    let mut permissions = fs::metadata(&opener).expect("fake opener metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&opener, permissions).expect("make fake opener executable");
    let current_path = env::var_os("PATH").unwrap_or_default();
    let executable_path =
        env::join_paths(std::iter::once(opener_directory.clone()).chain(env::split_paths(&current_path)))
            .expect("build executable search path");

    let output = fixture
        .command(&["--no-cache", "--html", "--open"])
        .env("PATH", executable_path)
        .env("TMPDIR", &fixture.temporary_root)
        .env("DALIL_OPEN_MARKER", &marker)
        .output()
        .expect("run HTML opener fixture");

    assert!(
        output.status.success(),
        "dalil failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("opened HTML report"));
    let report_path = PathBuf::from(fs::read_to_string(&marker).expect("read opened report path"));
    assert!(report_path.starts_with(&fixture.temporary_root));
    assert!(
        fs::read_to_string(&report_path)
            .expect("read temporary HTML report")
            .starts_with("<!doctype html>")
    );
    assert_eq!(
        fs::metadata(report_path.parent().expect("report directory"))
            .expect("report directory metadata")
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    assert_eq!(
        fs::metadata(&report_path)
            .expect("report metadata")
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
}

#[test]
fn open_warns_and_keeps_stdout_for_non_html_formats() {
    let fixture = MixedMapFixtureRepository::new();
    for arguments in [
        ["--no-cache", "--open"].as_slice(),
        ["--no-cache", "--json", "--open"].as_slice(),
    ] {
        let output = fixture.run(arguments);
        assert!(output.status.success());
        assert!(!output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains("only applies to HTML output"));
    }
}

#[test]
fn support_reports_render_as_html_documents() {
    let fixture = FixtureRepository::new();
    for arguments in [
        ["capabilities", "--html"].as_slice(),
        ["doctor", "--html"].as_slice(),
        ["cache", "status", "--html"].as_slice(),
    ] {
        let output = fixture.run(arguments);
        let html = stdout(&output);
        assert!(output.status.success(), "HTML report failed: {arguments:?}");
        assert!(output.stderr.is_empty());
        assert!(html.starts_with("<!doctype html>"));
        assert!(html.contains("Complete report data"));
    }
}

#[test]
fn markdown_snapshot_is_direct_and_readable() {
    let fixture = FixtureRepository::new();
    let output = fixture.run(&["map"]);

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let stable_markdown = stdout(&output)
        .lines()
        .filter(|line| !line.starts_with("Repository: `"))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    assert_eq!(
        stable_markdown,
        "# Dalil map\n\
         \n\
         Schema version: 1\n\
         Scope: `.`\n\
         Status: Analyzed\n\
         \n\
         ## Summary\n\
         \n\
         Analyzed 0 Rust source files and recorded 0 omitted paths within the selected source scope.\n\
         \n\
         ## Source map\n\
         \n\
         Map scope: `.`\n\
         Query pack: `rust-v1`\n\
         Inventory: 0 tracked (0 modified), 0 untracked, 0 analyzed, 0 omitted, 0 classified\n\
         \n\
         ### Map limitations\n\
         \n\
         - Rust definitions and references are extracted lexically; only explicit call or import evidence contributes file relationships, and types, macros, and runtime behavior are not semantically resolved.\n\
         - Reference names can have multiple lexical definition candidates; ambiguity is reported rather than treated as a semantic call edge.\n\
         - Tracked files are eligible even when ignore rules match them, except deterministic generated/vendor/minified classifications; exact focus paths can opt in within the safety limits.\n\
         \n\
         ### Rust files\n\
         \n\
         No Rust files were analyzed.\n"
    );
}

#[test]
fn color_options_never_change_json_stdout() {
    let fixture = MixedMapFixtureRepository::new();
    let never = fixture.run(&["--no-cache", "--color", "never", "--json"]);
    let always = fixture.run(&["--no-cache", "--color", "always", "--json"]);

    assert!(never.status.success());
    assert!(always.status.success());
    assert!(never.stderr.is_empty());
    assert!(always.stderr.is_empty());
    assert_eq!(stdout(&never), stdout(&always));
    assert_plain_report(&stdout(&always));
}

#[test]
fn automatic_diagnostic_color_honors_no_color() {
    let fixture = FixtureRepository::new();
    let no_color = fixture
        .command(&["--format", "markdown", "--json"])
        .env("NO_COLOR", "1")
        .output()
        .expect("run no-color fixture command");
    let always = fixture.run(&["--color", "always", "--format", "markdown", "--json"]);

    assert_eq!(no_color.status.code(), Some(2));
    assert!(no_color.stdout.is_empty());
    assert!(!String::from_utf8_lossy(&no_color.stderr).contains('\u{1b}'));

    assert_eq!(always.status.code(), Some(2));
    assert!(always.stdout.is_empty());
    assert!(String::from_utf8_lossy(&always.stderr).contains('\u{1b}'));
}

#[test]
fn parser_and_usage_errors_use_the_documented_exit_category_and_stderr() {
    let fixture = FixtureRepository::new();
    let invalid_value = fixture.run(&["--format", "xml"]);
    let conflicting_output = fixture.run(&["--format", "markdown", "--json"]);
    let conflicting_aliases = fixture.run(&["--json", "--html"]);

    assert_eq!(invalid_value.status.code(), Some(2));
    assert!(invalid_value.stdout.is_empty());
    assert!(String::from_utf8_lossy(&invalid_value.stderr).contains("error:"));

    assert_eq!(conflicting_output.status.code(), Some(2));
    assert!(conflicting_output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&conflicting_output.stderr).contains("cannot be combined"));

    assert_eq!(conflicting_aliases.status.code(), Some(2));
    assert!(conflicting_aliases.stdout.is_empty());
    assert!(String::from_utf8_lossy(&conflicting_aliases.stderr).contains("cannot be combined"));
}
