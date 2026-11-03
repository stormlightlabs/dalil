use super::*;

#[test]
fn scope_and_snippet_are_compact_and_one_based() {
    let source = b"mod outer { fn parse(value: usize) { let _ = value; } }";
    let ParsedSource { symbols, findings, status, limitations } = parse_source(source, &RUST_SUPPORT);

    assert!(findings.is_empty());
    assert_eq!(status, FileAnalysisStatus::Complete);
    assert!(limitations.is_empty());
    let parse = symbols
        .iter()
        .find(|symbol| symbol.name == "parse" && symbol.role == SymbolRole::Definition)
        .expect("function definition");
    assert_eq!(parse.location.start.line, 1);
    assert_eq!(parse.location.start.column, 16);
    assert_eq!(parse.scope, vec!["outer"]);
    assert!(parse.context.starts_with("fn parse(value: usize)"));
}

#[test]
fn malformed_rust_is_partial_with_an_explicit_parse_finding() {
    let ParsedSource { symbols, findings, status, limitations } = parse_source(b"fn broken( {", &RUST_SUPPORT);

    assert_eq!(status, FileAnalysisStatus::Partial);
    assert!(!symbols.is_empty());
    assert!(!findings.is_empty());
    assert!(limitations.iter().any(|limitation| limitation.contains("parse errors")));
}

#[test]
fn javascript_query_pack_extracts_module_class_and_function_symbols() {
    let source = br#"
            import { helper } from "./helper.js";
            export function build(value) { return new Widget(value, helper); }
            export class Widget { render() { return helper(); } }
        "#;
    let ParsedSource { symbols, findings, status, limitations } = parse_source(source, &JAVASCRIPT_SUPPORT);

    assert_eq!(status, FileAnalysisStatus::Complete, "{findings:?}");
    assert!(limitations.is_empty());
    assert!(
        findings
            .iter()
            .all(|finding| finding.kind != MapFindingKind::QueryError)
    );
    assert!(symbols.iter().any(|symbol| {
        symbol.name == "build" && symbol.kind == SymbolKind::Function && symbol.role == SymbolRole::Definition
    }));
    assert!(symbols.iter().any(|symbol| {
        symbol.name == "Widget" && symbol.kind == SymbolKind::Class && symbol.role == SymbolRole::Definition
    }));
    assert!(symbols.iter().any(|symbol| {
        symbol.name == "render" && symbol.kind == SymbolKind::Method && symbol.role == SymbolRole::Definition
    }));
    let render = symbols
        .iter()
        .find(|symbol| symbol.name == "render" && symbol.role == SymbolRole::Definition)
        .expect("method definition");
    assert_eq!(render.scope, vec!["Widget"]);
    assert!(render.location.start.line > 0);
    assert!(render.context.starts_with("render()"));
    assert!(
        symbols
            .iter()
            .any(|symbol| symbol.name == "helper" && symbol.role == SymbolRole::Reference)
    );
}

#[test]
fn typescript_and_tsx_query_packs_extract_typed_symbols_without_cross_language_loss() {
    let typescript = br#"
            import { helper } from "./helper";
            export interface User { name: string; }
            export class Service { run(user: User) { return helper(user.name); } }
            export function create(user: User): Service { return new Service(); }
        "#;
    let tsx = br#"
            export function View(props: { label: string }) { return <button>{props.label}</button>; }
        "#;

    let parsed_typescript = parse_source(typescript, &TYPESCRIPT_SUPPORT);
    let parsed_tsx = parse_source(tsx, &TYPESCRIPT_TSX_SUPPORT);
    assert_eq!(
        parsed_typescript.status,
        FileAnalysisStatus::Complete,
        "{parsed_typescript:?}"
    );
    assert_eq!(parsed_tsx.status, FileAnalysisStatus::Complete, "{parsed_tsx:?}");
    assert!(
        parsed_typescript
            .findings
            .iter()
            .all(|finding| finding.kind != MapFindingKind::QueryError)
    );
    assert!(
        parsed_tsx
            .findings
            .iter()
            .all(|finding| finding.kind != MapFindingKind::QueryError)
    );
    assert!(
        parsed_typescript
            .symbols
            .iter()
            .any(|symbol| symbol.name == "User" && symbol.kind == SymbolKind::Interface)
    );
    assert!(
        parsed_typescript
            .symbols
            .iter()
            .any(|symbol| symbol.name == "Service" && symbol.kind == SymbolKind::Class)
    );
    assert!(
        parsed_typescript
            .symbols
            .iter()
            .any(|symbol| symbol.name == "helper" && symbol.role == SymbolRole::Reference)
    );
    let run = parsed_typescript
        .symbols
        .iter()
        .find(|symbol| symbol.name == "run" && symbol.role == SymbolRole::Definition)
        .expect("method definition");
    assert_eq!(run.scope, vec!["Service"]);
    assert!(run.context.starts_with("run(user: User)"));
    let view = parsed_tsx
        .symbols
        .iter()
        .find(|symbol| symbol.name == "View" && symbol.kind == SymbolKind::Function)
        .expect("TSX function definition");
    assert!(view.location.start.line > 0);
    assert!(view.context.starts_with("function View"));
}

#[test]
fn python_and_ruby_query_packs_extract_definitions_references_and_nested_scopes() {
    let python = br#"
class Service:
    def run(self, value):
        return helper(value)

def create(value):
    return Service().run(value)
"#;
    let ruby = br#"
module Billing
  class Service
    def run(value)
      helper(value)
    end
  end
end

def build
  Service.new
end
"#;

    let parsed_python = parse_source(python, &PYTHON_SUPPORT);
    let parsed_ruby = parse_source(ruby, &RUBY_SUPPORT);
    assert_eq!(parsed_python.status, FileAnalysisStatus::Complete, "{parsed_python:?}");
    assert_eq!(parsed_ruby.status, FileAnalysisStatus::Complete, "{parsed_ruby:?}");
    assert!(parsed_python.findings.is_empty(), "{parsed_python:?}");
    assert!(parsed_ruby.findings.is_empty(), "{parsed_ruby:?}");

    let python_run = parsed_python
        .symbols
        .iter()
        .find(|symbol| symbol.name == "run" && symbol.role == SymbolRole::Definition)
        .expect("Python method definition");
    assert_eq!(python_run.kind, SymbolKind::Function);
    assert_eq!(python_run.scope, vec!["Service"]);
    assert!(python_run.context.starts_with("def run(self, value):"));
    assert!(
        parsed_python
            .symbols
            .iter()
            .any(|symbol| { symbol.name == "helper" && symbol.role == SymbolRole::Reference })
    );

    let ruby_run = parsed_ruby
        .symbols
        .iter()
        .find(|symbol| symbol.name == "run" && symbol.role == SymbolRole::Definition)
        .expect("Ruby method definition");
    assert_eq!(ruby_run.kind, SymbolKind::Method);
    assert_eq!(ruby_run.scope, vec!["Billing", "Service"]);
    assert!(ruby_run.context.starts_with("def run(value)"));
    assert!(
        parsed_ruby
            .symbols
            .iter()
            .any(|symbol| { symbol.name == "Service" && symbol.role == SymbolRole::Reference })
    );
}

