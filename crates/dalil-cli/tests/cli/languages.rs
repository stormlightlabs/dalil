use super::*;

#[test]
fn mixed_language_map_is_explicit_deterministic_and_keeps_other_findings() {
    let fixture = MixedMapFixtureRepository::new();
    let first = fixture.run(&["map", "--profile", "evidence", "--no-cache", "--json"]);
    let second = fixture.run(&["map", "--profile", "evidence", "--no-cache", "--json"]);
    let first_stdout = stdout(&first);
    let second_stdout = stdout(&second);
    let json: Value = serde_json::from_str(&first_stdout).expect("valid mixed-language map JSON");

    assert!(
        first.status.success(),
        "mixed map failed: {}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert!(
        second.status.success(),
        "repeated mixed map failed: {}",
        String::from_utf8_lossy(&second.stderr)
    );
    assert!(first.stderr.is_empty());
    assert!(second.stderr.is_empty());
    assert_plain_report(&first_stdout);
    assert_eq!(
        first_stdout, second_stdout,
        "mixed-language map ordering must be deterministic"
    );
    assert_eq!(json["map"]["query_pack"], "mixed");
    assert_eq!(json["map"]["query_packs"]["javascript"], "javascript-v1");
    assert_eq!(json["map"]["query_packs"]["javascript_jsx"], "javascript-v1");
    assert_eq!(json["map"]["query_packs"]["typescript"], "typescript-v1");
    assert_eq!(json["map"]["query_packs"]["typescript_tsx"], "typescript-v1");
    assert_eq!(json["map"]["query_packs"]["python"], "python-v1");
    assert_eq!(json["map"]["query_packs"]["ruby"], "ruby-v1");
    assert_eq!(json["map"]["query_packs"]["go"], "go-v1");
    assert_eq!(json["map"]["query_packs"]["lua"], "lua-v1");
    assert_eq!(json["map"]["query_packs"]["zig"], "zig-v1");

    let files = json["map"]["files"].as_array().expect("mixed map files");
    for (path, language, extension) in [
        ("src/lib.rs", "rust", "rs"),
        ("src/module.js", "javascript", "js"),
        ("src/panel.jsx", "javascript_jsx", "jsx"),
        ("src/types.ts", "typescript", "ts"),
        ("src/component.tsx", "typescript_tsx", "tsx"),
        ("src/service.py", "python", "py"),
        ("src/service.rb", "ruby", "rb"),
        ("src/service.go", "go", "go"),
        ("src/service_test.go", "go", "go"),
        ("src/service.lua", "lua", "lua"),
        ("src/service.zig", "zig", "zig"),
        (".luacheckrc", "lua", ""),
        ("scripts/lua-tool", "lua", ""),
    ] {
        let file = files
            .iter()
            .find(|file| file["path"] == path)
            .unwrap_or_else(|| panic!("missing language fixture file {path}; files: {files:?}"));
        assert_eq!(file["language"], language);
        assert_eq!(file["extension"], extension);
        assert_eq!(file["status"], "complete");
        assert!(!file["symbols"].as_array().expect("symbols").is_empty());
    }
    assert_eq!(
        files
            .iter()
            .find(|file| file["path"] == "src/broken.js")
            .expect("malformed JavaScript file")["status"],
        "partial"
    );
    assert_eq!(
        files
            .iter()
            .find(|file| file["path"] == "src/broken.zig")
            .expect("malformed Zig file")["status"],
        "partial"
    );
    assert!(
        files
            .iter()
            .find(|file| file["path"] == "src/module.js")
            .expect("JavaScript file")["symbols"]
            .as_array()
            .expect("JavaScript symbols")
            .iter()
            .any(|symbol| symbol["name"] == "Widget" && symbol["kind"] == "class")
    );
    assert!(
        files
            .iter()
            .find(|file| file["path"] == "src/types.ts")
            .expect("TypeScript file")["symbols"]
            .as_array()
            .expect("TypeScript symbols")
            .iter()
            .any(|symbol| symbol["name"] == "User" && symbol["kind"] == "interface")
    );
    let python = files
        .iter()
        .find(|file| file["path"] == "src/service.py")
        .expect("Python file");
    assert!(
        python["symbols"]
            .as_array()
            .expect("Python symbols")
            .iter()
            .any(|symbol| {
                symbol["name"] == "Service" && symbol["kind"] == "class" && symbol["role"] == "definition"
            })
    );
    assert!(
        python["symbols"]
            .as_array()
            .expect("Python symbols")
            .iter()
            .any(|symbol| {
                symbol["name"] == "run"
                    && symbol["kind"] == "function"
                    && symbol["role"] == "definition"
                    && symbol["scope"] == serde_json::json!(["Service"])
            })
    );
    assert!(
        python["symbols"]
            .as_array()
            .expect("Python symbols")
            .iter()
            .any(|symbol| { symbol["name"] == "helper" && symbol["role"] == "reference" })
    );
    let ruby = files
        .iter()
        .find(|file| file["path"] == "src/service.rb")
        .expect("Ruby file");
    assert!(
        ruby["symbols"].as_array().expect("Ruby symbols").iter().any(|symbol| {
            symbol["name"] == "Billing" && symbol["kind"] == "module" && symbol["role"] == "definition"
        })
    );
    assert!(ruby["symbols"].as_array().expect("Ruby symbols").iter().any(|symbol| {
        symbol["name"] == "run"
            && symbol["kind"] == "method"
            && symbol["role"] == "definition"
            && symbol["scope"] == serde_json::json!(["Billing", "Service"])
    }));
    assert!(
        ruby["symbols"]
            .as_array()
            .expect("Ruby symbols")
            .iter()
            .any(|symbol| { symbol["name"] == "Service" && symbol["role"] == "reference" })
    );
    let go = files
        .iter()
        .find(|file| file["path"] == "src/service.go")
        .expect("Go file");
    assert!(go["symbols"].as_array().expect("Go symbols").iter().any(|symbol| {
        symbol["name"] == "Service"
            && symbol["kind"] == "struct"
            && symbol["role"] == "definition"
            && symbol["visibility"] == "public"
    }));
    assert!(go["symbols"].as_array().expect("Go symbols").iter().any(|symbol| {
        symbol["name"] == "NewService" && symbol["kind"] == "function" && symbol["role"] == "definition"
    }));
    let zig = files
        .iter()
        .find(|file| file["path"] == "src/service.zig")
        .expect("Zig file");
    assert!(zig["symbols"].as_array().expect("Zig symbols").iter().any(|symbol| {
        symbol["name"] == "Service"
            && symbol["kind"] == "struct"
            && symbol["role"] == "definition"
            && symbol["visibility"] == "public"
    }));
    assert!(zig["symbols"].as_array().expect("Zig symbols").iter().any(|symbol| {
        symbol["name"] == "Nested"
            && symbol["kind"] == "type"
            && symbol["role"] == "definition"
            && symbol["scope"] == serde_json::json!(["Service"])
    }));
    assert!(
        zig["limitations"]
            .as_array()
            .expect("Zig limitations")
            .iter()
            .any(|limitation| {
                limitation
                    .as_str()
                    .is_some_and(|value| value.contains("comptime evaluation"))
            })
    );
    let go_test = files
        .iter()
        .find(|file| file["path"] == "src/service_test.go")
        .expect("Go test file");
    assert!(
        go_test["symbols"]
            .as_array()
            .expect("Go test symbols")
            .iter()
            .any(|symbol| {
                symbol["name"] == "TestNewService" && symbol["kind"] == "function" && symbol["role"] == "definition"
            })
    );
    let lua = files
        .iter()
        .find(|file| file["path"] == "src/service.lua")
        .expect("Lua file");
    assert!(
        lua["symbols"].as_array().expect("Lua symbols").iter().any(|symbol| {
            symbol["name"] == "build" && symbol["kind"] == "function" && symbol["role"] == "definition"
        })
    );
    assert!(lua["symbols"].as_array().expect("Lua symbols").iter().any(|symbol| {
        symbol["name"] == "render"
            && symbol["kind"] == "method"
            && symbol["role"] == "definition"
            && symbol["scope"] == serde_json::json!(["M"])
    }));
    assert!(lua["symbols"].as_array().expect("Lua symbols").iter().any(|symbol| {
        symbol["name"] == "src.lua_helper" && symbol["kind"] == "import" && symbol["evidence"] == "import"
    }));

    for path in [
        "src/broken.py",
        "src/broken.rb",
        "src/broken.go",
        "src/broken.lua",
        "src/broken.zig",
    ] {
        let file = files
            .iter()
            .find(|file| file["path"] == path)
            .expect("malformed dynamic-language file");
        assert_eq!(file["status"], "partial");
        assert!(!file["limitations"].as_array().expect("file limitations").is_empty());
    }

    let omissions = json["map"]["omissions"].as_array().expect("mixed omissions");
    assert!(
        omissions
            .iter()
            .any(|omission| { omission["path"] == "src/ignored.js" && omission["reason"] == "ignored_untracked" })
    );
    assert!(
        omissions
            .iter()
            .any(|omission| { omission["path"] == "README.md" && omission["reason"] == "non_source" })
    );
    assert!(
        json["map"]["findings"]
            .as_array()
            .expect("mixed findings")
            .iter()
            .all(|finding| finding["kind"] != "query_error")
    );
    assert!(
        json["map"]["findings"]
            .as_array()
            .expect("mixed findings")
            .iter()
            .any(|finding| finding["kind"] == "parse_error" && finding["path"] == "src/broken.js")
    );

    let markdown = fixture.run(&["map", "--profile", "evidence"]);
    let markdown_stdout = stdout(&markdown);
    assert!(markdown.status.success());
    assert!(markdown.stderr.is_empty());
    assert!(markdown_stdout.contains("JavaScript files"));
    assert!(markdown_stdout.contains("JavaScript (JSX) files"));
    assert!(markdown_stdout.contains("TypeScript files"));
    assert!(markdown_stdout.contains("TypeScript (TSX) files"));
    assert!(markdown_stdout.contains("Python files"));
    assert!(markdown_stdout.contains("Ruby files"));
    assert!(markdown_stdout.contains("Go files"));
    assert!(markdown_stdout.contains("Lua files"));
    assert!(markdown_stdout.contains("Zig files"));
    assert!(markdown_stdout.contains("src/broken.py"));
    assert!(markdown_stdout.contains("Tree-sitter reported parse errors in this Python file"));
    assert!(markdown_stdout.contains("src/broken.rb"));
    assert!(markdown_stdout.contains("Tree-sitter reported parse errors in this Ruby file"));
    assert!(markdown_stdout.contains("src/broken.go"));
    assert!(markdown_stdout.contains("Tree-sitter reported parse errors in this Go file"));
    assert!(markdown_stdout.contains("src/broken.lua"));
    assert!(markdown_stdout.contains("Tree-sitter reported parse errors in this Lua file"));
    assert!(markdown_stdout.contains("src/broken.zig"));
    assert!(markdown_stdout.contains("Tree-sitter reported parse errors in this Zig file"));
    assert!(markdown_stdout.contains("dynamic `require`"));
    assert!(markdown_stdout.contains("comptime evaluation"));
    assert!(markdown_stdout.contains("query-pack provenance"));
    assert_plain_report(&markdown_stdout);
}

