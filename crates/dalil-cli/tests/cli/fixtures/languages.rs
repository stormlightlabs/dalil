use std::{
    env, fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::Ordering,
    time::{SystemTime, UNIX_EPOCH},
};

use super::{FIXTURE_COUNTER, write_commit, write_file, write_tree};

pub(crate) struct MixedMapFixtureRepository {
    pub(crate) root: PathBuf,
    pub(crate) cache: PathBuf,
    pub(crate) temporary_root: PathBuf,
}

impl MixedMapFixtureRepository {
    pub(crate) fn new() -> Self {
        let suffix = format!(
            "dalil-mixed-map-{}-{}",
            std::process::id(),
            FIXTURE_COUNTER.fetch_add(1, Ordering::Relaxed)
        );
        let temporary_root = env::temp_dir().join(suffix);
        let root = temporary_root.join("repository");
        let cache = temporary_root.join("xdg-cache");
        fs::create_dir_all(root.join("src")).expect("create mixed map fixture source scope");
        fs::create_dir_all(root.join("scripts")).expect("create mixed map fixture entry-point scope");
        fs::create_dir_all(&cache).expect("create mixed map fixture cache");

        let tracked_files = [
            (".gitignore", "src/ignored.js\n"),
            ("README.md", "mixed-language source map fixture\n"),
            ("go.mod", "module example.com/fixture\n\ngo 1.24\n"),
            ("src/lib.rs", "pub fn parse() { let value = 1; let _ = value; }\n"),
            ("src/broken.js", "export function broken( {\n"),
            (
                "src/module.js",
                "import { helper } from \"./helper.js\";\nexport function build(value) { return new Widget(value, helper); }\nexport class Widget { render() { return helper(); } }\n",
            ),
            (
                "src/types.ts",
                "export interface User { name: string; }\nexport class Service { run(user: User) { return user.name; } }\nexport function create(user: User): Service { return new Service(); }\n",
            ),
            (
                "src/component.tsx",
                "export function View(props: { label: string }) { return <button>{props.label}</button>; }\n",
            ),
            (
                "src/service.py",
                "from helpers import helper\n\nclass Service:\n    def run(self, value):\n        return helper(value)\n\ndef create(value):\n    return Service().run(value)\n",
            ),
            ("src/broken.py", "def broken(:\n    pass\n"),
            (
                "src/service.rb",
                "module Billing\n  class Service\n    def run(value)\n      helper(value)\n    end\n  end\nend\n\ndef build\n  Service.new\nend\n",
            ),
            ("src/broken.rb", "def broken(\nend\n"),
            (
                "src/service.go",
                "package fixture\n\nimport \"fmt\"\n\ntype Service[T any] struct {\n    Value T\n    hidden int\n}\n\ntype Runner interface {\n    Run() error\n}\n\nconst Exported = 1\nvar localValue = 2\n\nfunc NewService[T any](value T) *Service[T] {\n    fmt.Println(value)\n    return &Service[T]{Value: value}\n}\n\nfunc (service *Service[T]) Run() error { return nil }\n",
            ),
            (
                "src/service_test.go",
                "package fixture\n\nfunc TestNewService(t *testing.T) {\n    NewService(1)\n}\n",
            ),
            ("src/duplicate_one.go", "package fixture\nfunc Duplicate() {}\n"),
            ("src/duplicate_two.go", "package fixture\nfunc Duplicate() {}\n"),
            (
                "src/duplicate_use.go",
                "package fixture\nfunc useDuplicate() { Duplicate() }\n",
            ),
            ("src/broken.go", "package fixture\nfunc Broken( {\n"),
            (
                "src/service.lua",
                "local helper = require('src.lua_helper')\nlocal M = {}\nfunction M.build(value) return helper.transform(value) end\nfunction M:render() return self:build(1) end\nreturn M\n",
            ),
            (
                "src/lua_helper.lua",
                "local M = {}\nfunction M.transform(value) return value end\nreturn M\n",
            ),
            ("src/duplicate_one.lua", "function duplicate() end\n"),
            ("src/duplicate_two.lua", "function duplicate() end\n"),
            ("src/duplicate_use.lua", "return duplicate()\n"),
            ("src/broken.lua", "local function broken(\nreturn { value = 1\n"),
            (
                "src/service.zig",
                "const helper = @import(\"zig_helper.zig\");\n\npub const Service = struct {\n    value: []const u8,\n    pub const Nested = union(enum) { text: []const u8, code: u32 };\n\n    pub fn create(comptime T: type, value: T) !Service {\n        _ = value;\n        return .{ .value = helper.render(@typeName(T)) };\n    }\n};\n\ntest \"service creates a value\" {\n    const service = try Service.create(u8, 1);\n    _ = service.value;\n}\n",
            ),
            (
                "src/zig_helper.zig",
                "pub fn render(value: []const u8) []const u8 { return value; }\n",
            ),
            ("src/broken.zig", "pub fn Broken( {\n"),
            (".luacheckrc", "return { globals = { 'vim' } }\n"),
            (
                "scripts/lua-tool",
                "#!/usr/bin/env lua\nlocal service = require('src.service')\nreturn service.build(1)\n",
            ),
        ];
        let repository = gix::init(&root).expect("initialize mixed map fixture repository");
        let tree = write_tree(&repository, &tracked_files);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time is after the Unix epoch")
            .as_secs() as i64;
        let commit = write_commit(
            &repository,
            tree,
            &[],
            "Mixed Map Fixture",
            "mixed@example.com",
            now,
            "Initial mixed-language source map fixture",
        );
        drop(repository);

        write_file(root.join(".git/HEAD"), b"ref: refs/heads/main\n");
        write_file(root.join(".git/refs/heads/main"), format!("{commit}\n").as_bytes());
        for (path, contents) in tracked_files {
            write_file(root.join(path), contents.as_bytes());
        }
        write_file(
            root.join("src/panel.jsx"),
            b"import React from \"react\";\nexport function Panel() { return <div />; }\n",
        );
        write_file(root.join("src/ignored.js"), b"export function ignored() {}\n");
        gix::open(&root).expect("open mixed map fixture repository");

        Self { root, cache, temporary_root }
    }