#[test]
fn java_and_c_sharp_query_packs_extract_types_scopes_and_references() {
    let java = br#"
package com.example;
import java.util.List;

public class Service extends BaseService implements Runner {
    private class Hidden {}
    private Helper helper;

    public Result run(Input input) {
        return new Result(helper(input));
    }

    private Result helper(Input input) {
        return input.result();
    }
}

interface Runner {}
"#;
    let c_sharp = br#"
using System;

namespace Example.App {
    public class Service : BaseService, IRunner {
        private class Hidden {}
        private Helper helper;

        public Result Run(Input input) {
            helper.Execute(input);
            return new Result();
        }
    }

    public struct Value {}
    public interface IRunner {}
}
"#;

    let parsed_java = parse_source(java, &JAVA_SUPPORT);
    let parsed_c_sharp = parse_source(c_sharp, &C_SHARP_SUPPORT);
    assert_eq!(parsed_java.status, FileAnalysisStatus::Complete, "{parsed_java:?}");
    assert_eq!(
        parsed_c_sharp.status,
        FileAnalysisStatus::Complete,
        "{parsed_c_sharp:?}"
    );
    assert!(parsed_java.findings.is_empty(), "{parsed_java:?}");
    assert!(parsed_c_sharp.findings.is_empty(), "{parsed_c_sharp:?}");

    assert!(parsed_java.symbols.iter().any(|symbol| {
        symbol.name == "com.example" && symbol.kind == SymbolKind::Module && symbol.role == SymbolRole::Definition
    }));
    assert!(parsed_java.symbols.iter().any(|symbol| {
        symbol.name == "Service" && symbol.kind == SymbolKind::Class && symbol.role == SymbolRole::Definition
    }));
    assert!(parsed_java.symbols.iter().any(|symbol| {
        symbol.name == "Hidden" && symbol.kind == SymbolKind::Class && symbol.role == SymbolRole::Definition
    }));
    assert!(parsed_java.symbols.iter().any(|symbol| {
        symbol.name == "helper" && symbol.kind == SymbolKind::Field && symbol.role == SymbolRole::Definition
    }));
    let java_run = parsed_java
        .symbols
        .iter()
        .find(|symbol| symbol.name == "run" && symbol.role == SymbolRole::Definition)
        .expect("Java method definition");
    assert_eq!(java_run.scope, vec!["Service"]);
    assert!(java_run.context.starts_with("public Result run(Input input)"));
    assert!(
        parsed_java
            .symbols
            .iter()
            .any(|symbol| symbol.name == "Input" && symbol.role == SymbolRole::Reference)
    );
    assert!(parsed_java.symbols.iter().any(|symbol| symbol.name == "helper"
        && symbol.kind == SymbolKind::Method
        && symbol.role == SymbolRole::Reference));

    assert!(parsed_c_sharp.symbols.iter().any(|symbol| {
        symbol.name == "Example.App" && symbol.kind == SymbolKind::Module && symbol.role == SymbolRole::Definition
    }));
    assert!(parsed_c_sharp.symbols.iter().any(|symbol| {
        symbol.name == "Service" && symbol.kind == SymbolKind::Class && symbol.role == SymbolRole::Definition
    }));
    assert!(parsed_c_sharp.symbols.iter().any(|symbol| {
        symbol.name == "Value" && symbol.kind == SymbolKind::Struct && symbol.role == SymbolRole::Definition
    }));
    assert!(parsed_c_sharp.symbols.iter().any(|symbol| {
        symbol.name == "Hidden" && symbol.kind == SymbolKind::Class && symbol.role == SymbolRole::Definition
    }));
    let c_sharp_run = parsed_c_sharp
        .symbols
        .iter()
        .find(|symbol| symbol.name == "Run" && symbol.role == SymbolRole::Definition)
        .expect("C# method definition");
    assert_eq!(c_sharp_run.scope, vec!["Example.App", "Service"]);
    assert!(c_sharp_run.context.starts_with("public Result Run(Input input)"));
    assert!(
        parsed_c_sharp
            .symbols
            .iter()
            .any(|symbol| symbol.name == "Helper" && symbol.role == SymbolRole::Reference)
    );
    assert!(parsed_c_sharp.symbols.iter().any(|symbol| symbol.name == "Execute"
        && symbol.kind == SymbolKind::Method
        && symbol.role == SymbolRole::Reference));
}

#[test]
fn go_query_pack_extracts_packages_declarations_references_visibility_and_scopes() {
    let source = br#"
package service

import (
    "fmt"
    api "example.com/project/client"
)

type Identifier = string
type Box[T any] struct {
    Value T
    hidden int
}
type Runner interface {
    Run(Identifier) error
    hiddenRun()
}

const ExportedConstant = 1
const localConstant = 2
var ExportedVariable Identifier
var localVariable Identifier

func Build[T any](value T) *Box[T] {
    var LocalOnly Identifier
    LocalShort := value
    fmt.Println(value)
    helper()
    return &Box[T]{Value: value}
}

func helper() {}

func (box *Box[T]) Process(input Identifier) {
    api.Send(input)
    helper()
}
"#;
    let parsed = parse_source(source, &GO_SUPPORT);

    assert_eq!(parsed.status, FileAnalysisStatus::Complete, "{parsed:?}");
    assert!(parsed.findings.is_empty(), "{parsed:?}");
    for (name, kind) in [
        ("service", SymbolKind::Module),
        ("fmt", SymbolKind::Import),
        ("api", SymbolKind::Import),
        ("Identifier", SymbolKind::Type),
        ("Box", SymbolKind::Struct),
        ("Runner", SymbolKind::Interface),
        ("Value", SymbolKind::Field),
        ("Run", SymbolKind::Method),
        ("ExportedConstant", SymbolKind::Const),
        ("ExportedVariable", SymbolKind::Variable),
        ("Build", SymbolKind::Function),
        ("Process", SymbolKind::Method),
    ] {
        assert!(
            parsed
                .symbols
                .iter()
                .any(|symbol| { symbol.name == name && symbol.kind == kind && symbol.role == SymbolRole::Definition }),
            "missing {kind:?} definition {name}: {parsed:?}"
        );
    }

    let build = parsed
        .symbols
        .iter()
        .find(|symbol| symbol.name == "Build" && symbol.role == SymbolRole::Definition)
        .expect("Go function definition");
    assert_eq!(build.scope, vec!["service"]);
    assert_eq!(build.visibility, SymbolVisibility::Public);
    assert!(build.context.starts_with("func Build[T any]"));

    let process = parsed
        .symbols
        .iter()
        .find(|symbol| symbol.name == "Process" && symbol.role == SymbolRole::Definition)
        .expect("Go method definition");
    assert_eq!(process.scope, vec!["service", "Box"]);
    assert_eq!(process.visibility, SymbolVisibility::Public);

    let hidden = parsed
        .symbols
        .iter()
        .find(|symbol| symbol.name == "hidden" && symbol.role == SymbolRole::Definition)
        .expect("unexported Go field");
    assert_eq!(hidden.scope, vec!["service", "Box"]);
    assert_eq!(hidden.visibility, SymbolVisibility::Internal);
    let local = parsed
        .symbols
        .iter()
        .find(|symbol| symbol.name == "LocalOnly" && symbol.role == SymbolRole::Definition)
        .expect("function-local Go variable");
    assert_eq!(local.visibility, SymbolVisibility::Internal);
    let local_short = parsed
        .symbols
        .iter()
        .find(|symbol| symbol.name == "LocalShort" && symbol.role == SymbolRole::Definition)
        .expect("short Go variable declaration");
    assert_eq!(local_short.kind, SymbolKind::Variable);
    assert_eq!(local_short.visibility, SymbolVisibility::Internal);

    for (name, evidence) in [
        ("Println", SymbolEvidence::Call),
        ("Send", SymbolEvidence::Call),
        ("helper", SymbolEvidence::Call),
        ("Identifier", SymbolEvidence::TypeReference),
    ] {
        assert!(
            parsed.symbols.iter().any(|symbol| {
                symbol.name == name && symbol.role == SymbolRole::Reference && symbol.evidence == evidence
            }),
            "missing {evidence:?} reference {name}: {parsed:?}"
        );
    }
}

#[test]
fn malformed_go_is_partial_and_test_declarations_remain_available() {
    let valid = parse_source(
        b"package service\nfunc TestBuild(t *testing.T) { Build(1) }\n",
        &GO_SUPPORT,
    );
    assert_eq!(valid.status, FileAnalysisStatus::Complete, "{valid:?}");
    assert!(valid.symbols.iter().any(|symbol| {
        symbol.name == "TestBuild" && symbol.kind == SymbolKind::Function && symbol.role == SymbolRole::Definition
    }));

    let malformed = parse_source(b"package service\nfunc Broken( {\n", &GO_SUPPORT);
    assert_eq!(malformed.status, FileAnalysisStatus::Partial);
    assert!(
        malformed
            .findings
            .iter()
            .any(|finding| finding.kind == MapFindingKind::ParseError)
    );
    assert!(
        malformed
            .limitations
            .iter()
            .any(|limitation| limitation.contains("Go file"))
    );
}

#[test]
fn lua_query_pack_extracts_modules_declarations_references_and_scopes() {
    let source = br#"
local helper = require("app.helper")
local M = { version = 1 }
local local_value = 2
local declared_only
global_value = 3

local function build(value)
    local nested = value
    return helper.transform(nested)
end

function M.run(value)
    return build(value)
end

function M:render()
    return self:run(global_value)
end

M.assigned = function(value)
    return value
end

return {
    create = build,
    start = function() return M:render() end,
}
"#;
    let parsed = parse_source(source, &LUA_SUPPORT);

    assert_eq!(parsed.status, FileAnalysisStatus::Complete, "{parsed:?}");
    assert!(parsed.findings.is_empty(), "{parsed:?}");
    for (name, kind) in [
        ("app.helper", SymbolKind::Import),
        ("helper", SymbolKind::Variable),
        ("local_value", SymbolKind::Variable),
        ("declared_only", SymbolKind::Variable),
        ("global_value", SymbolKind::Variable),
        ("build", SymbolKind::Function),
        ("run", SymbolKind::Function),
        ("render", SymbolKind::Method),
        ("assigned", SymbolKind::Function),
        ("version", SymbolKind::Field),
        ("create", SymbolKind::Field),
        ("start", SymbolKind::Function),
    ] {
        assert!(
            parsed
                .symbols
                .iter()
                .any(|symbol| { symbol.name == name && symbol.kind == kind && symbol.role == SymbolRole::Definition }),
            "missing {kind:?} definition {name}: {parsed:?}"
        );
    }

    let build = parsed
        .symbols
        .iter()
        .find(|symbol| symbol.name == "build" && symbol.kind == SymbolKind::Function)
        .expect("local Lua function");
    assert_eq!(build.visibility, SymbolVisibility::Internal);
    let nested = parsed
        .symbols
        .iter()
        .find(|symbol| symbol.name == "nested" && symbol.role == SymbolRole::Definition)
        .expect("nested local variable");
    assert_eq!(nested.scope, vec!["build"]);
    let run = parsed
        .symbols
        .iter()
        .find(|symbol| symbol.name == "run" && symbol.role == SymbolRole::Definition)
        .expect("dot method definition");
    assert_eq!(run.scope, vec!["M"]);
    let render = parsed
        .symbols
        .iter()
        .find(|symbol| symbol.name == "render" && symbol.role == SymbolRole::Definition)
        .expect("colon method definition");
    assert_eq!(render.scope, vec!["M"]);
    for name in ["transform", "build", "run", "render"] {
        assert!(
            parsed
                .symbols
                .iter()
                .any(|symbol| symbol.name == name && symbol.role == SymbolRole::Reference),
            "missing reference {name}: {parsed:?}"
        );
    }
    assert!(
        parsed
            .limitations
            .iter()
            .any(|limitation| { limitation.contains("dynamic `require`") && limitation.contains("metatable") })
    );
}