#[test]
fn go_map_supports_focus_package_edges_ambiguity_provenance_and_reading_plans() {
    let fixture = MixedMapFixtureRepository::new();
    let map = fixture.run(&[
        "map",
        "--profile",
        "evidence",
        "--no-cache",
        "--focus-path",
        "src/service.go",
        "--json",
    ]);
    let value: Value = serde_json::from_slice(&map.stdout).expect("valid focused Go map JSON");
    assert!(
        map.status.success(),
        "focused Go map failed: {}",
        String::from_utf8_lossy(&map.stderr)
    );
    assert!(map.stderr.is_empty());
    assert_eq!(value["map"]["query_packs"]["go"], "go-v1");
    assert_eq!(value["provenance"]["languages"]["go"]["grammar"], "tree-sitter-go");
    assert_eq!(value["provenance"]["languages"]["go"]["query_pack"], "go-v1");
    assert_eq!(value["map"]["ranking"][0]["path"], "src/service.go");
    let go_file = value["map"]["files"]
        .as_array()
        .expect("focused map files")
        .iter()
        .find(|file| file["path"] == "src/service.go")
        .expect("focused Go file");
    assert!(
        go_file["symbols"]
            .as_array()
            .expect("focused Go symbols")
            .iter()
            .any(|symbol| {
                symbol["name"] == "Run"
                    && symbol["kind"] == "method"
                    && symbol["role"] == "definition"
                    && symbol["scope"] == serde_json::json!(["fixture", "Service"])
            })
    );
    assert!(value["map"]["edges"].as_array().expect("Go edges").iter().any(|edge| {
        edge["source"] == "src/service_test.go"
            && edge["target"] == "src/service.go"
            && edge["symbol"] == "NewService"
            && edge["resolution_reason"] == "same_module"
    }));
    assert!(
        value["map"]["findings"]
            .as_array()
            .expect("Go findings")
            .iter()
            .any(|finding| {
                finding["kind"] == "ambiguous_reference"
                    && finding["detail"].as_str().unwrap_or_default().contains("Duplicate")
            })
    );

    let orientation = fixture.run(&["orient", "--no-cache", "--focus-path", "src/service.go", "--json"]);
    let orientation_value: Value =
        serde_json::from_slice(&orientation.stdout).expect("valid focused Go orientation JSON");
    assert!(orientation.status.success());
    assert!(
        orientation_recommendations(&orientation_value)
            .iter()
            .any(|recommendation| {
                recommendation["path"] == "src/service.go"
                    && recommendation["evidence_kinds"]
                        .as_array()
                        .is_some_and(|kinds| kinds.iter().any(|kind| kind == "source_map"))
            })
    );

    let first_cached = fixture.run(&["map", "--focus-path", "scripts/lua-tool", "--json"]);
    let second_cached = fixture.run(&["map", "--focus-path", "scripts/lua-tool", "--json"]);
    let cached_value: Value = serde_json::from_slice(&second_cached.stdout).expect("valid cached Lua map JSON");
    assert!(first_cached.status.success());
    assert!(second_cached.status.success());
    assert_eq!(cached_value["provenance"]["languages"]["lua"]["query_pack"], "lua-v1");
    assert!(cached_value["map"]["cache"]["hits"].as_u64().unwrap_or_default() > 0);
}