    pub(crate) fn run(&self, arguments: &[&str]) -> Output {
        self.command(arguments).output().expect("run mixed map fixture command")
    }

    pub(crate) fn command(&self, arguments: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_dalil"));
        command
            .args(arguments)
            .current_dir(&self.root)
            .env("XDG_CACHE_HOME", &self.cache);
        command
    }
}

impl Drop for MixedMapFixtureRepository {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.temporary_root);
    }
}

pub(crate) struct JavaCSharpMapFixtureRepository {
    pub(crate) root: PathBuf,
    pub(crate) cache: PathBuf,
    pub(crate) temporary_root: PathBuf,
}

impl JavaCSharpMapFixtureRepository {
    pub(crate) fn new() -> Self {
        let suffix = format!(
            "dalil-java-csharp-map-{}-{}",
            std::process::id(),
            FIXTURE_COUNTER.fetch_add(1, Ordering::Relaxed)
        );
        let temporary_root = env::temp_dir().join(suffix);
        let root = temporary_root.join("repository");
        let cache = temporary_root.join("xdg-cache");
        fs::create_dir_all(root.join("src")).expect("create Java and C# map fixture source scope");
        fs::create_dir_all(&cache).expect("create Java and C# map fixture cache");

        let tracked_files = [
            (".gitignore", "src/ignored.java\n"),
            ("README.md", "Java and C# source map fixture\n"),
            ("src/lib.rs", "pub fn parse() {}\n"),
            (
                "src/service.java",
                "package example;\nimport java.util.List;\n\npublic class Service extends BaseService {\n    private class Hidden {}\n\n    public Result run(Input input) {\n        return new Result(input.value());\n    }\n}\n\ninterface Runner {}\n",
            ),
            (
                "src/consumer.java",
                "package consumer;\n\nclass Consumer {\n    Service make() {\n        return new Service();\n    }\n}\n",
            ),
            (
                "src/service.cs",
                "using System;\n\nnamespace Example.App {\n    public class Service : BaseService, IRunner {\n        private class Hidden {}\n        private Helper helper;\n\n        public Result Run(Input input) {\n            helper.Execute(input);\n            return new Result();\n        }\n    }\n\n    public struct Value {}\n    public interface IRunner {}\n}\n",
            ),
            ("src/broken.cs", "namespace Broken {\n    public class Broken( {\n"),
        ];
        let repository = gix::init(&root).expect("initialize Java and C# map fixture repository");
        let tree = write_tree(&repository, &tracked_files);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time is after the Unix epoch")
            .as_secs() as i64;
        let commit = write_commit(
            &repository,
            tree,
            &[],
            "JVM and CLR Fixture",
            "languages@example.com",
            now,
            "Initial Java and C# source map fixture",
        );
        drop(repository);

        write_file(root.join(".git/HEAD"), b"ref: refs/heads/main\n");
        write_file(root.join(".git/refs/heads/main"), format!("{commit}\n").as_bytes());
        for (path, contents) in tracked_files {
            write_file(root.join(path), contents.as_bytes());
        }
        write_file(root.join("src/untracked.java"), b"package fresh; class Fresh {}\n");
        write_file(root.join("src/ignored.java"), b"package ignored; class Ignored {}\n");
        gix::open(&root).expect("open Java and C# map fixture repository");

        Self { root, cache, temporary_root }
    }

    pub(crate) fn run(&self, arguments: &[&str]) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_dalil"));
        command
            .args(arguments)
            .current_dir(&self.root)
            .env("XDG_CACHE_HOME", &self.cache);
        command.output().expect("run Java and C# map fixture command")
    }
}

impl Drop for JavaCSharpMapFixtureRepository {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.temporary_root);
    }
}