#[test]
fn malformed_lua_is_partial_and_dynamic_require_is_not_import_evidence() {
    let dynamic = parse_source(
        b"local name = 'app.helper'\nlocal helper = require(name)\n",
        &LUA_SUPPORT,
    );
    assert_eq!(dynamic.status, FileAnalysisStatus::Complete, "{dynamic:?}");
    assert!(dynamic.symbols.iter().all(|symbol| symbol.kind != SymbolKind::Import));

    let malformed = parse_source(b"local function broken(\n  return { value = 1\n", &LUA_SUPPORT);
    assert_eq!(malformed.status, FileAnalysisStatus::Partial);
    assert!(
        malformed
            .findings
            .iter()
            .any(|finding| finding.kind == MapFindingKind::ParseError)
    );
}

#[test]
fn zig_query_pack_extracts_containers_tests_imports_references_visibility_and_scopes() {
    let source = br#"
const helper = @import("helper.zig");

pub const Api = struct {
    value: []const u8,
    pub const Nested = union(enum) {
        text: []const u8,
        code: u32,
    };

    pub fn build(comptime T: type, value: T) !Api {
        _ = value;
        return .{ .value = helper.render(@typeName(T)) };
    }

    fn hidden() void {}
};

test "Api build" {
    const api = try Api.build(u8, 1);
    _ = api.value;
}

const Duplicate = struct { value: u8 };
const Duplicate = enum { value };
"#;
    let parsed = parse_source(source, &ZIG_SUPPORT);

    assert_eq!(parsed.status, FileAnalysisStatus::Complete, "{parsed:?}");
    assert!(parsed.findings.is_empty(), "{parsed:?}");
    for (name, kind) in [
        ("helper.zig", SymbolKind::Import),
        ("Api", SymbolKind::Struct),
        ("Nested", SymbolKind::Type),
        ("value", SymbolKind::Field),
        ("text", SymbolKind::Field),
        ("build", SymbolKind::Function),
        ("hidden", SymbolKind::Function),
        ("Api build", SymbolKind::Function),
    ] {
        assert!(
            parsed
                .symbols
                .iter()
                .any(|symbol| symbol.name == name && symbol.kind == kind && symbol.role == SymbolRole::Definition),
            "missing {kind:?} definition {name}: {parsed:?}"
        );
    }

    let api = parsed
        .symbols
        .iter()
        .find(|symbol| symbol.name == "Api" && symbol.kind == SymbolKind::Struct)
        .expect("Zig container definition");
    assert_eq!(api.visibility, SymbolVisibility::Public);
    assert!(api.context.starts_with("pub const Api = struct"));
    assert_eq!(
        parsed
            .symbols
            .iter()
            .filter(|symbol| {
                symbol.name == "Api" && symbol.role == SymbolRole::Definition && symbol.location == api.location
            })
            .count(),
        1,
        "a Zig container is not also emitted as an untyped variable: {parsed:?}"
    );

    let nested = parsed
        .symbols
        .iter()
        .find(|symbol| symbol.name == "Nested" && symbol.kind == SymbolKind::Type)
        .expect("nested Zig container definition");
    assert_eq!(nested.scope, vec!["Api"]);
    assert_eq!(nested.visibility, SymbolVisibility::Public);

    let hidden = parsed
        .symbols
        .iter()
        .find(|symbol| symbol.name == "hidden" && symbol.kind == SymbolKind::Function)
        .expect("internal Zig function definition");
    assert_eq!(hidden.scope, vec!["Api"]);
    assert_eq!(hidden.visibility, SymbolVisibility::Internal);
    assert!(parsed.symbols.iter().all(|symbol| {
        !(symbol.name == "value" && symbol.role == SymbolRole::Definition && symbol.location.start.line == 12)
    }));

    for (name, evidence) in [
        ("render", SymbolEvidence::Call),
        ("Api", SymbolEvidence::TypeReference),
        ("T", SymbolEvidence::TypeReference),
        ("value", SymbolEvidence::MemberReference),
    ] {
        assert!(
            parsed.symbols.iter().any(|symbol| {
                symbol.name == name && symbol.role == SymbolRole::Reference && symbol.evidence == evidence
            }),
            "missing {evidence:?} reference {name}: {parsed:?}"
        );
    }
    assert!(parsed.limitations.iter().any(|limitation| {
        limitation.contains("literal `@import`")
            && limitation.contains("comptime evaluation")
            && limitation.contains("error-union flow")
    }));
    assert_eq!(
        parsed
            .symbols
            .iter()
            .filter(|symbol| symbol.name == "Duplicate" && symbol.role == SymbolRole::Definition)
            .count(),
        2,
        "duplicate Zig declarations retain distinct locations: {parsed:?}"
    );
}

#[test]
fn malformed_zig_is_partial_and_non_literal_import_is_not_import_evidence() {
    let non_literal = parse_source(
        b"const module_name = \"helper.zig\";\nconst helper = @import(module_name);\n",
        &ZIG_SUPPORT,
    );
    assert_eq!(non_literal.status, FileAnalysisStatus::Complete, "{non_literal:?}");
    assert!(
        non_literal
            .symbols
            .iter()
            .all(|symbol| symbol.kind != SymbolKind::Import)
    );

    let malformed = parse_source(b"pub fn Broken( {\n", &ZIG_SUPPORT);
    assert_eq!(malformed.status, FileAnalysisStatus::Partial);
    assert!(
        malformed
            .findings
            .iter()
            .any(|finding| finding.kind == MapFindingKind::ParseError)
    );
    assert!(
        malformed
            .limitations
            .iter()
            .any(|limitation| limitation.contains("Zig file"))
    );
}

#[test]
fn zig_symbol_limit_counts_unique_symbols_after_query_overlap_is_removed() {
    let source = b"const Api = struct { marker: u8, };\nconst value: Api = undefined;\n";
    let mut analyzer = LanguageAnalyzer::new(&ZIG_SUPPORT);
    let limits = ReportLimits { max_symbols_per_file: 4, ..ReportLimits::for_profile(AnalysisProfile::Evidence) };
    let parsed = parse_source_with_analyzer(source, &mut analyzer, &limits);

    assert_eq!(parsed.status, FileAnalysisStatus::Complete, "{parsed:?}");
    assert_eq!(parsed.symbols.len(), 4, "{parsed:?}");
    assert!(
        parsed
            .limitations
            .iter()
            .all(|limitation| !limitation.contains("symbol limit"))
    );
    assert!(parsed.symbols.iter().any(|symbol| {
        symbol.name == "Api" && symbol.kind == SymbolKind::Struct && symbol.role == SymbolRole::Definition
    }));
    assert!(parsed.symbols.iter().any(|symbol| {
        symbol.name == "value" && symbol.kind == SymbolKind::Variable && symbol.role == SymbolRole::Definition
    }));
}

#[test]
fn javascript_typescript_and_jsx_extensions_select_explicit_language_variants() {
    assert_eq!(
        support_for_path(Path::new("module.js")).unwrap().language,
        SourceLanguage::JavaScript
    );
    assert_eq!(
        support_for_path(Path::new("module.jsx")).unwrap().language,
        SourceLanguage::JavaScriptJsx
    );
    assert_eq!(
        support_for_path(Path::new("module.ts")).unwrap().language,
        SourceLanguage::TypeScript
    );
    assert_eq!(
        support_for_path(Path::new("module.tsx")).unwrap().language,
        SourceLanguage::TypeScriptTsx
    );
    assert_eq!(
        support_for_path(Path::new("module.py")).unwrap().language,
        SourceLanguage::Python
    );
    assert_eq!(
        support_for_path(Path::new("module.pyi")).unwrap().language,
        SourceLanguage::Python
    );
    assert_eq!(
        support_for_path(Path::new("Rakefile.rake")).unwrap().language,
        SourceLanguage::Ruby
    );
    assert_eq!(
        support_for_path(Path::new("Gemfile.gemspec")).unwrap().language,
        SourceLanguage::Ruby
    );
    assert_eq!(
        support_for_path(Path::new("Service.java")).unwrap().language,
        SourceLanguage::Java
    );
    assert_eq!(
        support_for_path(Path::new("Service.cs")).unwrap().language,
        SourceLanguage::CSharp
    );
    assert_eq!(
        support_for_path(Path::new("service_test.go")).unwrap().language,
        SourceLanguage::Go
    );
    assert_eq!(
        support_for_path(Path::new("module.lua")).unwrap().language,
        SourceLanguage::Lua
    );
    assert_eq!(
        support_for_path(Path::new("module.zig")).unwrap().language,
        SourceLanguage::Zig
    );
    assert_eq!(
        support_for_path(Path::new("package.rockspec")).unwrap().language,
        SourceLanguage::Lua
    );
    assert_eq!(
        support_for_path(Path::new(".luacheckrc")).unwrap().language,
        SourceLanguage::Lua
    );
    assert_eq!(
        support_for_path(Path::new(".busted")).unwrap().language,
        SourceLanguage::Lua
    );
    assert_eq!(
        lua_support_for_entry_source(Path::new("bin/tool"), "#!/usr/bin/env lua\nprint('ok')")
            .unwrap()
            .language,
        SourceLanguage::Lua
    );
    assert_eq!(
        lua_support_for_entry_source(Path::new("scripts/tool"), "#!/usr/bin/luajit\nprint('ok')")
            .unwrap()
            .language,
        SourceLanguage::Lua
    );
    assert!(lua_support_for_entry_source(Path::new("bin/tool"), "#!/bin/sh\necho ok").is_none());
    assert!(lua_support_for_entry_source(Path::new("tool"), "#!/usr/bin/env lua\nprint('ok')").is_none());
    assert!(support_for_path(Path::new("module.vue")).is_none());
}