#[test]
fn lua_map_supports_literal_require_edges_provenance_focus_and_reading_plans() {
    let fixture = MixedMapFixtureRepository::new();
    let map = fixture.run(&[
        "map",
        "--profile",
        "evidence",
        "--no-cache",
        "--focus-path",
        "src/service.lua",
        "--json",
    ]);
    let value: Value = serde_json::from_slice(&map.stdout).expect("valid focused Lua map JSON");
    assert!(
        map.status.success(),
        "focused Lua map failed: {}",
        String::from_utf8_lossy(&map.stderr)
    );
    assert!(map.stderr.is_empty());
    assert_eq!(value["map"]["query_packs"]["lua"], "lua-v1");
    assert_eq!(value["provenance"]["languages"]["lua"]["grammar"], "tree-sitter-lua");
    assert_eq!(value["provenance"]["languages"]["lua"]["grammar_version"], "0.5.0");
    assert_eq!(value["provenance"]["languages"]["lua"]["query_pack"], "lua-v1");
    assert_eq!(value["map"]["ranking"][0]["path"], "src/service.lua");
    assert!(value["map"]["edges"].as_array().expect("Lua edges").iter().any(|edge| {
        edge["source"] == "src/service.lua"
            && edge["target"] == "src/lua_helper.lua"
            && edge["symbol"] == "transform"
            && edge["resolution_reason"] == "imported_module"
    }));
    assert!(
        value["map"]["edges"]
            .as_array()
            .expect("Lua edges")
            .iter()
            .all(|edge| { !(edge["source"] == "src/duplicate_use.lua" && edge["symbol"] == "duplicate") })
    );

    let orientation = fixture.run(&["orient", "--no-cache", "--focus-path", "src/service.lua", "--json"]);
    let orientation_value: Value =
        serde_json::from_slice(&orientation.stdout).expect("valid focused Lua orientation JSON");
    assert!(orientation.status.success());
    assert!(
        orientation_recommendations(&orientation_value)
            .iter()
            .any(|recommendation| {
                recommendation["path"] == "src/service.lua"
                    && recommendation["evidence_kinds"]
                        .as_array()
                        .is_some_and(|kinds| kinds.iter().any(|kind| kind == "source_map"))
            })
    );
}