#[test]
fn lexical_edges_require_explicit_same_file_or_import_evidence_and_group_ambiguity() {
    fn file(path: &str, language: SourceLanguage, parsed: ParsedSource) -> SourceFile {
        SourceFile {
            path: path.to_owned(),
            language,
            extension: path.rsplit('.').next().unwrap_or_default().to_owned(),
            worktree_state: WorktreeState::Tracked,
            status: parsed.status,
            symbols: parsed.symbols,
            limitations: parsed.limitations,
            classifications: Vec::new(),
            classification_overridden: false,
        }
    }

    let rust_caller = file(
        "src/caller.rs",
        SourceLanguage::Rust,
        parse_source(b"fn caller() { target(); }", &RUST_SUPPORT),
    );
    let rust_target = file(
        "src/target.rs",
        SourceLanguage::Rust,
        parse_source(b"pub fn target() {}", &RUST_SUPPORT),
    );
    assert!(build_lexical_edges(&[rust_caller, rust_target], 32, 32).is_empty());
    let rust_imported_caller = file(
        "src/imported.rs",
        SourceLanguage::Rust,
        parse_source(b"use crate::target::target; fn caller() { target(); }", &RUST_SUPPORT),
    );
    let rust_imported_edges = build_lexical_edges(
        &[
            rust_imported_caller,
            file(
                "src/target.rs",
                SourceLanguage::Rust,
                parse_source(b"pub fn target() {}", &RUST_SUPPORT),
            ),
        ],
        32,
        32,
    );
    let rust_imported_edge = rust_imported_edges.first().expect("Rust import-aware edge");
    assert_eq!(
        rust_imported_edge.resolution_reason,
        LexicalResolutionReason::ImportedModule
    );
    assert_eq!(rust_imported_edge.confidence, ConfidenceTier::High);

    let go_same_package = build_lexical_edges(
        &[
            file(
                "service/caller.go",
                SourceLanguage::Go,
                parse_source(b"package service\nfunc caller() { Build() }", &GO_SUPPORT),
            ),
            file(
                "service/build.go",
                SourceLanguage::Go,
                parse_source(b"package service\nfunc Build() {}", &GO_SUPPORT),
            ),
        ],
        32,
        32,
    );
    let go_edge = go_same_package.first().expect("Go same-package edge");
    assert_eq!(go_edge.symbol, "Build");
    assert_eq!(go_edge.resolution_reason, LexicalResolutionReason::SameModule);
    assert_eq!(go_edge.confidence, ConfidenceTier::High);

    let go_different_directories = build_lexical_edges(
        &[
            file(
                "one/caller.go",
                SourceLanguage::Go,
                parse_source(b"package service\nfunc caller() { Build() }", &GO_SUPPORT),
            ),
            file(
                "two/build.go",
                SourceLanguage::Go,
                parse_source(b"package service\nfunc Build() {}", &GO_SUPPORT),
            ),
        ],
        32,
        32,
    );
    assert!(go_different_directories.is_empty());

    let go_imported = build_lexical_edges(
        &[
            file(
                "cmd/app/main.go",
                SourceLanguage::Go,
                parse_source(
                    b"package main\nimport client \"example.com/project/internal/client\"\nfunc main() { client.Build() }",
                    &GO_SUPPORT,
                ),
            ),
            file(
                "internal/client/build.go",
                SourceLanguage::Go,
                parse_source(b"package client\nfunc Build() {}", &GO_SUPPORT),
            ),
        ],
        32,
        32,
    );
    let imported_go_edge = go_imported.first().expect("Go import-path edge");
    assert_eq!(imported_go_edge.symbol, "Build");
    assert_eq!(
        imported_go_edge.resolution_reason,
        LexicalResolutionReason::ImportedModule
    );

    let lua_imported = build_lexical_edges(
        &[
            file(
                "app/main.lua",
                SourceLanguage::Lua,
                parse_source(
                    b"local helper = require('app.helper')\nreturn helper.build()",
                    &LUA_SUPPORT,
                ),
            ),
            file(
                "app/helper.lua",
                SourceLanguage::Lua,
                parse_source(b"local M = {}\nfunction M.build() end\nreturn M", &LUA_SUPPORT),
            ),
            file(
                "app/helper.js",
                SourceLanguage::JavaScript,
                parse_source(b"export function build() {}", &JAVASCRIPT_SUPPORT),
            ),
        ],
        32,
        32,
    );
    let lua_edge = lua_imported.first().expect("Lua literal-require edge");
    assert_eq!(lua_edge.source, "app/main.lua");
    assert_eq!(lua_edge.target, "app/helper.lua");
    assert_eq!(lua_edge.symbol, "build");
    assert_eq!(lua_edge.resolution_reason, LexicalResolutionReason::ImportedModule);
    assert!(lua_imported.iter().all(|edge| edge.target != "app/helper.js"));

    let javascript_caller = file(
        "src/caller.js",
        SourceLanguage::JavaScript,
        parse_source(
            br#"import { target } from "./target.js";
target();"#,
            &JAVASCRIPT_SUPPORT,
        ),
    );
    let javascript_target = file(
        "src/target.js",
        SourceLanguage::JavaScript,
        parse_source(b"export function target() {}", &JAVASCRIPT_SUPPORT),
    );
    let explicit_edges = build_lexical_edges(&[javascript_caller, javascript_target], 32, 32);
    let edge = explicit_edges.first().expect("import-aware edge");
    assert_eq!(edge.resolution_reason, LexicalResolutionReason::ImportedModule);
    assert_eq!(edge.confidence, ConfidenceTier::High);
    assert_eq!(edge.target_visibility, SymbolVisibility::Public);
    assert!(!edge.candidate_group.is_empty());

    let dotted_javascript_import = build_lexical_edges(
        &[
            file(
                "src/caller.js",
                SourceLanguage::JavaScript,
                parse_source(
                    br#"import { target } from "./foo.bar.js";
target();"#,
                    &JAVASCRIPT_SUPPORT,
                ),
            ),
            file(
                "src/foo.bar.js",
                SourceLanguage::JavaScript,
                parse_source(b"export function target() {}", &JAVASCRIPT_SUPPORT),
            ),
            file(
                "src/foo/bar.js",
                SourceLanguage::JavaScript,
                parse_source(b"export function target() {}", &JAVASCRIPT_SUPPORT),
            ),
        ],
        32,
        32,
    );
    assert_eq!(dotted_javascript_import.len(), 1);
    assert_eq!(dotted_javascript_import[0].target, "src/foo.bar.js");

    let ambiguous_caller = file(
        "src/ambiguous.js",
        SourceLanguage::JavaScript,
        parse_source(
            br#"import { target } from "./unknown.js";
target();"#,
            &JAVASCRIPT_SUPPORT,
        ),
    );
    let ambiguous_one = file(
        "src/one.js",
        SourceLanguage::JavaScript,
        parse_source(b"export function target() {}", &JAVASCRIPT_SUPPORT),
    );
    let ambiguous_two = file(
        "src/two.js",
        SourceLanguage::JavaScript,
        parse_source(b"export function target() {}", &JAVASCRIPT_SUPPORT),
    );
    let ambiguous_files = [ambiguous_caller, ambiguous_one, ambiguous_two];
    let ambiguous_edges = build_lexical_edges(&ambiguous_files, 32, 32);
    assert_eq!(ambiguous_edges.len(), 2);
    assert!(ambiguous_edges.iter().all(|edge| edge.ambiguous));
    assert_eq!(ambiguous_edges[0].candidate_group, ambiguous_edges[1].candidate_group);
    assert!(
        build_lexical_edges(&ambiguous_files, 1, 32)
            .iter()
            .all(|edge| edge.candidates.len() <= 1)
    );
    assert!(build_lexical_edges(&ambiguous_files, 32, 1).len() <= 1);
    assert!(build_lexical_edges(&ambiguous_files, 32, 0).is_empty());
    let mut findings = Vec::new();
    add_ambiguity_findings(&ambiguous_edges, &mut findings, 32);
    assert_eq!(findings.len(), 1);
    assert!(findings[0].detail.contains("deduplicated definition candidates"));
}

#[test]
fn cache_file_paths_normalize_without_basename_matching_or_scope_leaks() {
    let repository = Path::new("/repo");
    assert_eq!(
        normalized_cache_file_path("src\\lib.rs", repository),
        Some("src/lib.rs".to_owned())
    );
    assert_eq!(
        normalized_cache_file_path("/repo/src/./lib.rs", repository),
        Some("src/lib.rs".to_owned())
    );
    assert_eq!(
        normalized_cache_file_path("lib.rs", repository),
        Some("lib.rs".to_owned())
    );
    assert_eq!(normalized_cache_file_path("../src/lib.rs", repository), None);
}

#[test]
fn cache_pruning_is_count_bounded_and_path_deterministic() {
    let root = std::env::temp_dir().join(format!("dalil-cache-prune-{}", std::process::id()));
    let repository = root.join("repositories").join("repo");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&repository).expect("create cache prune fixture");
    for index in 0..(CACHE_MAX_RECORDS_PER_REPOSITORY + 4) {
        fs::write(repository.join(format!("record-{index:03}.json")), b"{}").expect("write cache prune record");
    }

    let (removed, _) = prune_cache_directory(&root, &repository).expect("prune cache fixture");
    assert_eq!(removed, 4);
    let remaining = collect_cache_files(&root, &repository).expect("read pruned cache fixture");
    assert_eq!(remaining.len(), CACHE_MAX_RECORDS_PER_REPOSITORY);
    fs::remove_dir_all(root).expect("remove cache prune fixture");
}

#[test]
fn query_failure_is_partial_and_does_not_discard_definition_findings() {
    let support =
        LanguageSupport { references: "(not_a_real_javascript_node) @reference.identifier", ..JAVASCRIPT_SUPPORT };
    let parsed = parse_source(b"function build() {}", &support);

    assert_eq!(parsed.status, FileAnalysisStatus::Partial);
    assert!(
        parsed
            .findings
            .iter()
            .any(|finding| finding.kind == MapFindingKind::QueryError)
    );
    assert!(
        parsed
            .symbols
            .iter()
            .any(|symbol| symbol.name == "build" && symbol.role == SymbolRole::Definition)
    );
    assert!(
        parsed
            .limitations
            .iter()
            .any(|limitation| limitation.contains("query-pack queries failed"))
    );
}

#[test]
fn snippet_selection_respects_every_tiny_token_budget() {
    let source = b"pub fn a_very_long_name(value: usize) { let _ = value; }";
    let ParsedSource { symbols, .. } = parse_source(source, &RUST_SUPPORT);
    let file = SourceFile {
        path: "src/lib.rs".to_owned(),
        language: SourceLanguage::Rust,
        extension: "rs".to_owned(),
        worktree_state: WorktreeState::Tracked,
        status: FileAnalysisStatus::Complete,
        symbols,
        limitations: Vec::new(),
        classifications: Vec::new(),
        classification_overridden: false,
    };
    let ranking = vec![FileRank {
        path: file.path.clone(),
        score: 1_000_000,
        focus_matches: 0,
        contributions: RankingContributions::default(),
        matched_seeds: Vec::new(),
    }];
    let settings = MapSettings::default();
    for budget in 1..=64 {
        let selection = select_snippets(std::slice::from_ref(&file), &[], &ranking, &[], budget, &settings);
        assert!(selection.estimated_tokens <= budget, "budget {budget}: {selection:?}");
        assert!(
            selection
                .snippets
                .iter()
                .all(|snippet| snippet.estimated_tokens <= budget)
        );
    }
}

#[test]
fn snippet_selection_is_diverse_and_excludes_unfocused_generated_code() {
    let file = |path: &str| {
        let ParsedSource { symbols, .. } = parse_source(b"pub fn selected() {}", &RUST_SUPPORT);
        SourceFile {
            path: path.to_owned(),
            language: SourceLanguage::Rust,
            extension: "rs".to_owned(),
            worktree_state: WorktreeState::Tracked,
            status: FileAnalysisStatus::Complete,
            symbols,
            limitations: Vec::new(),
            classifications: Vec::new(),
            classification_overridden: false,
        }
    };
    let mut generated = file("src/generated/api.rs");
    generated.classifications.push(SourceClassification {
        kind: SourceClassificationKind::Generated,
        reason: "generated_directory:generated".to_owned(),
    });
    let files = vec![
        file("src/worker.rs"),
        file("src/core/hub.rs"),
        file("src/core/other.rs"),
        file("examples/demo.rs"),
        file("tests/worker.rs"),
        generated,
    ];
    let ranking = files
        .iter()
        .map(|file| {
            let (score, matched_seeds) = match file.path.as_str() {
                "src/worker.rs" => (
                    10_000_000,
                    vec![RankingSeedMatch { kind: RankingSeedKind::TaskTerm, seed: "worker".to_owned() }],
                ),
                "src/core/hub.rs" | "src/core/other.rs" => (9_000_000, Vec::new()),
                "examples/demo.rs" => (3_000_000, Vec::new()),
                "tests/worker.rs" => (2_000_000, Vec::new()),
                _ => (100_000_000, Vec::new()),
            };
            FileRank {
                path: file.path.clone(),
                score,
                focus_matches: 0,
                contributions: RankingContributions::default(),
                matched_seeds,
            }
        })
        .collect::<Vec<_>>();

    let first = select_snippets(&files, &[], &ranking, &[], 1_000, &MapSettings::default());
    let second = select_snippets(&files, &[], &ranking, &[], 1_000, &MapSettings::default());

    assert_eq!(first, second);
    assert!((3..=5).contains(&first.snippets.len()));
    assert!(first.snippets.iter().any(|snippet| snippet.path == "src/worker.rs"));
    assert_eq!(first.snippets[1].path, "src/core/hub.rs");
    assert!(first.snippets.iter().any(|snippet| snippet.path == "examples/demo.rs"));
    assert!(first.snippets.iter().any(|snippet| snippet.path == "tests/worker.rs"));
    assert!(
        first
            .snippets
            .iter()
            .all(|snippet| snippet.path != "src/generated/api.rs")
    );
    assert_eq!(first.primary_languages, vec![SourceLanguage::Rust]);
}

#[test]
fn snippet_selection_covers_multiple_relevant_project_roots() {
    let file = |path: &str| {
        let ParsedSource { symbols, .. } = parse_source(b"pub fn selected() {}", &RUST_SUPPORT);
        SourceFile {
            path: path.to_owned(),
            language: SourceLanguage::Rust,
            extension: "rs".to_owned(),
            worktree_state: WorktreeState::Tracked,
            status: FileAnalysisStatus::Complete,
            symbols,
            limitations: Vec::new(),
            classifications: Vec::new(),
            classification_overridden: false,
        }
    };
    let files = vec![
        file("packages/api/src/hub.rs"),
        file("packages/api/src/other.rs"),
        file("packages/web/src/app.rs"),
    ];
    let roots = vec![
        ProjectRoot {
            path: "packages/api".to_owned(),
            kind: ProjectRootKind::Package,
            reason: "fixture package".to_owned(),
            manifests: Vec::new(),
            manifest_metadata: Vec::new(),
            landmark_total: 0,
            recommendation_total: 0,
            recommended_paths: Vec::new(),
        },
        ProjectRoot {
            path: "packages/web".to_owned(),
            kind: ProjectRootKind::Package,
            reason: "fixture package".to_owned(),
            manifests: Vec::new(),
            manifest_metadata: Vec::new(),
            landmark_total: 0,
            recommendation_total: 0,
            recommended_paths: Vec::new(),
        },
    ];
    let ranking = files
        .iter()
        .map(|file| FileRank {
            path: file.path.clone(),
            score: match file.path.as_str() {
                "packages/api/src/hub.rs" => 10_000_000,
                "packages/api/src/other.rs" => 9_000_000,
                _ => 8_000_000,
            },
            focus_matches: 0,
            contributions: RankingContributions::default(),
            matched_seeds: Vec::new(),
        })
        .collect::<Vec<_>>();

    let selection = select_snippets(&files, &[], &ranking, &roots, 1_000, &MapSettings::default());

    assert_eq!(selection.snippets.len(), 3);
    assert!(
        selection
            .snippets
            .iter()
            .any(|snippet| snippet.path == "packages/api/src/hub.rs")
    );
    assert!(
        selection
            .snippets
            .iter()
            .any(|snippet| snippet.path == "packages/web/src/app.rs")
    );
}