#[test]
fn zig_map_supports_literal_import_edges_provenance_focus_and_reading_plans() {
    let fixture = MixedMapFixtureRepository::new();
    let map = fixture.run(&[
        "map",
        "--profile",
        "evidence",
        "--no-cache",
        "--focus-path",
        "src/service.zig",
        "--json",
    ]);
    let value: Value = serde_json::from_slice(&map.stdout).expect("valid focused Zig map JSON");
    assert!(
        map.status.success(),
        "focused Zig map failed: {}",
        String::from_utf8_lossy(&map.stderr)
    );
    assert!(map.stderr.is_empty());
    assert_eq!(value["map"]["query_packs"]["zig"], "zig-v1");
    assert_eq!(value["provenance"]["languages"]["zig"]["grammar"], "tree-sitter-zig");
    assert_eq!(value["provenance"]["languages"]["zig"]["grammar_version"], "1.1.2");
    assert_eq!(value["provenance"]["languages"]["zig"]["query_pack"], "zig-v1");
    assert_eq!(value["map"]["ranking"][0]["path"], "src/service.zig");
    let zig_file = value["map"]["files"]
        .as_array()
        .expect("focused map files")
        .iter()
        .find(|file| file["path"] == "src/service.zig")
        .expect("focused Zig file");
    assert!(
        zig_file["symbols"]
            .as_array()
            .expect("focused Zig symbols")
            .iter()
            .any(|symbol| {
                symbol["name"] == "service creates a value"
                    && symbol["kind"] == "function"
                    && symbol["role"] == "definition"
            })
    );
    assert!(value["map"]["edges"].as_array().expect("Zig edges").iter().any(|edge| {
        edge["source"] == "src/service.zig"
            && edge["target"] == "src/zig_helper.zig"
            && edge["symbol"] == "render"
            && edge["resolution_reason"] == "imported_module"
    }));

    let orientation = fixture.run(&["orient", "--no-cache", "--focus-path", "src/service.zig", "--json"]);
    let orientation_value: Value =
        serde_json::from_slice(&orientation.stdout).expect("valid focused Zig orientation JSON");
    assert!(orientation.status.success());
    assert!(
        orientation_recommendations(&orientation_value)
            .iter()
            .any(|recommendation| {
                recommendation["path"] == "src/service.zig"
                    && recommendation["evidence_kinds"]
                        .as_array()
                        .is_some_and(|kinds| kinds.iter().any(|kind| kind == "source_map"))
            })
    );

    let first_cached = fixture.run(&["map", "--focus-path", "src/service.zig", "--json"]);
    let second_cached = fixture.run(&["map", "--focus-path", "src/service.zig", "--json"]);
    let cached_value: Value = serde_json::from_slice(&second_cached.stdout).expect("valid cached Zig map JSON");
    assert!(first_cached.status.success());
    assert!(second_cached.status.success());
    assert_eq!(cached_value["provenance"]["languages"]["zig"]["query_pack"], "zig-v1");
    assert!(cached_value["map"]["cache"]["hits"].as_u64().unwrap_or_default() > 0);
}