#[test]
fn snippet_selection_reports_task_relevant_paths_that_do_not_fit() {
    let ParsedSource { symbols, .. } = parse_source(b"pub fn worker() {}", &RUST_SUPPORT);
    let file = SourceFile {
        path: "src/worker.rs".to_owned(),
        language: SourceLanguage::Rust,
        extension: "rs".to_owned(),
        worktree_state: WorktreeState::Tracked,
        status: FileAnalysisStatus::Complete,
        symbols,
        limitations: Vec::new(),
        classifications: Vec::new(),
        classification_overridden: false,
    };
    let ranking = vec![FileRank {
        path: file.path.clone(),
        score: 1_000_000,
        focus_matches: 0,
        contributions: RankingContributions::default(),
        matched_seeds: vec![RankingSeedMatch { kind: RankingSeedKind::TaskTerm, seed: "worker".to_owned() }],
    }];

    let selection = select_snippets(
        std::slice::from_ref(&file),
        &[],
        &ranking,
        &[],
        1,
        &MapSettings::default(),
    );

    assert_eq!(
        selection.shortfall.as_ref().map(|shortfall| shortfall.returned),
        Some(0)
    );
    assert_eq!(selection.omitted_relevant_paths.len(), 1);
    assert_eq!(selection.omitted_relevant_paths[0].path, "src/worker.rs");
}

#[test]
fn task_text_changes_ranking_and_retains_its_contributions() {
    let file = |path: &str, source: &[u8]| {
        let ParsedSource { symbols, .. } = parse_source(source, &RUST_SUPPORT);
        SourceFile {
            path: path.to_owned(),
            language: SourceLanguage::Rust,
            extension: "rs".to_owned(),
            worktree_state: WorktreeState::Tracked,
            status: FileAnalysisStatus::Complete,
            symbols,
            limitations: Vec::new(),
            classifications: Vec::new(),
            classification_overridden: false,
        }
    };
    let files = [
        file("src/parser.rs", b"pub fn parse_request() {}"),
        file("src/render.rs", b"pub fn render_response() {}"),
    ];
    let parser_settings = MapSettings {
        task_seeds: TaskSeeds { task: Some("parse request".to_owned()), ..TaskSeeds::default() },
        ..MapSettings::default()
    };
    let render_settings = MapSettings {
        task_seeds: TaskSeeds { task: Some("render response".to_owned()), ..TaskSeeds::default() },
        ..MapSettings::default()
    };

    let parser_ranking = rank_files(&files, &[], &[], &BTreeMap::new(), &parser_settings);
    let render_ranking = rank_files(&files, &[], &[], &BTreeMap::new(), &render_settings);

    assert_eq!(parser_ranking[0].path, "src/parser.rs");
    assert_eq!(render_ranking[0].path, "src/render.rs");
    assert!(parser_ranking[0].contributions.centrality > parser_ranking[1].contributions.centrality);
    assert!(render_ranking[0].contributions.centrality > render_ranking[1].contributions.centrality);
    assert!(parser_ranking[0].contributions.lexical_relevance > 0);
    assert!(
        parser_ranking[0]
            .matched_seeds
            .iter()
            .any(|seed| seed.kind == RankingSeedKind::TaskTerm && seed.seed == "parse")
    );
    assert_eq!(
        parser_ranking[0].score,
        parser_ranking[0]
            .contributions
            .centrality
            .saturating_add(parser_ranking[0].contributions.seed_proximity)
            .saturating_add(parser_ranking[0].contributions.lexical_relevance)
            .saturating_add(parser_ranking[0].contributions.history_evidence)
            .saturating_add(parser_ranking[0].contributions.explicit_focus)
    );
}

#[test]
fn classification_rules_are_deterministic_and_conservative() {
    let generated = classified_path("src/generated/parser.generated.rs");
    assert!(generated.iter().any(|classification| {
        classification.kind == SourceClassificationKind::Generated
            && classification.reason == "generated_directory:generated"
    }));
    assert!(generated.iter().any(|classification| {
        classification.kind == SourceClassificationKind::Generated && classification.reason == "generated_filename"
    }));
    assert!(
        classified_path("vendor/bundle.min.js")
            .iter()
            .any(|classification| { classification.kind == SourceClassificationKind::Vendor })
    );
    assert!(
        classified_path("assets/app.js.map")
            .iter()
            .any(|classification| { classification.kind == SourceClassificationKind::SourceMap })
    );
    assert!(classified_path("src/generated_parser.rs").is_empty());
}

#[test]
fn generated_markers_and_bounded_minification_are_content_evidence() {
    assert!(generated_marker(
        "// Code generated by fixture. DO NOT EDIT.\nfn generated() {}"
    ));
    assert!(!generated_marker("fn main() { println!(\"generated file\"); }"));

    let minified = format!("const value={};", "x".repeat(1_200));
    assert!(minified_heuristic("bundle.js", &minified));
    assert!(!minified_heuristic("maintained.rs", &minified));
    assert!(!minified_heuristic(
        "maintained.js",
        &format!("const value = {};\n", "x ".repeat(600))
    ));
}

fn rust_file(path: &str, source: &str) -> SourceFile {
    let parsed = parse_source(source.as_bytes(), &RUST_SUPPORT);
    SourceFile {
        path: path.to_owned(),
        language: SourceLanguage::Rust,
        extension: "rs".to_owned(),
        worktree_state: WorktreeState::Tracked,
        status: parsed.status,
        symbols: parsed.symbols,
        limitations: parsed.limitations,
        classifications: Vec::new(),
        classification_overridden: false,
    }
}

#[test]
fn qualified_paths_resolve_cross_file_calls_without_imports() {
    let caller = rust_file("src/caller.rs", "fn caller() { target::target(); }");
    let call = caller
        .symbols
        .iter()
        .find(|symbol| symbol.name == "target" && symbol.location.start.column == 23)
        .expect("qualified call reference");
    assert_eq!(call.evidence, SymbolEvidence::Call);
    assert_eq!(call.qualifier.as_deref(), Some("target"));

    // An unanchored `target::` needs a `mod target;` in the crate.
    let lib = rust_file("src/lib.rs", "mod target;");
    let edges = build_lexical_edges(
        &[caller, lib.clone(), rust_file("src/target.rs", "pub fn target() {}")],
        32,
        32,
    );
    let edge = edges.first().expect("qualified-path edge");
    assert_eq!(edge.target, "src/target.rs");
    assert_eq!(edge.resolution_reason, LexicalResolutionReason::QualifiedPath);
    assert_eq!(edge.confidence, ConfidenceTier::High);

    let crate_path = rust_file("src/crate_path.rs", "fn caller() { crate::target::target(); }");
    assert_eq!(
        build_lexical_edges(&[crate_path, rust_file("src/target.rs", "pub fn target() {}")], 32, 32)
            .first()
            .map(|edge| edge.resolution_reason),
        Some(LexicalResolutionReason::QualifiedPath)
    );

    for source in ["fn caller() { other::target(); }", "fn caller() { super::target(); }"] {
        let edges = build_lexical_edges(
            &[
                rust_file("src/caller.rs", source),
                lib.clone(),
                rust_file("src/target.rs", "pub fn target() {}"),
            ],
            32,
            32,
        );
        assert!(edges.is_empty(), "{source} produced {edges:?}");
    }

    // `fs` names std's module here, not src/util/fs.rs.
    let std_import = rust_file("src/caller.rs", "use std::fs;\nfn caller() { fs::read(\"x\"); }");
    let util_fs = rust_file("src/util/fs.rs", "pub fn read(path: &str) {}");
    let edges = build_lexical_edges(&[std_import, util_fs.clone()], 32, 32);
    assert!(
        edges
            .iter()
            .all(|edge| edge.resolution_reason != LexicalResolutionReason::QualifiedPath),
        "{edges:?}"
    );

    // A `use` from inside the crate keeps the qualified-path edge.
    let crate_import = rust_file(
        "src/caller.rs",
        "use crate::util::fs;\nfn caller() { fs::read(\"x\"); }",
    );
    let edges = build_lexical_edges(&[crate_import, util_fs], 32, 32);
    assert!(
        edges.iter().any(|edge| edge.target == "src/util/fs.rs"
            && edge.resolution_reason == LexicalResolutionReason::QualifiedPath
            && edge.confidence == ConfidenceTier::High),
        "{edges:?}"
    );
}

fn qualified_edges(files: &[SourceFile]) -> Vec<(String, ConfidenceTier)> {
    build_lexical_edges(files, 32, 32)
        .into_iter()
        .filter(|edge| edge.resolution_reason == LexicalResolutionReason::QualifiedPath)
        .map(|edge| (edge.target, edge.confidence))
        .collect()
}

#[test]
fn qualified_paths_resolve_to_mod_rs() {
    let edges = qualified_edges(&[
        rust_file("src/lib.rs", "mod map;"),
        rust_file("src/caller.rs", "fn caller() { map::target(); }"),
        rust_file("src/map/mod.rs", "pub fn target() {}"),
    ]);
    assert_eq!(edges, [("src/map/mod.rs".to_owned(), ConfidenceTier::High)]);
}

#[test]
fn qualified_paths_stay_inside_the_callers_crate() {
    let edges = qualified_edges(&[
        rust_file("crates/a/src/lib.rs", "mod caller;\nmod util;"),
        rust_file("crates/a/src/caller.rs", "fn caller() { util::helper(); }"),
        rust_file("crates/b/src/lib.rs", "mod util;"),
        rust_file("crates/b/src/util.rs", "pub fn helper() {}"),
    ]);
    assert!(edges.is_empty(), "{edges:?}");

    let edges = qualified_edges(&[
        rust_file("crates/a/src/lib.rs", "mod caller;\nmod util;"),
        rust_file("crates/a/src/caller.rs", "fn caller() { util::helper(); }"),
        rust_file("crates/a/src/util.rs", "pub fn helper() {}"),
        rust_file("crates/b/src/lib.rs", "mod util;"),
        rust_file("crates/b/src/util.rs", "pub fn helper() {}"),
    ]);
    assert_eq!(edges, [("crates/a/src/util.rs".to_owned(), ConfidenceTier::High)]);
}

#[test]
fn extern_crate_paths_without_a_module_declaration_do_not_resolve() {
    let edges = qualified_edges(&[
        rust_file("src/caller.rs", "fn caller() { serde_json::from_str(\"{}\"); }"),
        rust_file("src/serde_json.rs", "pub fn from_str(value: &str) {}"),
    ]);
    assert!(edges.is_empty(), "{edges:?}");
}

#[test]
fn crate_source_roots() {
    assert_eq!(graph::crate_source_root("crates/a/src/map/mod.rs"), "crates/a/src/");
    assert_eq!(graph::crate_source_root("src/caller.rs"), "src/");
    assert_eq!(graph::crate_source_root("tests/cli/main.rs"), "tests/");
    assert_eq!(graph::crate_source_root("build.rs"), "");
}

#[test]
fn repeated_references_do_not_use_up_the_edge_limit() {
    let caller = rust_file(
        "src/caller.rs",
        "fn caller() { target::first(); target::first(); target::first(); target::first(); target::second(); }",
    );
    let target = rust_file("src/target.rs", "pub fn first() {}\npub fn second() {}");
    let lib = rust_file("src/lib.rs", "mod target;");
    let edges = build_lexical_edges(&[caller, lib, target], 32, 2);
    let symbols = edges.iter().map(|edge| edge.symbol.as_str()).collect::<Vec<_>>();
    assert_eq!(symbols, ["first", "second"]);
}

#[test]
fn definitions_and_calls_survive_the_per_file_symbol_limit() {
    let mut source = String::from("fn filler(value: usize) {\n");
    for _ in 0..200 {
        source.push_str("    let _ = value;\n");
    }
    source.push_str("}\npub fn late() { target::late_call(); late_bare; }\n");
    let mut analyzer = LanguageAnalyzer::new(&RUST_SUPPORT);
    let parsed = parse_source_with_analyzer(
        source.as_bytes(),
        &mut analyzer,
        &ReportLimits::for_profile(AnalysisProfile::Compact),
    );

    assert!(parsed.symbols.len() > 128);
    assert!(
        parsed
            .symbols
            .iter()
            .any(|symbol| symbol.name == "late" && symbol.role == SymbolRole::Definition)
    );
    assert!(
        parsed
            .symbols
            .iter()
            .any(|symbol| symbol.name == "late_call" && symbol.evidence == SymbolEvidence::Call)
    );
    assert!(!parsed.symbols.iter().any(|symbol| symbol.name == "late_bare"));
    assert!(
        parsed
            .limitations
            .iter()
            .any(|limitation| limitation.contains("per-file symbol limit (128)"))
    );
}

#[test]
fn definitions_and_calls_stop_at_the_exempt_per_file_cap() {
    let mut source = String::from("fn caller() {\n");
    for index in 0..200 {
        source.push_str(&format!("    callee_{index}();\n"));
    }
    source.push_str("}\n");
    let limits = ReportLimits { max_symbols_per_file: 4, ..ReportLimits::for_profile(AnalysisProfile::Evidence) };
    let mut analyzer = LanguageAnalyzer::new(&RUST_SUPPORT);
    let parsed = parse_source_with_analyzer(source.as_bytes(), &mut analyzer, &limits);

    assert_eq!(parsed.symbols.len(), parser::exempt_symbol_limit(4));
    assert_eq!(parsed.symbols.len(), 64);
    assert!(
        parsed
            .symbols
            .iter()
            .any(|symbol| symbol.name == "caller" && symbol.role == SymbolRole::Definition)
    );
    assert!(
        parsed
            .limitations
            .iter()
            .any(|limitation| limitation.contains("limit for definitions and calls (64)")),
        "{:?}",
        parsed.limitations
    );
}

#[test]
fn the_total_symbol_budget_drops_bare_references_and_keeps_definitions_and_calls() {
    let mut symbols = rust_file(
        "src/lib.rs",
        "fn first(value: usize) { let _ = value; }\nfn late() { target::late_call(); late_bare; }",
    )
    .symbols;
    let bare_references = symbols
        .iter()
        .filter(|symbol| !parser::retained_past_symbol_limits(symbol))
        .count();
    assert!(bare_references > 1);

    analysis::retain_within_symbol_budget(&mut symbols, 1);

    assert_eq!(
        symbols
            .iter()
            .filter(|symbol| !parser::retained_past_symbol_limits(symbol))
            .count(),
        1
    );
    assert!(
        symbols
            .iter()
            .any(|symbol| symbol.name == "late" && symbol.role == SymbolRole::Definition)
    );
    assert!(
        symbols
            .iter()
            .any(|symbol| symbol.name == "late_call" && symbol.evidence == SymbolEvidence::Call)
    );
    assert!(!symbols.iter().any(|symbol| symbol.name == "late_bare"));
}