#[test]
fn java_and_c_sharp_map_is_first_class_and_preserves_visibility_duplicates_and_limitations() {
    let fixture = JavaCSharpMapFixtureRepository::new();
    let first = fixture.run(&["map", "--no-cache", "--json"]);
    let second = fixture.run(&["map", "--no-cache", "--json"]);
    let first_stdout = stdout(&first);
    let second_stdout = stdout(&second);
    let json: Value = serde_json::from_str(&first_stdout).expect("valid Java and C# map JSON");

    assert!(
        first.status.success(),
        "map failed: {}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert!(
        second.status.success(),
        "repeated map failed: {}",
        String::from_utf8_lossy(&second.stderr)
    );
    assert!(first.stderr.is_empty());
    assert!(second.stderr.is_empty());
    assert_plain_report(&first_stdout);
    assert_eq!(
        first_stdout, second_stdout,
        "Java and C# map ordering must be deterministic"
    );
    assert_eq!(json["map"]["query_pack"], "mixed");
    assert_eq!(json["map"]["query_packs"]["java"], "java-v1");
    assert_eq!(json["map"]["query_packs"]["c_sharp"], "c-sharp-v1");

    let files = json["map"]["files"].as_array().expect("Java and C# map files");
    for (path, language, extension) in [
        ("src/service.java", "java", "java"),
        ("src/consumer.java", "java", "java"),
        ("src/service.cs", "c_sharp", "cs"),
    ] {
        let file = files
            .iter()
            .find(|file| file["path"] == path)
            .expect("first-class language fixture file");
        assert_eq!(file["language"], language);
        assert_eq!(file["extension"], extension);
        assert_eq!(file["status"], "complete");
        assert!(!file["symbols"].as_array().expect("symbols").is_empty());
    }
    let untracked = files
        .iter()
        .find(|file| file["path"] == "src/untracked.java")
        .expect("untracked Java file");
    assert_eq!(untracked["worktree_state"], "untracked");

    let java = files
        .iter()
        .find(|file| file["path"] == "src/service.java")
        .expect("Java file");
    assert!(
        java["symbols"].as_array().expect("Java symbols").iter().any(|symbol| {
            symbol["name"] == "example" && symbol["kind"] == "module" && symbol["role"] == "definition"
        })
    );
    assert!(
        java["symbols"].as_array().expect("Java symbols").iter().any(|symbol| {
            symbol["name"] == "Service" && symbol["kind"] == "class" && symbol["role"] == "definition"
        })
    );
    assert!(
        java["symbols"].as_array().expect("Java symbols").iter().any(|symbol| {
            symbol["name"] == "Hidden" && symbol["kind"] == "class" && symbol["role"] == "definition"
        })
    );
    assert!(java["symbols"].as_array().expect("Java symbols").iter().any(|symbol| {
        symbol["name"] == "run"
            && symbol["kind"] == "method"
            && symbol["role"] == "definition"
            && symbol["location"]["start"]["line"].as_u64().unwrap_or(0) > 0
            && symbol["context"]
                .as_str()
                .unwrap_or_default()
                .starts_with("public Result run")
    }));
    assert!(
        java["symbols"]
            .as_array()
            .expect("Java symbols")
            .iter()
            .any(|symbol| { symbol["name"] == "Input" && symbol["kind"] == "type" && symbol["role"] == "reference" })
    );

    let c_sharp = files
        .iter()
        .find(|file| file["path"] == "src/service.cs")
        .expect("C# file");
    assert!(c_sharp["symbols"].as_array().expect("C# symbols").iter().any(|symbol| {
        symbol["name"] == "Example.App" && symbol["kind"] == "module" && symbol["role"] == "definition"
    }));
    assert!(
        c_sharp["symbols"].as_array().expect("C# symbols").iter().any(|symbol| {
            symbol["name"] == "Service" && symbol["kind"] == "class" && symbol["role"] == "definition"
        })
    );
    assert!(
        c_sharp["symbols"].as_array().expect("C# symbols").iter().any(|symbol| {
            symbol["name"] == "Value" && symbol["kind"] == "struct" && symbol["role"] == "definition"
        })
    );
    assert!(
        c_sharp["symbols"].as_array().expect("C# symbols").iter().any(|symbol| {
            symbol["name"] == "Hidden" && symbol["kind"] == "class" && symbol["role"] == "definition"
        })
    );
    assert!(
        c_sharp["symbols"].as_array().expect("C# symbols").iter().any(|symbol| {
            symbol["name"] == "Execute" && symbol["kind"] == "method" && symbol["role"] == "reference"
        })
    );

    let broken = files
        .iter()
        .find(|file| file["path"] == "src/broken.cs")
        .expect("malformed C# file");
    assert_eq!(broken["status"], "partial");
    assert!(!broken["limitations"].as_array().expect("C# limitations").is_empty());

    let omissions = json["map"]["omissions"].as_array().expect("map omissions");
    assert!(
        omissions
            .iter()
            .any(|omission| { omission["path"] == "src/ignored.java" && omission["reason"] == "ignored_untracked" })
    );
    assert!(
        omissions
            .iter()
            .any(|omission| { omission["path"] == "README.md" && omission["reason"] == "non_source" })
    );
    assert!(
        !json["map"]["findings"]
            .as_array()
            .expect("map findings")
            .iter()
            .any(|finding| {
                finding["kind"] == "ambiguous_reference"
                    && finding["detail"].as_str().unwrap_or_default().contains("Service")
            })
    );

    let markdown = fixture.run(&["map"]);
    let markdown_stdout = stdout(&markdown);
    assert!(markdown.status.success());
    assert!(markdown.stderr.is_empty());
    assert!(markdown_stdout.contains("Java files"));
    assert!(markdown_stdout.contains("C# files"));
    assert!(markdown_stdout.contains("src/broken.cs"));
    assert!(markdown_stdout.contains("Tree-sitter reported parse errors in this C# file"));
    assert_plain_report(&markdown_stdout);
}