fn cache_fixture(name: &str) -> (PathBuf, CacheStore) {
    let root = std::env::temp_dir().join(format!("dalil-cache-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create cache fixture");
    let repository_root = "/repository".to_owned();
    let store =
        CacheStore { root: Some(root.clone()), repository_id: digest_hex(repository_root.as_bytes()), repository_root };
    (root, store)
}

fn cache_index_identity() -> IndexIdentity {
    IndexIdentity {
        scope_path: String::new(),
        head: None,
        worktree_state: String::new(),
        manifest_state: String::new(),
        profile: AnalysisProfile::Compact,
        map_tokens: DEFAULT_MAP_TOKENS,
        analysis_options: String::new(),
        content_state: String::new(),
        language_pack_identity: BTreeMap::new(),
    }
}

/// Rewrites the `tool_version` of every cache file under `root`.
fn restamp_cache_files(root: &Path, tool_version: &str) {
    for entry in WalkBuilder::new(root).hidden(false).build().flatten() {
        if !entry.path().is_file() {
            continue;
        }
        let mut value: serde_json::Value =
            serde_json::from_slice(&fs::read(entry.path()).expect("read cache file")).expect("cache JSON");
        value["tool_version"] = serde_json::Value::String(tool_version.to_owned());
        fs::write(entry.path(), serde_json::to_vec(&value).expect("cache JSON")).expect("write cache file");
    }
}

#[test]
fn cache_written_by_the_previous_tool_version_is_rejected() {
    // Version 10 stored edges without qualified-path resolution; reusing them
    // would hide callers, so this version must not read that cache.
    assert_ne!(CACHE_TOOL_VERSION, "dalil-map-v10");
    let (root, store) = cache_fixture("tool-version");
    let identity = cache_index_identity();
    let parsed = parse_source(b"pub fn target() {}", &RUST_SUPPORT);
    assert!(
        store
            .write("src/lib.rs", &RUST_SUPPORT, "fingerprint", 128, &parsed)
            .is_none()
    );
    assert!(
        store
            .write_index(
                &identity,
                BTreeMap::new(),
                Vec::new(),
                Vec::new(),
                BTreeMap::new(),
                TrigramIndex::default()
            )
            .is_none()
    );
    assert_eq!(store.load_index(&identity).status, PersistentIndexStatus::Hit);
    assert!(
        store
            .load("src/lib.rs", &RUST_SUPPORT, "fingerprint", 128, CacheMode::Auto)
            .is_some()
    );

    restamp_cache_files(&root, "dalil-map-v10");
    assert_eq!(store.load_index(&identity).status, PersistentIndexStatus::Incompatible);
    assert!(
        store
            .load("src/lib.rs", &RUST_SUPPORT, "fingerprint", 128, CacheMode::Auto)
            .is_none()
    );
    fs::remove_dir_all(root).expect("remove cache fixture");
}

#[test]
fn manual_cache_mode_skips_records_parsed_under_another_symbol_limit() {
    let (root, store) = cache_fixture("manual-limit");
    let parsed = parse_source(b"pub fn target() {}", &RUST_SUPPORT);
    assert!(store.write("src/lib.rs", &RUST_SUPPORT, "old", 128, &parsed).is_none());

    let same_limit = store
        .load("src/lib.rs", &RUST_SUPPORT, "new", 128, CacheMode::Manual)
        .expect("record under the same limit");
    assert!(same_limit.stale);
    assert!(
        store
            .load("src/lib.rs", &RUST_SUPPORT, "new", 2_048, CacheMode::Manual)
            .is_none()
    );
    assert!(
        store
            .load("src/lib.rs", &RUST_SUPPORT, "old", 2_048, CacheMode::Manual)
            .is_none()
    );
    fs::remove_dir_all(root).expect("remove cache fixture");
}

#[test]
fn changed_definitions_map_body_edits_to_their_enclosing_definition() {
    let source = "use std::fmt::Write;\n\
struct First;\n\
impl First {\n\
    fn new() -> Self {\n\
        helper(); helper(); writeln!(out, \"x\").expect(\"write\");\n\
        Self\n\
    }\n\
}\n\
struct Second;\n\
impl Second {\n\
    fn new() -> Self {\n\
        other(1, 2);\n\
        Self\n\
    }\n\
}\n";
    let symbols = parse_source(source.as_bytes(), &RUST_SUPPORT).symbols;
    let changed = |start: usize, end: usize| {
        changes::changed_definitions(&symbols, &[ChangedLineRange { start, end }])
            .into_iter()
            .map(|symbol| (symbol.name, symbol.location.start.line))
            .collect::<Vec<_>>()
    };

    assert_eq!(changed(5, 5), [("new".to_owned(), 4)]);
    assert_eq!(changed(12, 12), [("new".to_owned(), 11)]);
    assert_eq!(
        changed(5, 12),
        [("new".to_owned(), 4), ("Second".to_owned(), 9), ("new".to_owned(), 11)]
    );
    assert!(changed(1, 1).is_empty(), "imports are not changed definitions");
}

fn changed_names(source: &str, support: &LanguageSupport, line: usize) -> Vec<(String, usize)> {
    let symbols = parse_source(source.as_bytes(), support).symbols;
    changes::changed_definitions(&symbols, &[ChangedLineRange { start: line, end: line }])
        .into_iter()
        .map(|symbol| (symbol.name, symbol.location.start.line))
        .collect()
}

#[test]
fn changed_definitions_map_a_go_method_body_to_the_method() {
    let source = "package p\ntype R struct{}\nfunc (r R) M() {\n\tx := foo()\n\t_ = x\n}\n";
    assert_eq!(changed_names(source, &GO_SUPPORT, 4), [("M".to_owned(), 3)]);
}

#[test]
fn changed_definitions_map_a_javascript_arrow_body_to_its_binding() {
    let source = "const f = () => {\n  g();\n};\nconst h = function () {\n  const y = g();\n};\n";
    for support in [&JAVASCRIPT_SUPPORT, &TYPESCRIPT_SUPPORT] {
        assert_eq!(changed_names(source, support, 2), [("f".to_owned(), 1)]);
        assert_eq!(changed_names(source, support, 5), [("h".to_owned(), 4)]);
    }
}

#[test]
fn changed_definitions_map_a_named_function_expression_to_its_binding() {
    let source = "const f = function g() {\n  a();\n};\nconsole.log(function h() {\n  a();\n});\n";
    for support in [&JAVASCRIPT_SUPPORT, &TYPESCRIPT_SUPPORT] {
        assert_eq!(changed_names(source, support, 2), [("f".to_owned(), 1)]);
        let symbols = parse_source(source.as_bytes(), support).symbols;
        let unbound = symbols
            .iter()
            .find(|symbol| symbol.name == "a" && symbol.location.start.line == 5)
            .expect("reference inside the unbound expression");
        assert_eq!(unbound.scope, ["h"], "an unbound named expression keeps its name");
    }
}

#[test]
fn owning_definition_applies_its_filter_after_resolving_the_owner() {
    let source = "function b() {}\nclass A {\n  run() {}\n}\nconst run = () => {\n  b();\n};\n";
    let symbols = parse_source(source.as_bytes(), &JAVASCRIPT_SUPPORT).symbols;
    let call = symbols
        .iter()
        .find(|symbol| symbol.name == "b" && symbol.role != SymbolRole::Definition)
        .expect("call to b");
    let owner = changes::owning_definition(call, &symbols, |_| true).expect("owner of the call");
    assert_eq!((owner.name.as_str(), owner.location.start.line), ("run", 5));
    let not_variable = |symbol: &SourceSymbol| symbol.kind != SymbolKind::Variable;
    assert!(
        changes::owning_definition(call, &symbols, not_variable).is_none(),
        "a rejected owner must not fall back to the method `A.run`"
    );
}

#[test]
fn changed_definitions_map_lua_table_function_bodies_to_the_function() {
    let source = "local M = {}\nfunction M.foo()\n  g()\nend\nfunction M:bar()\n  g()\nend\nreturn M\n";
    assert_eq!(changed_names(source, &LUA_SUPPORT, 3), [("foo".to_owned(), 2)]);
    assert_eq!(changed_names(source, &LUA_SUPPORT, 6), [("bar".to_owned(), 5)]);
}

#[test]
fn changed_definitions_map_locals_to_their_function_and_keep_module_constants() {
    let source = "X = 1\ndef f():\n    x = g()\n    y = 2\n    return x\nclass C:\n    z = 3\n";
    assert_eq!(changed_names(source, &PYTHON_SUPPORT, 1), [("X".to_owned(), 1)]);
    assert_eq!(changed_names(source, &PYTHON_SUPPORT, 3), [("f".to_owned(), 2)]);
    assert_eq!(changed_names(source, &PYTHON_SUPPORT, 4), [("f".to_owned(), 2)]);
    assert_eq!(changed_names(source, &PYTHON_SUPPORT, 7), [("z".to_owned(), 7)]);
    let go = "package p\nfunc F() {\n\tx := 1\n\t_ = x\n}\n";
    assert_eq!(changed_names(go, &GO_SUPPORT, 3), [("F".to_owned(), 2)]);
}

fn line_ranges(changes: &changes::LineChanges) -> Vec<(usize, usize)> {
    changes.ranges.iter().map(|range| (range.start, range.end)).collect()
}

#[test]
fn changed_lines_report_one_range_per_hunk() {
    let ranges = |before: Option<&str>, after: Option<&str>| {
        line_ranges(&changes::changed_lines(
            before.map(str::as_bytes),
            after.map(str::as_bytes),
        ))
    };
    let before = "a\nb\nc\nd\ne\nf\ng\n";

    assert_eq!(ranges(Some(before), Some("A\nb\nc\nd\ne\nf\nG\n")), [(1, 1), (7, 7)]);
    assert_eq!(ranges(Some(before), Some("a\nb\nnew\nc\nd\ne\nf\ng\n")), [(3, 3)]);
    assert_eq!(ranges(None, Some("x\ny\n")), [(1, 2)]);
    assert_eq!(ranges(Some(before), Some(before)), []);
    assert!(
        changes::changed_lines(Some(b"a\n"), Some(&[0xff, 0xfe]))
            .ranges
            .is_empty()
    );
}

#[test]
fn changed_lines_list_pure_removals_without_a_range() {
    let changes = |before: &str, after: &str| changes::changed_lines(Some(before.as_bytes()), Some(after.as_bytes()));
    let before = "a\nb\nc\nd\ne\nf\ng\n";

    let middle = changes(before, "a\nb\nc\nf\ng\n");
    assert_eq!(line_ranges(&middle), []);
    assert_eq!(middle.removed_after, [3]);
    let top = changes(before, "b\nc\nd\ne\nf\ng\n");
    assert_eq!(line_ranges(&top), []);
    assert_eq!(top.removed_after, [0]);
    let emptied = changes(before, "");
    assert_eq!(line_ranges(&emptied), []);
    assert_eq!(emptied.removed_after, [0]);
    let mixed = changes(before, "A\nb\nc\nf\ng\n");
    assert_eq!(line_ranges(&mixed), [(1, 1)]);
    assert_eq!(mixed.removed_after, [3]);
    assert!(!mixed.over_limit);
}

#[test]
fn changed_lines_over_the_size_limit_report_one_range() {
    let changes = |before: &str, after: &str, max_lines: usize, max_bytes: usize| {
        changes::changed_lines_limited(Some(before.as_bytes()), Some(after.as_bytes()), max_lines, max_bytes)
    };
    let before = "a\nb\nc\nd\ne\n";

    let by_lines = changes(before, "A\nb\nc\nd\nE\n", 4, 1_000);
    assert!(by_lines.over_limit);
    assert_eq!(line_ranges(&by_lines), [(1, 5)]);
    let by_bytes = changes(before, "a\nB\nc\nD\ne\n", 100, 8);
    assert!(by_bytes.over_limit);
    assert_eq!(line_ranges(&by_bytes), [(2, 4)]);
    let removal = changes(before, "a\nb\ne\n", 2, 1_000);
    assert_eq!(line_ranges(&removal), []);
    assert_eq!(removal.removed_after, [2]);
    let unchanged = changes(before, "a\r\nb\r\nc\r\nd\r\ne\r\n", 2, 1_000);
    assert_eq!((line_ranges(&unchanged), unchanged.removed_after), (vec![], vec![]));
    let within = changes(before, "A\nb\nc\nd\nE\n", 5, 1_000);
    assert!(!within.over_limit);
    assert_eq!(line_ranges(&within), [(1, 1), (5, 5)]);
}

#[test]
fn changed_lines_ignore_line_terminators() {
    let ranges = |before: &str, after: &str| {
        line_ranges(&changes::changed_lines(Some(before.as_bytes()), Some(after.as_bytes())))
    };

    assert_eq!(ranges("fn a() {}\r\nfn b() {}\r\n", "fn a() {}\nfn b() {}\n"), []);
    assert_eq!(ranges("a\nb", "a\nb\nc\n"), [(3, 3)]);
    assert_eq!(ranges("a\r\nb\r\nc\r\n", "a\nB\r\nc\n"), [(2, 2)]);
}
